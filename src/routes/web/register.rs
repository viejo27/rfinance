use actix_web::{HttpResponse, Responder, get};
use askama::Template;

#[derive(Template)]
#[template(path = "register.html")]
struct RegisterTemplate {}

#[get("/register")]
async fn register() -> impl Responder {
    let register = RegisterTemplate {};
    HttpResponse::Ok()
        // Sin content-type el HTML servía como texto plano (el sniffing lo
        // disimulaba; nosniff lo expuso). Mismo patrón que home.rs.
        .content_type("text/html; charset=utf-8")
        .body(register.render().unwrap())
}
