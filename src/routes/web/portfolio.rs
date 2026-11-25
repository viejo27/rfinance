use actix_session::Session;
use actix_web::{HttpResponse, Responder, get};
use askama::Template;

use crate::date;

#[derive(Template)]
#[template(path = "portfolio.html")]
struct PortfolioTemplate<'a> {
    email: &'a str,
    cutoff: String,
    today_iso: String,
}

#[get("/inversiones")]
async fn portfolio(session: Session) -> impl Responder {
    let email = session
        .get::<String>("user_email")
        .unwrap_or(None)
        .unwrap_or_default();
    let (today_iso, cutoff) = date::today();
    let template = PortfolioTemplate {
        email: &email,
        cutoff,
        today_iso,
    };
    match template.render() {
        Ok(html) => HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(html),
        Err(_) => HttpResponse::InternalServerError().body("Error al renderizar la plantilla"),
    }
}
