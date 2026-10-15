use actix_session::Session;
use actix_web::{HttpResponse, Responder, delete, patch, post, web};
use serde::Deserialize;
use sqlx::PgPool;

use crate::routes::api::estado::usuario_de_sesion;
use crate::routes::api::recurrencias::materializar;

#[derive(Deserialize)]
pub struct RecIn {
    pub cuotas: Option<i16>,
}

/// Payload del form de captura (alta y edición mandan el mismo shape).
#[derive(Deserialize)]
pub struct MovIn {
    pub fecha: String,
    pub cuenta_id: i32,
    pub categoria_id: Option<i32>,
    pub concepto: String,
    pub monto: f64,
    pub es_ingreso: bool,
    pub moneda: String,
    pub rec: Option<RecIn>,
}

fn moneda_valida(moneda: &str) -> bool {
    moneda == "ARS" || moneda == "USD"
}

/// Valida un ISO YYYY-MM-DD parseable y con mes/día en rango.
fn fecha_valida(fecha: &str) -> bool {
    let b = fecha.as_bytes();
    if fecha.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    let (y, m, d) = (
        fecha[0..4].parse::<i64>().unwrap_or(0),
        fecha[5..7].parse::<i64>().unwrap_or(0),
        fecha[8..10].parse::<i64>().unwrap_or(0),
    );
    y >= 2000 && (1..=12).contains(&m) && (1..=31).contains(&d)
}

fn dia_de(fecha: &str) -> i16 {
    fecha[8..10].parse::<i16>().unwrap_or(1).min(28)
}

fn valida_comun(input: &MovIn) -> Result<(), String> {
    if !fecha_valida(&input.fecha) {
        return Err("La fecha debe ser un día real en formato AAAA-MM-DD".into());
    }
    if input.concepto.trim().is_empty() {
        return Err("El concepto es obligatorio".into());
    }
    if !(input.monto > 0.0 && input.monto.is_finite()) {
        return Err("El monto debe ser mayor que cero".into());
    }
    if !moneda_valida(&input.moneda) {
        return Err("La moneda debe ser ARS o USD".into());
    }
    Ok(())
}

async fn categoria_txt_de(
    conn: &mut sqlx::PgConnection,
    user_id: i32,
    categoria_id: Option<i32>,
) -> Result<String, String> {
    match categoria_id {
        None => Ok("SIN CATEGORÍA".to_string()),
        Some(id) => {
            let fila: Option<(String,)> =
                sqlx::query_as("SELECT nombre FROM categorias WHERE id = $1 AND user_id = $2")
                    .bind(id)
                    .bind(user_id)
                    .fetch_optional(&mut *conn)
                    .await
                    .map_err(|_| "Error al buscar la categoría".to_string())?;
            fila.map(|(n,)| n)
                .ok_or_else(|| "Categoría no encontrada".to_string())
        }
    }
}

