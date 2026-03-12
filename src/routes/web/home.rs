use actix_web::{HttpResponse, Responder, get, web};
use askama::Template;
use sqlx::PgPool;

const SESSION_COOKIE_NAME: &str = "rfinance_session";

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate<'a> {
    name: &'a i32,
}

#[get("/")]
async fn index(db_pool: web::Data<PgPool>, req: actix_web::HttpRequest) -> impl Responder {
    let session_cookie = req.cookie(SESSION_COOKIE_NAME);

    if session_cookie.is_none() {
        return HttpResponse::Found()
            .append_header(("Location", "/login"))
            .finish();
    }

    let res = sqlx::query!("SELECT 20 + 7 as sum")
        .fetch_one(db_pool.get_ref())
        .await
        .expect("Error executing query");
    let hello = IndexTemplate {
        name: &res.sum.unwrap(),
    };
    HttpResponse::Ok().body(hello.render().unwrap())
}
