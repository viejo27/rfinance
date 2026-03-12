use actix_web::{HttpResponse, Responder, post};

const SESSION_COOKIE_NAME: &str = "rfinance_session";

#[post("/api/logout")]
async fn logout() -> impl Responder {
    let cookie = actix_web::cookie::Cookie::build(SESSION_COOKIE_NAME, "")
        .path("/")
        .max_age(actix_web::cookie::time::Duration::ZERO)
        .http_only(true)
        .finish();

    HttpResponse::Ok().cookie(cookie).body("Logout exitoso")
}
