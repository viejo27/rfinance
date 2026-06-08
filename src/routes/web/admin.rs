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
async fn admin(db_pool: web::Data<PgPool>) -> impl Responder {
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
