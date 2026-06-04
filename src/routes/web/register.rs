use actix_web::{HttpResponse, Responder, get};
use askama::Template;

#[derive(Template)]
#[template(path = "register.html")]
struct RegisterTemplate {}

#[get("/register")]
async fn register() -> impl Responder {
    let register = RegisterTemplate {};
    HttpResponse::Ok().body(register.render().unwrap())
}
