use actix_web::{HttpResponse, Responder, get, web};
use askama::Template;
use sqlx::PgPool;

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate<'a> {
    name: &'a i32,
}

#[get("/")]
async fn index(db_pool: web::Data<PgPool>) -> impl Responder {
    let res = sqlx::query!("SELECT 20 + 7 as sum")
        .fetch_one(db_pool.get_ref())
        .await
        .expect("Error executing query");
    let hello = IndexTemplate {
        name: &res.sum.unwrap(),
    };
    HttpResponse::Ok().body(hello.render().unwrap())
}
