use actix_web::{HttpResponse, Responder, get, web};
use askama::Template;
use serde::Deserialize;
use sqlx::PgPool;

use crate::mail::token;

#[derive(Template)]
#[template(path = "forgot-password.html")]
struct ForgotPasswordTemplate {}

#[derive(Template)]
#[template(path = "reset-password.html")]
struct ResetPasswordTemplate<'a> {
    token: &'a str,
    valid: bool,
}

#[derive(Deserialize)]
pub struct ResetQuery {
    token: Option<String>,
}

#[get("/forgot-password")]
async fn forgot_password() -> impl Responder {
    render(&ForgotPasswordTemplate {})
}

#[get("/reset-password")]
async fn reset_password(query: web::Query<ResetQuery>, pool: web::Data<PgPool>) -> impl Responder {
    let token = query.token.as_deref().unwrap_or("");
    let valid = token::is_well_formed(token) && token_exists(pool.get_ref(), token).await;
    render(&ResetPasswordTemplate { token, valid })
}

async fn token_exists(pool: &PgPool, token: &str) -> bool {
    let found: Option<(i32,)> = sqlx::query_as(
        "SELECT 1 FROM email_tokens
         WHERE token_hash = $1 AND kind = 'reset'
           AND used_at IS NULL AND expires_at > CURRENT_TIMESTAMP",
    )
    .bind(token::hash(token))
    .fetch_optional(pool)
    .await
    .unwrap_or(None);
    found.is_some()
}

fn render(template: &impl Template) -> HttpResponse {
    match template.render() {
        Ok(html) => HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(html),
        Err(_) => HttpResponse::InternalServerError().body("Error al renderizar la plantilla"),
    }
}
