use actix_session::Session;
use actix_web::{HttpResponse, Responder, get, web};
use askama::Template;
use sqlx::PgPool;

use crate::fecha;
use crate::routes::api::recurrencias::materializar;

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate<'a> {
    email: &'a str,
    corte: String,
    hoy_iso: String,
}

#[get("/")]
async fn index(session: Session, pool: web::Data<PgPool>) -> impl Responder {
    let email = session
        .get::<String>("user_email")
        .unwrap_or(None)
        .unwrap_or_default();

    // El estado financiero vive en la base (cuentas, categorías,
    // movimientos, recurrencias) y el frontend lo carga del API.
    // Antes de servir la página, materializo las recurrencias: cada mes
    // pendiente ya aparece cargado.

    if let Some(user_id) = session.get::<i32>("user_id").unwrap_or(None)
        && let Ok(mut conn) = pool.get_ref().acquire().await
    {
        let _ = materializar(&mut conn, user_id, &fecha::hoy_iso()).await;
    }
    let (hoy_iso, corte) = fecha::hoy();
    let plantilla = IndexTemplate {
        email: &email,
        corte,
        hoy_iso,
    };

    match plantilla.render() {
        Ok(html) => HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(html),
        Err(_) => HttpResponse::InternalServerError().body("Error al renderizar la plantilla"),
    }
}
