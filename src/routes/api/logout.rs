use actix_session::Session;
use actix_web::{HttpResponse, Responder, post};

#[post("/api/logout")]
async fn logout(session: Session) -> impl Responder {
    session.purge();
    HttpResponse::Ok().body("Logout exitoso")
}
