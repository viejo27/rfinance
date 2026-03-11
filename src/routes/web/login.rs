use actix_web::{HttpResponse, Responder, get};
use askama::Template;

#[derive(Template)]
#[template(path = "login.html")]
struct LoginTemplate {}

#[get("/login")]
async fn login() -> impl Responder {
    let login = LoginTemplate {};
    HttpResponse::Ok().body(login.render().unwrap())
}
