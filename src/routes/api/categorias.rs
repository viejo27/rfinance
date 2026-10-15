use actix_session::Session;
use actix_web::{HttpResponse, Responder, delete, patch, post, web};
use serde::Deserialize;
use sqlx::PgPool;

use crate::routes::api::estado::usuario_de_sesion;

/// Payload del form de alta/edición: el front siempre manda los tres
/// campos completos (reemplazo total), así el límite puede volver a null.
#[derive(Deserialize)]
pub struct CategoriaIn {
    pub nombre: String,
    pub limite: Option<f64>,
    pub moneda: String,
}

fn moneda_valida(moneda: &str) -> bool {
    moneda == "ARS" || moneda == "USD"
}

pub async fn crear_categoria(
    conn: &mut sqlx::PgConnection,
    user_id: i32,
    input: &CategoriaIn,
) -> Result<i32, String> {
    let nombre = input.nombre.trim().to_string();
    if nombre.is_empty() {
        return Err("El nombre de la categoría es obligatorio".into());
    }
    if !moneda_valida(&input.moneda) {
        return Err("La moneda debe ser ARS o USD".into());
    }
    let duplicada: Option<(i32,)> =
        sqlx::query_as("SELECT id FROM categorias WHERE user_id = $1 AND nombre = $2")
            .bind(user_id)
            .bind(&nombre)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|_| "Error al verificar el nombre".to_string())?;
    if duplicada.is_some() {
        return Err("Esa categoría ya existe".into());
    }
    let fila: (i32,) = sqlx::query_as(
        "INSERT INTO categorias (user_id, nombre, limite, moneda)
        VALUES ($1, $2, $3::numeric, $4) RETURNING id",
    )
    .bind(user_id)
    .bind(&nombre)
    .bind(input.limite)
    .bind(&input.moneda)
    .fetch_one(&mut *conn)
    .await
    .map_err(|_| "Error al crear la categoría".to_string())?;
    Ok(fila.0)
}

/// Reemplazo completo de la categoría; el snapshot de texto de los
/// asientos sincroniza al nuevo nombre.
pub async fn editar_categoria(
    conn: &mut sqlx::PgConnection,
    user_id: i32,
    categoria_id: i32,
    input: &CategoriaIn,
) -> Result<(), String> {
    let nombre = input.nombre.trim().to_string();
    if nombre.is_empty() {
        return Err("El nombre de la categoría es obligatorio".into());
    }
    if !moneda_valida(&input.moneda) {
        return Err("La moneda debe ser ARS o USD".into());
    }
    let previo: Option<(String,)> =
        sqlx::query_as("SELECT nombre FROM categorias WHERE id = $1 AND user_id = $2")
            .bind(categoria_id)
            .bind(user_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|_| "Error al buscar la categoría".to_string())?;
    let previo = previo
        .map(|(n,)| n)
        .ok_or_else(|| "Categoría no encontrada".to_string())?;
    if nombre != previo {
        let dup: Option<(i32,)> = sqlx::query_as(
            "SELECT id FROM categorias WHERE user_id = $1 AND nombre = $2 AND id <> $3",
        )
        .bind(user_id)
        .bind(&nombre)
        .bind(categoria_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|_| "Error al verificar el nombre".to_string())?;
        if dup.is_some() {
            return Err("Esa categoría ya existe".into());
        }
    }
    sqlx::query(
        "UPDATE categorias SET nombre = $3::text, limite = $4::numeric, moneda = $5
        WHERE id = $1 AND user_id = $2",
    )
    .bind(categoria_id)
    .bind(user_id)
    .bind(&nombre)
    .bind(input.limite)
    .bind(&input.moneda)
    .execute(&mut *conn)
    .await
    .map_err(|_| "Error al guardar la categoría".to_string())?;
    if nombre != previo {
        sqlx::query(
            "UPDATE movimientos SET categoria_txt = $3 WHERE categoria_id = $1 AND user_id = $2",
        )
        .bind(categoria_id)
        .bind(user_id)
        .bind(&nombre)
        .execute(&mut *conn)
        .await
        .map_err(|_| "Error al sincronizar asientos".to_string())?;
    }
    Ok(())
}

/// Borrar categoría: los asientos quedan con categoria_id NULL y su
/// snapshot de texto (el FK con SET NULL lo hace).
pub async fn borrar_categoria(
    conn: &mut sqlx::PgConnection,
    user_id: i32,
    categoria_id: i32,
) -> sqlx::Result<u64> {
    let res = sqlx::query("DELETE FROM categorias WHERE id = $1 AND user_id = $2")
        .bind(categoria_id)
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

#[post("/api/categorias")]
pub async fn categoria_crear(
    body: web::Json<CategoriaIn>,
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
    match crear_categoria(&mut conn, user_id, &body).await {
        Ok(id) => HttpResponse::Created().json(serde_json::json!({ "id": id })),
        Err(msg) => respuesta_err(msg),
    }
}

#[patch("/api/categorias/{id}")]
pub async fn categoria_patch(
    path: web::Path<i32>,
    body: web::Json<CategoriaIn>,
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
    match editar_categoria(&mut conn, user_id, path.into_inner(), &body).await {
        Ok(()) => HttpResponse::Ok().body("Categoría guardada"),
        Err(msg) => respuesta_err(msg),
    }
}

#[delete("/api/categorias/{id}")]
pub async fn categoria_borrar(
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
    match borrar_categoria(&mut conn, user_id, path.into_inner()).await {
        Ok(0) => HttpResponse::NotFound().finish(),
        Ok(_) => HttpResponse::Ok().body("Categoría borrada"),
        Err(_) => HttpResponse::InternalServerError().body("Error al borrar la categoría"),
    }
}
