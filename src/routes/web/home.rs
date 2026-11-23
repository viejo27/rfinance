use actix_session::Session;
use actix_web::{HttpResponse, Responder, get, web};
use askama::Template;
use sqlx::PgPool;

use crate::date;
use crate::mail::Mailer;
use crate::routes::api::recurrences::materialize;

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate<'a> {
    email: &'a str,
    cutoff: String,
    today_iso: String,
    email_verified: bool,
}

#[get("/")]
async fn index(
    session: Session,
    pool: web::Data<PgPool>,
    mailer: web::Data<Mailer>,
) -> impl Responder {
    let email = session
        .get::<String>("user_email")
        .unwrap_or(None)
        .unwrap_or_default();
    let user_id = session.get::<i32>("user_id").unwrap_or(None);

    // El estado financiero vive en la base (cuentas, categorías,
    // movimientos, recurrencias) y el frontend lo carga del API.
    // Antes de servir la página, materializo las recurrencias: cada mes
    // pendiente ya aparece cargado.

    if let Some(user_id) = user_id
        && let Ok(mut conn) = pool.get_ref().acquire().await
    {
        let _ = materialize(&mut conn, user_id, &date::today_iso()).await;
    }

    // Si la consulta falla, no muestro el aviso: el banner es una ayuda,
    // no un bloqueo, y no quiero un falso "sin verificar" por un error.
    // Con el correo deshabilitado tampoco hay forma de verificar: silencio.
    let email_verified = if !mailer.is_enabled() {
        true
    } else {
        match user_id {
            Some(user_id) => sqlx::query_as::<_, (bool,)>(
                "SELECT email_verified_at IS NOT NULL FROM users WHERE id = $1",
            )
            .bind(user_id)
            .fetch_optional(pool.get_ref())
            .await
            .ok()
            .flatten()
            .map(|(verified,)| verified)
            .unwrap_or(true),
            None => true,
        }
    };

    let (today_iso, cutoff) = date::today();
    let template = IndexTemplate {
        email: &email,
        cutoff,
        today_iso,
        email_verified,
    };

    match template.render() {
        Ok(html) => HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(html),
        Err(_) => HttpResponse::InternalServerError().body("Error al renderizar la plantilla"),
    }
}
