use actix_session::Session;
use actix_web::{HttpResponse, Responder, delete, patch, post, web};
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::routes::api::snapshot::session_user_id;

/// Payload del form de alta/edición: el front siempre manda los tres
/// campos completos (reemplazo total), así el límite puede volver a null.
#[derive(Deserialize)]
pub struct CategoryInput {
    pub name: String,
    pub budget_limit: Option<f64>,
    pub currency: String,
}

fn is_valid_currency(currency: &str) -> bool {
    currency == "ARS" || currency == "USD"
}

pub async fn create_category(
    conn: &mut sqlx::PgConnection,
    user_id: i32,
    input: &CategoryInput,
) -> Result<Uuid, String> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err("El nombre de la categoría es obligatorio".into());
    }
    if !is_valid_currency(&input.currency) {
        return Err("La moneda debe ser ARS o USD".into());
    }
    let duplicate: Option<(i32,)> =
        sqlx::query_as("SELECT id FROM categories WHERE user_id = $1 AND name = $2")
            .bind(user_id)
            .bind(&name)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|_| "Error al verificar el nombre".to_string())?;
    if duplicate.is_some() {
        return Err("Esa categoría ya existe".into());
    }
    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO categories (user_id, name, budget_limit, currency)
        VALUES ($1, $2, $3::numeric, $4) RETURNING public_id",
    )
    .bind(user_id)
    .bind(&name)
    .bind(input.budget_limit)
    .bind(&input.currency)
    .fetch_one(&mut *conn)
    .await
    .map_err(|_| "Error al crear la categoría".to_string())?;
    Ok(row.0)
}

/// Reemplazo completo de la categoría; el snapshot de texto de los
/// asientos sincroniza al nuevo nombre.
pub async fn update_category(
    conn: &mut sqlx::PgConnection,
    user_id: i32,
    public_id: Uuid,
    input: &CategoryInput,
) -> Result<(), String> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err("El nombre de la categoría es obligatorio".into());
    }
    if !is_valid_currency(&input.currency) {
        return Err("La moneda debe ser ARS o USD".into());
    }
    let previous: Option<(i32, String)> =
        sqlx::query_as("SELECT id, name FROM categories WHERE public_id = $1 AND user_id = $2")
            .bind(public_id)
            .bind(user_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|_| "Error al buscar la categoría".to_string())?;
    let (category_id, previous_name) =
        previous.ok_or_else(|| "Categoría no encontrada".to_string())?;
    if name != previous_name {
        let dup: Option<(i32,)> = sqlx::query_as(
            "SELECT id FROM categories WHERE user_id = $1 AND name = $2 AND id <> $3",
        )
        .bind(user_id)
        .bind(&name)
        .bind(category_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|_| "Error al verificar el nombre".to_string())?;
        if dup.is_some() {
            return Err("Esa categoría ya existe".into());
        }
    }
    sqlx::query(
        "UPDATE categories SET name = $3::text, budget_limit = $4::numeric, currency = $5
        WHERE id = $1 AND user_id = $2",
    )
    .bind(category_id)
    .bind(user_id)
    .bind(&name)
    .bind(input.budget_limit)
    .bind(&input.currency)
    .execute(&mut *conn)
    .await
    .map_err(|_| "Error al guardar la categoría".to_string())?;
    if name != previous_name {
        sqlx::query(
            "UPDATE transactions SET category_label = $3 WHERE category_id = $1 AND user_id = $2",
        )
        .bind(category_id)
        .bind(user_id)
        .bind(&name)
        .execute(&mut *conn)
        .await
        .map_err(|_| "Error al sincronizar asientos".to_string())?;
    }
    Ok(())
}

/// Borrar categoría: los asientos quedan con category_id NULL y su
/// snapshot de texto (el FK con SET NULL lo hace).
pub async fn delete_category(
    conn: &mut sqlx::PgConnection,
    user_id: i32,
    public_id: Uuid,
) -> sqlx::Result<u64> {
    let res = sqlx::query("DELETE FROM categories WHERE public_id = $1 AND user_id = $2")
        .bind(public_id)
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

#[post("/api/categories")]
pub async fn category_create(
    body: web::Json<CategoryInput>,
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
    match create_category(&mut conn, user_id, &body).await {
        Ok(id) => HttpResponse::Created().json(serde_json::json!({ "id": id })),
        Err(msg) => error_response(msg),
    }
}

#[patch("/api/categories/{id}")]
pub async fn category_update(
    path: web::Path<Uuid>,
    body: web::Json<CategoryInput>,
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
    match update_category(&mut conn, user_id, path.into_inner(), &body).await {
        Ok(()) => HttpResponse::Ok().body("Categoría guardada"),
        Err(msg) => error_response(msg),
    }
}

#[delete("/api/categories/{id}")]
pub async fn category_delete(
    path: web::Path<Uuid>,
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
    match delete_category(&mut conn, user_id, path.into_inner()).await {
        Ok(0) => HttpResponse::NotFound().finish(),
        Ok(_) => HttpResponse::Ok().body("Categoría borrada"),
        Err(_) => HttpResponse::InternalServerError().body("Error al borrar la categoría"),
    }
}
