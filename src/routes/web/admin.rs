use actix_session::Session;
use actix_web::{HttpResponse, Responder, get, post, web};
use actix_web_grants::protect;
use askama::Template;
use sqlx::PgPool;

use crate::mail::{Mailer, outbox, template};
use crate::role::Role::{self, Admin};

#[derive(Template)]
#[template(path = "admin.html")]
struct AdminTemplate<'a> {
    number: &'a i32,
}

#[get("/admin")]
#[protect(any("Admin"), ty = "Role", error = "admin_access_denied")]
async fn admin(session: Session, db_pool: web::Data<PgPool>) -> impl Responder {
    // El rol del middleware viaja firmado en la cookie y puede quedar
    // quemado hasta 7 días; la revalidación fresca cierra el cookie
    // forjado con rol admin de un usuario que en la base ya no lo es.
    let Some(user_id) = session.get::<i32>("user_id").unwrap_or(None) else {
        return admin_access_denied();
    };
    let role: Option<(String,)> =
        match sqlx::query_as("SELECT role FROM user_data WHERE user_id = $1")
            .bind(user_id)
            .fetch_optional(db_pool.get_ref())
            .await
        {
            Ok(r) => r,
            Err(_) => {
                return HttpResponse::InternalServerError().body("Error al verificar el rol");
            }
        };

    if !role.map(|(r,)| r == "admin").unwrap_or(false) {
        return admin_access_denied();
    }

    let res = sqlx::query!("SELECT 20 + 7 as sum")
        .fetch_one(db_pool.get_ref())
        .await
        .expect("Error executing query");
    let admin = AdminTemplate {
        number: &res.sum.unwrap(),
    };
    HttpResponse::Ok()
        // Sin content-type el HTML servía como texto plano (el sniffing lo
        // disimulaba; nosniff lo expuso). Mismo patrón que home.rs.
        .content_type("text/html; charset=utf-8")
        .body(admin.render().unwrap())
}

fn admin_access_denied() -> HttpResponse {
    HttpResponse::NotFound().finish()
}

/// POST /admin/mail-test. Manda un correo solo a la casilla del admin
/// logueado: sirve de prueba de humo del SMTP y no se puede usar como
/// relay para terceros.
#[post("/admin/mail-test")]
#[protect(any("Admin"), ty = "Role", error = "admin_access_denied")]
async fn mail_test(
    session: Session,
    pool: web::Data<PgPool>,
    mailer: web::Data<Mailer>,
) -> impl Responder {
    let Some(user_id) = session.get::<i32>("user_id").unwrap_or(None) else {
        return admin_access_denied();
    };
    if !mailer.is_enabled() {
        return HttpResponse::ServiceUnavailable().body("El envío de correo no está configurado");
    }

    let user: Option<(String,)> = match sqlx::query_as("SELECT email FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(pool.get_ref())
        .await
    {
        Ok(user) => user,
        Err(e) => {
            log::error!("admin mail-test: {e}");
            return HttpResponse::InternalServerError().body("Error de base de datos");
        }
    };
    let Some((email,)) = user else {
        return admin_access_denied();
    };

    let message = match template::test(&email, &mailer.app_url) {
        Ok(message) => message,
        Err(e) => {
            log::error!("admin mail-test: template: {e}");
            return HttpResponse::InternalServerError().body("Error al preparar el correo");
        }
    };

    match outbox::enqueue_pool(pool.get_ref(), Some(user_id), &message).await {
        Ok(_) => HttpResponse::Ok().body(format!(
            "Prueba encolada para {email} — revisá la casilla en unos segundos."
        )),
        Err(e) => {
            log::error!("admin mail-test: outbox: {e}");
            HttpResponse::InternalServerError().body("Error al encolar el correo")
        }
    }
}
