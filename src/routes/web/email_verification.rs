use actix_session::Session;
use actix_web::{HttpResponse, Responder, get, web};
use askama::Template;
use serde::Deserialize;
use sqlx::PgPool;

use crate::mail::token;

#[derive(Template)]
#[template(path = "verify.html")]
struct VerifyTemplate {
    success: bool,
    already: bool,
    logged_in: bool,
}

#[derive(Deserialize)]
pub struct VerifyQuery {
    token: Option<String>,
}

enum VerifyOutcome {
    Verified,
    AlreadyVerified,
    Invalid,
}

#[get("/verify-email")]
async fn verify_email(
    query: web::Query<VerifyQuery>,
    pool: web::Data<PgPool>,
    session: Session,
) -> impl Responder {
    let token = query.token.as_deref().unwrap_or("");
    let outcome = match verify(pool.get_ref(), token).await {
        Ok(outcome) => outcome,
        Err(e) => {
            log::error!("verify-email: {e}");
            VerifyOutcome::Invalid
        }
    };
    let logged_in = session
        .get::<String>("user_email")
        .unwrap_or(None)
        .is_some();

    let template = match outcome {
        VerifyOutcome::Verified => VerifyTemplate {
            success: true,
            already: false,
            logged_in,
        },
        VerifyOutcome::AlreadyVerified => VerifyTemplate {
            success: true,
            already: true,
            logged_in,
        },
        VerifyOutcome::Invalid => VerifyTemplate {
            success: false,
            already: false,
            logged_in,
        },
    };

    match template.render() {
        Ok(html) => HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(html),
        Err(_) => HttpResponse::InternalServerError().body("Error al renderizar la plantilla"),
    }
}

async fn verify(pool: &PgPool, raw_token: &str) -> Result<VerifyOutcome, sqlx::Error> {
    if !token::is_well_formed(raw_token) {
        return Ok(VerifyOutcome::Invalid);
    }

    let mut tx = pool.begin().await?;
    let row: Option<(i32, bool, bool)> = sqlx::query_as(
        "SELECT t.user_id,
                (t.used_at IS NULL AND t.expires_at > CURRENT_TIMESTAMP) AS is_valid,
                (u.email_verified_at IS NOT NULL) AS already_verified
         FROM email_tokens t
         JOIN users u ON u.id = t.user_id
         WHERE t.token_hash = $1 AND t.kind = 'verify'
         FOR UPDATE OF t",
    )
    .bind(token::hash(raw_token))
    .fetch_optional(&mut *tx)
    .await?;

    let Some((user_id, is_valid, already_verified)) = row else {
        return Ok(VerifyOutcome::Invalid);
    };
    if already_verified {
        return Ok(VerifyOutcome::AlreadyVerified);
    }
    if !is_valid {
        return Ok(VerifyOutcome::Invalid);
    }

    sqlx::query(
        "UPDATE users
         SET email_verified_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP
         WHERE id = $1",
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE email_tokens SET used_at = CURRENT_TIMESTAMP
         WHERE token_hash = $1 AND kind = 'verify'",
    )
    .bind(token::hash(raw_token))
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(VerifyOutcome::Verified)
}
