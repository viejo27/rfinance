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

/// Alcance de la edición de un asiento que pertenece a una recurrencia.
#[derive(Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Alcance {
    /// Cambiar solo esta fila; la serie y sus futuros quedan intactos.
    Unica,
    /// Cambiar esta fila y re-plantillar las que siguen después de ella.
    Siguientes,
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
    /// Solo lo mira el PATCH de asientos de recurrencia; ausente = Unica.
    #[serde(default)]
    pub alcance: Option<Alcance>,
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

/// Meses civiles completos entre dos ISO YYYY-MM-DD (0 si caen en el mismo mes).
fn meses_entre(desde: &str, hasta: &str) -> i16 {
    let indice = |f: &str| -> i32 {
        let anio: i32 = f[0..4].parse().unwrap_or(0);
        let mes: i32 = f[5..7].parse().unwrap_or(1);
        anio * 12 + mes
    };
    (indice(hasta) - indice(desde)).max(0) as i16
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

/// UPDATE de la fila del asiento, con el rec_id explícito (None = NULL).
async fn actualizar_fila(
    conn: &mut sqlx::PgConnection,
    user_id: i32,
    movimiento_id: i32,
    input: &MovIn,
    concepto: &str,
    categoria_txt: &str,
    rec_id: Option<i32>,
) -> Result<(), String> {
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
    .bind(categoria_txt)
    .bind(concepto)
    .bind(input.monto)
    .bind(input.es_ingreso)
    .bind(&input.moneda)
    .bind(&input.fecha)
    .bind(rec_id)
    .execute(&mut *conn)
    .await
    .map_err(|_| "Error al guardar el asiento".to_string())?;
    Ok(())
}

/// Edición de asiento. Cuando la fila pertenece a una recurrencia decide
/// `alcance`: `Unica` (default si no viene) cambia solo esta fila sin tocar
/// la serie; `Siguientes` borra las posteriores a ESTA fila (en función de su
/// fecha, nunca de "hoy"), re-plantilla la recurrencia desde aquí y vuelve a
/// materializar los futuros. Con `Siguientes` y sin `rec`, la serie termina
/// acá. Para filas sin recurrencia se ignora `alcance`; con `rec` arranca
/// una serie nueva desde esta fila. Devuelve cuántos asientos futuros quedó
/// generando la serie (0 si no aplica).
pub async fn editar_movimiento(
    conn: &mut sqlx::PgConnection,
    user_id: i32,
    movimiento_id: i32,
    input: &MovIn,
) -> Result<i64, String> {
    valida_comun(input)?;
    let concepto = input.concepto.trim().to_string();

    let previo: Option<(Option<i32>, String)> = sqlx::query_as(
        "SELECT rec_id, to_char(fecha, 'YYYY-MM-DD')
        FROM movimientos WHERE id = $1 AND user_id = $2",
    )
    .bind(movimiento_id)
    .bind(user_id)
    .fetch_optional(&mut *conn)
    .await
    .map_err(|_| "Error al buscar el asiento".to_string())?;
    let (rec_previa, fecha_original) = previo.ok_or_else(|| "Asiento no encontrado".to_string())?;

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

    let Some(rid) = rec_previa else {
        let Some(rec_in) = input.rec.as_ref() else {
            // asiento simple editado como simple
            actualizar_fila(
                conn,
                user_id,
                movimiento_id,
                input,
                &concepto,
                &categoria_txt,
                None,
            )
            .await?;
            return Ok(0);
        };
        // el asiento no tenía recurrencia: crearla desde esta fila y materializar
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
        actualizar_fila(
            conn,
            user_id,
            movimiento_id,
            input,
            &concepto,
            &categoria_txt,
            Some(fila.0),
        )
        .await?;
        materializar(&mut *conn, user_id, &crate::fecha::hoy_iso())
            .await
            .map_err(|_| "Error al materializar la recurrencia".to_string())?;
        let (n,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM movimientos WHERE rec_id = $1 AND user_id = $2 AND id <> $3",
        )
        .bind(fila.0)
        .bind(user_id)
        .bind(movimiento_id)
        .fetch_one(&mut *conn)
        .await
        .map_err(|_| "Error al contar los futuros".to_string())?;
        return Ok(n);
    };

    if input.alcance != Some(Alcance::Siguientes) {
        // Unica: solo esta fila. La serie, sus futuros y las filas anteriores
        // quedan intactas (era el bug: antes se cortaba todo "desde hoy").
        actualizar_fila(
            conn,
            user_id,
            movimiento_id,
            input,
            &concepto,
            &categoria_txt,
            Some(rid),
        )
        .await?;
        return Ok(0);
    }

    // Siguientes: cortar lo estrictamente posterior a la fecha original de
    // esta fila, nunca relativo a hoy, para no borrar programados anteriores.
    sqlx::query(
        "DELETE FROM movimientos
        WHERE rec_id = $1 AND user_id = $2 AND id <> $3 AND fecha > $4::date",
    )
    .bind(rid)
    .bind(user_id)
    .bind(movimiento_id)
    .bind(&fecha_original)
    .execute(&mut *conn)
    .await
    .map_err(|_| "Error al cortar los futuros".to_string())?;

    let Some(rec_in) = input.rec.as_ref() else {
        // la serie termina acá: cae la recurrencia y las filas previas
        // pierden el link por el FK SET NULL
        actualizar_fila(
            conn,
            user_id,
            movimiento_id,
            input,
            &concepto,
            &categoria_txt,
            None,
        )
        .await?;
        sqlx::query("DELETE FROM recurrencias WHERE id = $1 AND user_id = $2")
            .bind(rid)
            .bind(user_id)
            .execute(&mut *conn)
            .await
            .map_err(|_| "Error al borrar la recurrencia".to_string())?;
        return Ok(0);
    };

    // re-plantillar desde esta fila: el CUOTAS del form es el total de la serie
    // original, así que desde esta cuota en adelante quedan total - transcurridas
    let plantilla: Option<(String,)> = sqlx::query_as(
        "SELECT to_char(inicio, 'YYYY-MM-DD')
        FROM recurrencias WHERE id = $1 AND user_id = $2",
    )
    .bind(rid)
    .bind(user_id)
    .fetch_optional(&mut *conn)
    .await
    .map_err(|_| "Error al buscar la recurrencia".to_string())?;
    let (inicio,) = plantilla.ok_or_else(|| "Recurrencia no encontrada".to_string())?;
    let nuevas_cuotas = rec_in
        .cuotas
        .map(|c| (c - meses_entre(&inicio, &fecha_original)).max(1));

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
    .bind(nuevas_cuotas)
    .execute(&mut *conn)
    .await
    .map_err(|_| "Error al guardar la recurrencia".to_string())?;
    actualizar_fila(
        conn,
        user_id,
        movimiento_id,
        input,
        &concepto,
        &categoria_txt,
        Some(rid),
    )
    .await?;
    materializar(&mut *conn, user_id, &crate::fecha::hoy_iso())
        .await
        .map_err(|_| "Error al materializar la recurrencia".to_string())?;
    let (n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM movimientos
        WHERE rec_id = $1 AND user_id = $2 AND id <> $3 AND fecha > $4::date",
    )
    .bind(rid)
    .bind(user_id)
    .bind(movimiento_id)
    .bind(&fecha_original)
    .fetch_one(&mut *conn)
    .await
    .map_err(|_| "Error al contar los futuros".to_string())?;
    Ok(n)
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
    let id = path.into_inner();
    let mut conn = match pool.get_ref().acquire().await {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Error de conexión"),
    };
    match editar_movimiento(&mut conn, user_id, id, &body).await {
        // mismo shape que el POST: el frontend resalta data.id y avisa data.futuros
        Ok(futuros) => HttpResponse::Ok().json(serde_json::json!({ "id": id, "futuros": futuros })),
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

#[cfg(test)]
mod tests {
    use super::{Alcance, MovIn, RecIn, editar_movimiento};
    use crate::routes::api::recurrencias::ultimo_dia;
    use sqlx::Connection;
    use sqlx::PgConnection;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Mismo esquema que los tests de recurrencias: una transacción que se
    /// revierte, porque el rol de .env no tiene CREATEDB.
    async fn conectar() -> PgConnection {
        dotenvy::dotenv().ok();
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL para los tests");
        PgConnection::connect(&url).await.expect("conexión de test")
    }

    async fn usuario(tx: &mut PgConnection) -> i32 {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as u64 + d.as_secs())
            .unwrap_or(1);
        let email = format!("test-edit-{nanos}@rfinance.local");
        let (id,): (i32,) =
            sqlx::query_as("INSERT INTO users (email, password) VALUES ($1, 'x') RETURNING id")
                .bind(&email)
                .fetch_one(&mut *tx)
                .await
                .expect("user de test");
        id
    }

    /// Fecha del mes k (0 = el de `desde`) respetando el día y su recorte.
    fn fecha_mes(desde: &str, k: i32, dia: i16) -> String {
        let indice =
            desde[0..4].parse::<i32>().unwrap() * 12 + desde[5..7].parse::<i32>().unwrap() - 1 + k;
        let (anio, mes) = (indice / 12, indice % 12 + 1);
        let dia = i64::from(dia).min(ultimo_dia(i64::from(anio), i64::from(mes)));
        format!("{anio:04}-{mes:02}-{dia:02}")
    }

    /// Siembra una serie de `filas` asientos mensuales starting en `inicio`,
    /// todos a 320000. Devuelve (rec_id, ids en orden cronológico, cuenta_id).
    async fn sembrar_serie(
        tx: &mut PgConnection,
        user_id: i32,
        inicio: &str,
        cuotas: Option<i16>,
        filas: i32,
    ) -> (i32, Vec<i32>, i32) {
        let (cid,): (i32,) = sqlx::query_as(
            "INSERT INTO cuentas (user_id, nombre, moneda) VALUES ($1, 'Banco', 'ARS') RETURNING id",
        )
        .bind(user_id)
        .fetch_one(&mut *tx)
        .await
        .expect("cuenta de test");
        let (rid,): (i32,) = sqlx::query_as(
            "INSERT INTO recurrencias
                (user_id, cuenta_id, categoria_id, categoria_txt, concepto, monto,
                 es_ingreso, moneda, dia, inicio, cuotas, cuotas_generadas)
            VALUES ($1, $2, NULL, 'Alquiler', 'Alquiler', 320000, false, 'ARS', 12, $3::date, $4, $5)
            RETURNING id",
        )
        .bind(user_id)
        .bind(cid)
        .bind(inicio)
        .bind(cuotas)
        .bind(i16::try_from(filas).unwrap())
        .fetch_one(&mut *tx)
        .await
        .expect("recurrencia de test");
        let mut ids = Vec::new();
        for k in 0..filas {
            let fecha = fecha_mes(inicio, k, 12);
            let (id,): (i32,) = sqlx::query_as(
                "INSERT INTO movimientos
                    (user_id, cuenta_id, categoria_id, categoria_txt, concepto,
                     monto, es_ingreso, moneda, fecha, rec_id)
                VALUES ($1, $2, NULL, 'Alquiler', 'Alquiler', 320000::numeric, false, 'ARS', $3::date, $4)
                RETURNING id",
            )
            .bind(user_id)
            .bind(cid)
            .bind(&fecha)
            .bind(rid)
            .fetch_one(&mut *tx)
            .await
            .expect("asiento de test");
            ids.push(id);
        }
        (rid, ids, cid)
    }

    fn entrada(cuenta_id: i32, fecha: &str, monto: f64, concepto: &str) -> MovIn {
        MovIn {
            fecha: fecha.into(),
            cuenta_id,
            categoria_id: None,
            concepto: concepto.into(),
            monto,
            es_ingreso: false,
            moneda: "ARS".into(),
            rec: None,
            alcance: None,
        }
    }

    async fn montos_por_fecha(tx: &mut PgConnection, user_id: i32) -> Vec<(String, f64, bool)> {
        sqlx::query_as(
            "SELECT to_char(fecha, 'YYYY-MM-DD'), monto::float8, rec_id IS NOT NULL
            FROM movimientos WHERE user_id = $1 ORDER BY fecha",
        )
        .bind(user_id)
        .fetch_all(&mut *tx)
        .await
        .expect("montos")
    }

    async fn estado_rec(tx: &mut PgConnection, rid: i32) -> (String, Option<i16>, i16) {
        sqlx::query_as(
            "SELECT to_char(inicio, 'YYYY-MM-DD'), cuotas, cuotas_generadas
            FROM recurrencias WHERE id = $1",
        )
        .bind(rid)
        .fetch_one(&mut *tx)
        .await
        .expect("recurrencia")
    }

    // El bug original: editar un asiento de la serie con alcance Unica (o sin
    // alcance, clientes viejos) no debe borrar ni tocar ningún otro.
    #[async_std::test]
    async fn unica_deja_la_serie_intacta() {
        let mut conn = conectar().await;
        let mut tx = conn.begin().await.expect("tx");
        let user = usuario(&mut tx).await;
        let (rid, ids, cid) = sembrar_serie(&mut tx, user, "2026-10-12", Some(4), 4).await;

        let mut e = entrada(cid, "2026-12-12", 500.0, "Alquiler DIC");
        e.alcance = Some(Alcance::Unica);
        let futuros = editar_movimiento(&mut tx, user, ids[2], &e)
            .await
            .expect("editar");
        assert_eq!(futuros, 0);

        let filas = montos_por_fecha(&mut tx, user).await;
        assert_eq!(filas.len(), 4, "no debe borrar ni crear asientos");
        assert_eq!(filas[0].1, 320000.0);
        assert_eq!(filas[1].1, 320000.0);
        assert_eq!(filas[2].1, 500.0);
        assert_eq!(filas[3].1, 320000.0);
        assert!(
            filas.iter().all(|(_, _, ligada)| *ligada),
            "la serie sigue ligada"
        );
        assert_eq!(
            estado_rec(&mut tx, rid).await,
            ("2026-10-12".into(), Some(4), 4)
        );

        tx.rollback().await.expect("rollback");
    }

    // Clientes sin el campo alcance: default seguro (Unica), nada desaparece.
    #[async_std::test]
    async fn sin_alcance_es_unica_por_defecto() {
        let mut conn = conectar().await;
        let mut tx = conn.begin().await.expect("tx");
        let user = usuario(&mut tx).await;
        let (rid, ids, cid) = sembrar_serie(&mut tx, user, "2026-10-12", Some(4), 4).await;

        let e = entrada(cid, "2026-11-12", 500.0, "Alquiler NOV");
        editar_movimiento(&mut tx, user, ids[1], &e)
            .await
            .expect("editar");

        let filas = montos_por_fecha(&mut tx, user).await;
        assert_eq!(filas.len(), 4);
        assert_eq!(
            estado_rec(&mut tx, rid).await,
            ("2026-10-12".into(), Some(4), 4)
        );

        tx.rollback().await.expect("rollback");
    }

    // "Este y los siguientes": re-plantilla desde la fila editada; lo anterior
    // no se toca; las cuotas totales descuentan lo ya transcurrido.
    #[async_std::test]
    async fn siguientes_replanta_desde_la_fila() {
        let mut conn = conectar().await;
        let mut tx = conn.begin().await.expect("tx");
        let user = usuario(&mut tx).await;
        let (rid, ids, cid) = sembrar_serie(&mut tx, user, "2026-10-12", Some(4), 4).await;

        let mut e = entrada(cid, "2026-11-12", 500.0, "Alquiler");
        e.alcance = Some(Alcance::Siguientes);
        e.rec = Some(RecIn { cuotas: Some(4) });
        let futuros = editar_movimiento(&mut tx, user, ids[1], &e)
            .await
            .expect("editar");
        assert_eq!(futuros, 2, "DIC y ENE regenerados");

        let filas = montos_por_fecha(&mut tx, user).await;
        assert_eq!(filas.len(), 4, "OCT + NOV + DIC + ENE");
        assert_eq!(filas[0], ("2026-10-12".into(), 320000.0, true));
        for f in &filas[1..] {
            assert_eq!(f.1, 500.0);
            assert!(f.2);
        }
        assert_eq!(filas[1].0, "2026-11-12");
        assert_eq!(filas[2].0, "2026-12-12");
        assert_eq!(filas[3].0, "2027-01-12");
        // 4 totales desde el inicio original: 1 ya pasada (OCT), quedan 3 desde NOV
        assert_eq!(
            estado_rec(&mut tx, rid).await,
            ("2026-11-12".into(), Some(3), 3)
        );

        tx.rollback().await.expect("rollback");
    }

    // "Este y los siguientes" sin rec en el body: la serie termina acá.
    #[async_std::test]
    async fn siguientes_sin_rec_corta_la_serie() {
        let mut conn = conectar().await;
        let mut tx = conn.begin().await.expect("tx");
        let user = usuario(&mut tx).await;
        let (_rid, ids, cid) = sembrar_serie(&mut tx, user, "2026-10-12", Some(4), 4).await;

        let mut e = entrada(cid, "2026-11-12", 500.0, "Alquiler final");
        e.alcance = Some(Alcance::Siguientes);
        let futuros = editar_movimiento(&mut tx, user, ids[1], &e)
            .await
            .expect("editar");
        assert_eq!(futuros, 0);

        let filas = montos_por_fecha(&mut tx, user).await;
        assert_eq!(filas.len(), 2, "solo OCT y NOV, el resto se cortó");
        assert!(
            filas.iter().all(|(_, _, ligada)| !*ligada),
            "la serie cayó, quedan desligadas"
        );
        assert_eq!(filas[1].1, 500.0);

        tx.rollback().await.expect("rollback");
    }

    // Asiento simple (sin recurrencia): se edita la fila nomás.
    #[async_std::test]
    async fn simple_se_edita_en_paz() {
        let mut conn = conectar().await;
        let mut tx = conn.begin().await.expect("tx");
        let user = usuario(&mut tx).await;
        let (cid,): (i32,) = sqlx::query_as(
            "INSERT INTO cuentas (user_id, nombre, moneda) VALUES ($1, 'Banco', 'ARS') RETURNING id",
        )
        .bind(user)
        .fetch_one(&mut *tx)
        .await
        .expect("cuenta");
        let (mid,): (i32,) = sqlx::query_as(
            "INSERT INTO movimientos
                (user_id, cuenta_id, categoria_id, categoria_txt, concepto,
                 monto, es_ingreso, moneda, fecha, rec_id)
            VALUES ($1, $2, NULL, 'SIN CATEGORÍA', 'Café', 1000::numeric, false, 'ARS', '2026-11-03', NULL)
            RETURNING id",
        )
        .bind(user)
        .bind(cid)
        .fetch_one(&mut *tx)
        .await
        .expect("asiento");

        let e = entrada(cid, "2026-11-03", 1500.0, "Café con leche");
        let futuros = editar_movimiento(&mut tx, user, mid, &e)
            .await
            .expect("editar");
        assert_eq!(futuros, 0);
        let filas = montos_por_fecha(&mut tx, user).await;
        assert_eq!(filas, vec![("2026-11-03".to_string(), 1500.0, false)]);

        tx.rollback().await.expect("rollback");
    }
}
