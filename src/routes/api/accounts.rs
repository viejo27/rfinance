use actix_session::Session;
use actix_web::{HttpResponse, Responder, delete, patch, post, web};
use serde::Deserialize;
use sqlx::PgPool;

use crate::routes::api::snapshot::session_user_id;

#[derive(Deserialize)]
pub struct AccountInput {
    pub name: String,
    pub currency: String,
    pub opening_balance: Option<f64>,
}

#[derive(Deserialize)]
pub struct AccountPatch {
    pub name: Option<String>,
    pub currency: Option<String>,
    pub opening_balance: Option<f64>,
}

fn is_valid_currency(currency: &str) -> bool {
    currency == "ARS" || currency == "USD"
}

/// Crear cuenta. La lógica pura es testeable: Err con el texto del
/// problema (los que empiezan con "Error al" son fallas de base).
pub async fn create_account(
    conn: &mut sqlx::PgConnection,
    user_id: i32,
    input: &AccountInput,
) -> Result<i32, String> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err("El nombre de la cuenta es obligatorio".into());
    }
    if !is_valid_currency(&input.currency) {
        return Err("La moneda debe ser ARS o USD".into());
    }
    let duplicate: Option<(i32,)> =
        sqlx::query_as("SELECT id FROM accounts WHERE user_id = $1 AND name = $2")
            .bind(user_id)
            .bind(&name)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|_| "Error al verificar el nombre".to_string())?;
    if duplicate.is_some() {
        return Err("Ese nombre de cuenta ya existe".into());
    }
    let row: (i32,) = sqlx::query_as(
        "INSERT INTO accounts (user_id, name, currency, opening_balance)
        VALUES ($1, $2, $3, $4::numeric) RETURNING id",
    )
    .bind(user_id)
    .bind(&name)
    .bind(&input.currency)
    .bind(input.opening_balance.unwrap_or(0.0))
    .fetch_one(&mut *conn)
    .await
    .map_err(|_| "Error al crear la cuenta".to_string())?;
    Ok(row.0)
}

/// Editar cuenta (nombre, moneda, saldo inicial). El nombre de los
/// asientos no necesita sincronizar: el estado lo lee del JOIN.
pub async fn update_account(
    conn: &mut sqlx::PgConnection,
    user_id: i32,
    account_id: i32,
    patch: &AccountPatch,
) -> Result<(), String> {
    if patch
        .currency
        .as_deref()
        .is_some_and(|m| !is_valid_currency(m))
    {
        return Err("La moneda debe ser ARS o USD".into());
    }
    let previous_name: Option<(String,)> =
        sqlx::query_as("SELECT name FROM accounts WHERE id = $1 AND user_id = $2")
            .bind(account_id)
            .bind(user_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|_| "Error al buscar la cuenta".to_string())?;
    let previous_name = previous_name
        .map(|(n,)| n)
        .ok_or_else(|| "Cuenta no encontrada".to_string())?;

    if let Some(new_name) = &patch.name {
        let new_name = new_name.trim();
        if new_name.is_empty() {
            return Err("El nombre de la cuenta es obligatorio".into());
        }
        if new_name != previous_name {
            let dup: Option<(i32,)> = sqlx::query_as(
                "SELECT id FROM accounts WHERE user_id = $1 AND name = $2 AND id <> $3",
            )
            .bind(user_id)
            .bind(new_name)
            .bind(account_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|_| "Error al verificar el nombre".to_string())?;
            if dup.is_some() {
                return Err("Ese nombre de cuenta ya existe".into());
            }
        }
    }

    sqlx::query(
        "UPDATE accounts SET
            name = COALESCE($3::text, name),
            currency = COALESCE($4::text, currency),
            opening_balance = COALESCE($5::numeric, opening_balance)
        WHERE id = $1 AND user_id = $2",
    )
    .bind(account_id)
    .bind(user_id)
    .bind(patch.name.as_deref().map(str::trim))
    .bind(&patch.currency)
    .bind(patch.opening_balance)
    .execute(&mut *conn)
    .await
    .map_err(|_| "Error al guardar la cuenta".to_string())?;
    Ok(())
}

/// Borrar cuenta: los asientos caen por ON DELETE CASCADE.
pub async fn delete_account(
    conn: &mut sqlx::PgConnection,
    user_id: i32,
    account_id: i32,
) -> sqlx::Result<u64> {
    let res = sqlx::query("DELETE FROM accounts WHERE id = $1 AND user_id = $2")
        .bind(account_id)
        .bind(user_id)
        .execute(&mut *conn)
        .await?;
    Ok(res.rows_affected())
}

fn error_response(msg: String) -> HttpResponse {
    if msg.starts_with("Error al") {
        HttpResponse::InternalServerError().body(msg)
    } else {
        HttpResponse::BadRequest().body(msg)
    }
}

#[post("/api/accounts")]
pub async fn account_create(
    body: web::Json<AccountInput>,
    session: Session,
    pool: web::Data<PgPool>,
) -> impl Responder {
    let Some(user_id) = session_user_id(&session) else {
        return HttpResponse::Unauthorized().finish();
    };
    let mut conn = match pool.get_ref().acquire().await {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Error de conexión"),
    };
    match create_account(&mut conn, user_id, &body).await {
        Ok(id) => HttpResponse::Created().json(serde_json::json!({ "id": id })),
        Err(msg) => error_response(msg),
    }
}

#[patch("/api/accounts/{id}")]
pub async fn account_update(
    path: web::Path<i32>,
    body: web::Json<AccountPatch>,
    session: Session,
    pool: web::Data<PgPool>,
) -> impl Responder {
    let Some(user_id) = session_user_id(&session) else {
        return HttpResponse::Unauthorized().finish();
    };
    let mut conn = match pool.get_ref().acquire().await {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Error de conexión"),
    };
    match update_account(&mut conn, user_id, path.into_inner(), &body).await {
        Ok(()) => HttpResponse::Ok().body("Cuenta guardada"),
        Err(msg) => error_response(msg),
    }
}

#[delete("/api/accounts/{id}")]
pub async fn account_delete(
    path: web::Path<i32>,
    session: Session,
    pool: web::Data<PgPool>,
) -> impl Responder {
    let Some(user_id) = session_user_id(&session) else {
        return HttpResponse::Unauthorized().finish();
    };
    let mut conn = match pool.get_ref().acquire().await {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Error de conexión"),
    };
    match delete_account(&mut conn, user_id, path.into_inner()).await {
        Ok(0) => HttpResponse::NotFound().finish(),
        Ok(_) => HttpResponse::Ok().body("Cuenta borrada"),
        Err(_) => HttpResponse::InternalServerError().body("Error al borrar la cuenta"),
    }
}
