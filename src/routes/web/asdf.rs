use actix_web::{HttpResponse, Responder, get, web};
use askama::Template;
use sqlx::PgPool;

#[derive(Template)]
#[template(path = "asdf.html")]
struct AsdfTemplate<'a> {
    number: &'a i32,
}

#[get("/asdf")]
async fn asdf(db_pool: web::Data<PgPool>) -> impl Responder {
    let res = sqlx::query!("SELECT 20 + 7 as sum")
        .fetch_one(db_pool.get_ref())
        .await
        .expect("Error executing query");
    let asdf = AsdfTemplate {
        number: &res.sum.unwrap(),
    };
    HttpResponse::Ok().body(asdf.render().unwrap())
}