/// Alta de asiento; con `rec` crea la recurrencia y materializa los
/// futuros del año. Devuelve (id del asiento, futuros generados).
pub async fn crear_movimiento(
    conn: &mut sqlx::PgConnection,
    user_id: i32,
    input: &MovIn,
) -> Result<(i32, i64), String> {
    valida_comun(input)?;
    let concepto = input.concepto.trim().to_string();

    let cuenta: Option<(i32,)> =
        sqlx::query_as("SELECT id FROM cuentas WHERE id = $1 AND user_id = $2")
            .bind(input.cuenta_id)
            .bind(user_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|_| "Error al buscar la cuenta".to_string())?;
    if cuenta.is_none() {
        return Err("Cuenta no encontrada".into());
    }
    let categoria_txt = categoria_txt_de(&mut *conn, user_id, input.categoria_id).await?;

    let mut rec_id: Option<i32> = None;
    if input.rec.is_some() {
        let fila: (i32,) = sqlx::query_as(
            "INSERT INTO recurrencias
                (user_id, cuenta_id, categoria_id, categoria_txt, concepto,
                 monto, es_ingreso, moneda, dia, inicio, cuotas, cuotas_generadas)
            VALUES ($1, $2, $3, $4, $5, $6::numeric, $7, $8, $9, $10::date, $11, 1)
            RETURNING id",
        )
        .bind(user_id)
        .bind(input.cuenta_id)
        .bind(input.categoria_id)
        .bind(&categoria_txt)
        .bind(&concepto)
        .bind(input.monto)
        .bind(input.es_ingreso)
        .bind(&input.moneda)
        .bind(dia_de(&input.fecha))
        .bind(&input.fecha)
        .bind(input.rec.as_ref().and_then(|r| r.cuotas))
        .fetch_one(&mut *conn)
        .await
        .map_err(|_| "Error al crear la recurrencia".to_string())?;
        rec_id = Some(fila.0);
    }

    let fila: (i32,) = sqlx::query_as(
        "INSERT INTO movimientos
            (user_id, cuenta_id, categoria_id, categoria_txt, concepto,
             monto, es_ingreso, moneda, fecha, rec_id)
        VALUES ($1, $2, $3, $4, $5, $6::numeric, $7, $8, $9::date, $10)
        RETURNING id",
    )
    .bind(user_id)
    .bind(input.cuenta_id)
    .bind(input.categoria_id)
    .bind(&categoria_txt)
    .bind(&concepto)
    .bind(input.monto)
    .bind(input.es_ingreso)
    .bind(&input.moneda)
    .bind(&input.fecha)
    .bind(rec_id)
    .fetch_one(&mut *conn)
    .await
    .map_err(|_| "Error al crear el asiento".to_string())?;

    let futuros = if let Some(rid) = rec_id {
        materializar(&mut *conn, user_id, &crate::fecha::hoy_iso())
            .await
            .map_err(|_| "Error al materializar la recurrencia".to_string())?;
        let (n,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM movimientos WHERE rec_id = $1 AND id <> $2")
                .bind(rid)
                .bind(fila.0)
                .fetch_one(&mut *conn)
                .await
                .map_err(|_| "Error al contar los futuros".to_string())?;
        n
    } else {
        0
    };
    Ok((fila.0, futuros))
}

/// Edición de asiento; con `rec` regenera los futuros de la recurrencia
/// (borra los programados y los vuelve a materializar con los nuevos
/// datos). Sin `rec`, corta la recurrencia si el asiento tenía una.
pub async fn editar_movimiento(
    conn: &mut sqlx::PgConnection,
    user_id: i32,
    movimiento_id: i32,
    input: &MovIn,
) -> Result<(), String> {
    valida_comun(input)?;
    let concepto = input.concepto.trim().to_string();

    let previo: Option<(Option<i32>,)> =
        sqlx::query_as("SELECT rec_id FROM movimientos WHERE id = $1 AND user_id = $2")
            .bind(movimiento_id)
            .bind(user_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|_| "Error al buscar el asiento".to_string())?;
    let rec_previa = previo
        .map(|(r,)| r)
        .ok_or_else(|| "Asiento no encontrado".to_string())?;

    let cuenta: Option<(i32,)> =
        sqlx::query_as("SELECT id FROM cuentas WHERE id = $1 AND user_id = $2")
            .bind(input.cuenta_id)
            .bind(user_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|_| "Error al buscar la cuenta".to_string())?;
    if cuenta.is_none() {
        return Err("Cuenta no encontrada".into());
    }
    let categoria_txt = categoria_txt_de(&mut *conn, user_id, input.categoria_id).await?;

    match (rec_previa, input.rec.as_ref()) {
        (Some(rid), Some(rec_in)) => {
            // regenerar la recurrencia con los datos nuevos
            sqlx::query(
                "DELETE FROM movimientos WHERE rec_id = $1 AND user_id = $2 AND fecha > $3::date",
            )
            .bind(rid)
            .bind(user_id)
            .bind(crate::fecha::hoy_iso())
            .execute(&mut *conn)
            .await
            .map_err(|_| "Error al cortar los futuros".to_string())?;
            sqlx::query(
                "UPDATE recurrencias SET cuenta_id = $3, categoria_id = $4, categoria_txt = $5,
                    concepto = $6, monto = $7::numeric, es_ingreso = $8, moneda = $9,
                    dia = $10, inicio = $11::date, cuotas = $12, cuotas_generadas = 1, activa = true
                WHERE id = $1 AND user_id = $2",
            )
            .bind(rid)
            .bind(user_id)
            .bind(input.cuenta_id)
            .bind(input.categoria_id)
            .bind(&categoria_txt)
            .bind(&concepto)
            .bind(input.monto)
            .bind(input.es_ingreso)
            .bind(&input.moneda)
            .bind(dia_de(&input.fecha))
            .bind(&input.fecha)
            .bind(rec_in.cuotas)
            .execute(&mut *conn)
            .await
            .map_err(|_| "Error al guardar la recurrencia".to_string())?;
            sqlx::query(
                "UPDATE movimientos SET cuenta_id = $3, categoria_id = $4, categoria_txt = $5,
                    concepto = $6, monto = $7::numeric, es_ingreso = $8, moneda = $9,
                    fecha = $10::date, rec_id = $11
                WHERE id = $1 AND user_id = $2",
            )
            .bind(movimiento_id)
            .bind(user_id)
            .bind(input.cuenta_id)
            .bind(input.categoria_id)
            .bind(&categoria_txt)
            .bind(&concepto)
            .bind(input.monto)
            .bind(input.es_ingreso)
            .bind(&input.moneda)
            .bind(&input.fecha)
            .bind(rid)
            .execute(&mut *conn)
            .await
            .map_err(|_| "Error al guardar el asiento".to_string())?;
            materializar(&mut *conn, user_id, &crate::fecha::hoy_iso())
                .await
                .map_err(|_| "Error al materializar la recurrencia".to_string())?;
        }
        (Some(rid), None) => {
            // cortar la recurrencia: los futuros se borran y la rec cae;
            // los asientos ya cargados pierden el link por el FK SET NULL
            sqlx::query(
                "DELETE FROM movimientos WHERE rec_id = $1 AND user_id = $2 AND fecha > $3::date",
            )
            .bind(rid)
            .bind(user_id)
            .bind(crate::fecha::hoy_iso())
            .execute(&mut *conn)
            .await
            .map_err(|_| "Error al cortar los futuros".to_string())?;
            sqlx::query("DELETE FROM recurrencias WHERE id = $1 AND user_id = $2")
                .bind(rid)
                .bind(user_id)
                .execute(&mut *conn)
                .await
                .map_err(|_| "Error al borrar la recurrencia".to_string())?;
            sqlx::query(
                "UPDATE movimientos SET cuenta_id = $3, categoria_id = $4, categoria_txt = $5,
                    concepto = $6, monto = $7::numeric, es_ingreso = $8, moneda = $9,
                    fecha = $10::date, rec_id = NULL
                WHERE id = $1 AND user_id = $2",
            )
            .bind(movimiento_id)
            .bind(user_id)
            .bind(input.cuenta_id)
            .bind(input.categoria_id)
            .bind(&categoria_txt)
            .bind(&concepto)
            .bind(input.monto)
            .bind(input.es_ingreso)
            .bind(&input.moneda)
            .bind(&input.fecha)
            .execute(&mut *conn)
            .await
            .map_err(|_| "Error al guardar el asiento".to_string())?;
        }
        (None, Some(rec_in)) => {
            // el asiento no tenía recurrencia: crearla y materializar
            let fila: (i32,) = sqlx::query_as(
                "INSERT INTO recurrencias
                    (user_id, cuenta_id, categoria_id, categoria_txt, concepto,
                     monto, es_ingreso, moneda, dia, inicio, cuotas, cuotas_generadas)
                VALUES ($1, $2, $3, $4, $5, $6::numeric, $7, $8, $9, $10::date, $11, 1)
                RETURNING id",
            )
            .bind(user_id)
            .bind(input.cuenta_id)
            .bind(input.categoria_id)
            .bind(&categoria_txt)
            .bind(&concepto)
            .bind(input.monto)
            .bind(input.es_ingreso)
            .bind(&input.moneda)
            .bind(dia_de(&input.fecha))
            .bind(&input.fecha)
            .bind(rec_in.cuotas)
            .fetch_one(&mut *conn)
            .await
            .map_err(|_| "Error al crear la recurrencia".to_string())?;
            sqlx::query(
                "UPDATE movimientos SET cuenta_id = $3, categoria_id = $4, categoria_txt = $5,
                    concepto = $6, monto = $7::numeric, es_ingreso = $8, moneda = $9,
                    fecha = $10::date, rec_id = $11
                WHERE id = $1 AND user_id = $2",
            )
            .bind(movimiento_id)
            .bind(user_id)
            .bind(input.cuenta_id)
            .bind(input.categoria_id)
            .bind(&categoria_txt)
            .bind(&concepto)
            .bind(input.monto)
            .bind(input.es_ingreso)
            .bind(&input.moneda)
            .bind(&input.fecha)
            .bind(fila.0)
            .execute(&mut *conn)
            .await
            .map_err(|_| "Error al guardar el asiento".to_string())?;
            materializar(&mut *conn, user_id, &crate::fecha::hoy_iso())
                .await
                .map_err(|_| "Error al materializar la recurrencia".to_string())?;
        }
        (None, None) => {
            sqlx::query(
                "UPDATE movimientos SET cuenta_id = $3, categoria_id = $4, categoria_txt = $5,
                    concepto = $6, monto = $7::numeric, es_ingreso = $8, moneda = $9,
                    fecha = $10::date
                WHERE id = $1 AND user_id = $2",
            )
            .bind(movimiento_id)
            .bind(user_id)
            .bind(input.cuenta_id)
            .bind(input.categoria_id)
            .bind(&categoria_txt)
            .bind(&concepto)
            .bind(input.monto)
            .bind(input.es_ingreso)
            .bind(&input.moneda)
            .bind(&input.fecha)
            .execute(&mut *conn)
            .await
            .map_err(|_| "Error al guardar el asiento".to_string())?;
        }
    }
    Ok(())
}

/// Borrar solo el asiento (la recurrencia se corta con su endpoint).
pub async fn borrar_movimiento(
    conn: &mut sqlx::PgConnection,
    user_id: i32,
    movimiento_id: i32,
) -> sqlx::Result<u64> {
    let res = sqlx::query("DELETE FROM movimientos WHERE id = $1 AND user_id = $2")
        .bind(movimiento_id)
        .bind(user_id)
        .execute(&mut *conn)
        .await?;
    Ok(res.rows_affected())
}

fn respuesta_err(msg: String) -> HttpResponse {
    if msg.starts_with("Error al") {
        HttpResponse::InternalServerError().body(msg)
    } else {
        HttpResponse::BadRequest().body(msg)
    }
}

#[post("/api/movimientos")]
pub async fn movimiento_crear(
    body: web::Json<MovIn>,
    session: Session,
    pool: web::Data<PgPool>,
) -> impl Responder {
    let Some(user_id) = usuario_de_sesion(&session) else {
        return HttpResponse::Unauthorized().finish();
    };
    let mut conn = match pool.get_ref().acquire().await {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Error de conexión"),
    };
    match crear_movimiento(&mut conn, user_id, &body).await {
        Ok((id, futuros)) => {
            HttpResponse::Created().json(serde_json::json!({ "id": id, "futuros": futuros }))
        }
        Err(msg) => respuesta_err(msg),
    }
}

#[patch("/api/movimientos/{id}")]
pub async fn movimiento_patch(
    path: web::Path<i32>,
    body: web::Json<MovIn>,
    session: Session,
    pool: web::Data<PgPool>,
) -> impl Responder {
    let Some(user_id) = usuario_de_sesion(&session) else {
        return HttpResponse::Unauthorized().finish();
    };
    let mut conn = match pool.get_ref().acquire().await {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Error de conexión"),
    };
    match editar_movimiento(&mut conn, user_id, path.into_inner(), &body).await {
        Ok(()) => HttpResponse::Ok().body("Asiento guardado"),
        Err(msg) => respuesta_err(msg),
    }
}

#[delete("/api/movimientos/{id}")]
pub async fn movimiento_borrar(
    path: web::Path<i32>,
    session: Session,
    pool: web::Data<PgPool>,
) -> impl Responder {
    let Some(user_id) = usuario_de_sesion(&session) else {
        return HttpResponse::Unauthorized().finish();
    };
    let mut conn = match pool.get_ref().acquire().await {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Error de conexión"),
    };
    match borrar_movimiento(&mut conn, user_id, path.into_inner()).await {
        Ok(0) => HttpResponse::NotFound().finish(),
        Ok(_) => HttpResponse::Ok().body("Asiento borrado"),
        Err(_) => HttpResponse::InternalServerError().body("Error al borrar el asiento"),
    }
}
