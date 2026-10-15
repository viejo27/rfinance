use actix_session::Session;
use actix_web::{HttpResponse, Responder, delete, patch, post, web};
use serde::Deserialize;
use sqlx::PgPool;

use crate::routes::api::estado::usuario_de_sesion;

#[derive(Deserialize)]
pub struct CuentaIn {
    pub nombre: String,
    pub moneda: String,
    pub saldo_inicial: Option<f64>,
}

#[derive(Deserialize)]
pub struct CuentaPatch {
    pub nombre: Option<String>,
    pub moneda: Option<String>,
    pub saldo_inicial: Option<f64>,
}

fn moneda_valida(moneda: &str) -> bool {
    moneda == "ARS" || moneda == "USD"
}

/// Crear cuenta. La lógica pura es testeable: Err con el texto del
/// problema (los que empiezan con "Error al" son fallas de base).
pub async fn crear_cuenta(
    conn: &mut sqlx::PgConnection,
    user_id: i32,
    input: &CuentaIn,
) -> Result<i32, String> {
    let nombre = input.nombre.trim().to_string();
    if nombre.is_empty() {
        return Err("El nombre de la cuenta es obligatorio".into());
    }
    if !moneda_valida(&input.moneda) {
        return Err("La moneda debe ser ARS o USD".into());
    }
    let duplicada: Option<(i32,)> =
        sqlx::query_as("SELECT id FROM cuentas WHERE user_id = $1 AND nombre = $2")
            .bind(user_id)
            .bind(&nombre)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|_| "Error al verificar el nombre".to_string())?;
    if duplicada.is_some() {
        return Err("Ese nombre de cuenta ya existe".into());
    }
    let fila: (i32,) = sqlx::query_as(
        "INSERT INTO cuentas (user_id, nombre, moneda, saldo_inicial)
        VALUES ($1, $2, $3, $4::numeric) RETURNING id",
    )
    .bind(user_id)
    .bind(&nombre)
    .bind(&input.moneda)
    .bind(input.saldo_inicial.unwrap_or(0.0))
    .fetch_one(&mut *conn)
    .await
    .map_err(|_| "Error al crear la cuenta".to_string())?;
    Ok(fila.0)
}

/// Editar cuenta (nombre, moneda, saldo inicial). El nombre de los
/// asientos no necesita sincronizar: el estado lo lee del JOIN.
pub async fn editar_cuenta(
    conn: &mut sqlx::PgConnection,
    user_id: i32,
    cuenta_id: i32,
    patch: &CuentaPatch,
) -> Result<(), String> {
    if patch.moneda.as_deref().is_some_and(|m| !moneda_valida(m)) {
        return Err("La moneda debe ser ARS o USD".into());
    }
    let nombre_previo: Option<(String,)> =
        sqlx::query_as("SELECT nombre FROM cuentas WHERE id = $1 AND user_id = $2")
            .bind(cuenta_id)
            .bind(user_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|_| "Error al buscar la cuenta".to_string())?;
    let nombre_previo = nombre_previo
        .map(|(n,)| n)
        .ok_or_else(|| "Cuenta no encontrada".to_string())?;

    if let Some(nuevo) = &patch.nombre {
        let nuevo = nuevo.trim();
        if nuevo.is_empty() {
            return Err("El nombre de la cuenta es obligatorio".into());
        }
        if nuevo != nombre_previo {
            let dup: Option<(i32,)> = sqlx::query_as(
                "SELECT id FROM cuentas WHERE user_id = $1 AND nombre = $2 AND id <> $3",
            )
            .bind(user_id)
            .bind(nuevo)
            .bind(cuenta_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|_| "Error al verificar el nombre".to_string())?;
            if dup.is_some() {
                return Err("Ese nombre de cuenta ya existe".into());
            }
        }
    }

    sqlx::query(
        "UPDATE cuentas SET
            nombre = COALESCE($3::text, nombre),
            moneda = COALESCE($4::text, moneda),
            saldo_inicial = COALESCE($5::numeric, saldo_inicial)
        WHERE id = $1 AND user_id = $2",
    )
    .bind(cuenta_id)
    .bind(user_id)
    .bind(patch.nombre.as_deref().map(str::trim))
    .bind(&patch.moneda)
    .bind(patch.saldo_inicial)
    .execute(&mut *conn)
    .await
    .map_err(|_| "Error al guardar la cuenta".to_string())?;
    Ok(())
}

/// Borrar cuenta: los asientos caen por ON DELETE CASCADE.
pub async fn borrar_cuenta(
    conn: &mut sqlx::PgConnection,
    user_id: i32,
    cuenta_id: i32,
) -> sqlx::Result<u64> {
    let res = sqlx::query("DELETE FROM cuentas WHERE id = $1 AND user_id = $2")
        .bind(cuenta_id)
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

#[post("/api/cuentas")]
pub async fn cuenta_crear(
    body: web::Json<CuentaIn>,
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
    match crear_cuenta(&mut conn, user_id, &body).await {
        Ok(id) => HttpResponse::Created().json(serde_json::json!({ "id": id })),
        Err(msg) => respuesta_err(msg),
    }
}

#[patch("/api/cuentas/{id}")]
pub async fn cuenta_patch(
    path: web::Path<i32>,
    body: web::Json<CuentaPatch>,
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
    match editar_cuenta(&mut conn, user_id, path.into_inner(), &body).await {
        Ok(()) => HttpResponse::Ok().body("Cuenta guardada"),
        Err(msg) => respuesta_err(msg),
    }
}

#[delete("/api/cuentas/{id}")]
pub async fn cuenta_borrar(
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
    match borrar_cuenta(&mut conn, user_id, path.into_inner()).await {
        Ok(0) => HttpResponse::NotFound().finish(),
        Ok(_) => HttpResponse::Ok().body("Cuenta borrada"),
        Err(_) => HttpResponse::InternalServerError().body("Error al borrar la cuenta"),
    }
}
