use actix_web::{HttpResponse, Responder, post};

#[post("/login")]
async fn login() -> impl Responder {
    HttpResponse::Ok().body()
}
