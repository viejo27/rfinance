use actix_web::{HttpResponse, Responder, post};
use serde::Deserialize;

#[derive(Deserialize)]
struct LoginRequest {
    email: String,
    password: String,
}

#[post("/api/login")]
async fn login(body: actix_web::web::Json<LoginRequest>) -> impl Responder {
    let valid_email = "admin@admin.com";
    let valid_password = "password";
    
    if body.email == valid_email && body.password == valid_password {
        HttpResponse::Ok()
    } else {
        HttpResponse::Unauthorized()
    }
}
