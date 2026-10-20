use actix_session::Session;
use actix_web::{HttpResponse, Responder, get, web};
use actix_web_grants::protect;
use askama::Template;
use sqlx::PgPool;

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
    HttpResponse::Ok().body(admin.render().unwrap())
}

fn admin_access_denied() -> HttpResponse {
    HttpResponse::NotFound().finish()
}
