use actix_session::Session;
use actix_web::{HttpResponse, Responder, get, web};
use askama::Template;
use serde::Deserialize;
use sqlx::{PgConnection, PgPool};

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

#[derive(Debug, PartialEq, Eq)]
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
    let outcome = match pool.begin().await {
        Ok(mut tx) => match verify_token(&mut tx, token).await {
            Ok(VerifyOutcome::Verified) => match tx.commit().await {
                Ok(()) => VerifyOutcome::Verified,
                Err(e) => {
                    log::error!("verify-email: commit: {e}");
                    VerifyOutcome::Invalid
                }
            },
            Ok(outcome) => {
                let _ = tx.rollback().await;
                outcome
            }
            Err(e) => {
                log::error!("verify-email: {e}");
                let _ = tx.rollback().await;
                VerifyOutcome::Invalid
            }
        },
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

async fn verify_token(
    conn: &mut PgConnection,
    raw_token: &str,
) -> Result<VerifyOutcome, sqlx::Error> {
    if !token::is_well_formed(raw_token) {
        return Ok(VerifyOutcome::Invalid);
    }

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
    .fetch_optional(&mut *conn)
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
    .execute(&mut *conn)
    .await?;
    sqlx::query(
        "UPDATE email_tokens SET used_at = CURRENT_TIMESTAMP
         WHERE token_hash = $1 AND kind = 'verify'",
    )
    .bind(token::hash(raw_token))
    .execute(&mut *conn)
    .await?;
    Ok(VerifyOutcome::Verified)
}

#[cfg(test)]
mod tests {
    use super::{VerifyOutcome, verify_token};
    use crate::mail::token;
    use sqlx::{Connection, PgConnection};
    use std::time::{SystemTime, UNIX_EPOCH};

    async fn connect() -> PgConnection {
        dotenvy::dotenv().ok();
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL para los tests");
        PgConnection::connect(&url).await.expect("conexión de test")
    }

    async fn make_user(tx: &mut PgConnection) -> i32 {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(1);
        let email = format!("test-verify-{nanos}@rfinance.local");
        let (id,): (i32,) =
            sqlx::query_as("INSERT INTO users (email, password) VALUES ($1, 'x') RETURNING id")
                .bind(&email)
                .fetch_one(&mut *tx)
                .await
                .expect("user de test");
        id
    }

    async fn give_token(tx: &mut PgConnection, user_id: i32, kind: &str, raw: &str, minutes: i32) {
        sqlx::query(
            "INSERT INTO email_tokens (user_id, kind, token_hash, expires_at)
             VALUES ($1, $2, $3, CURRENT_TIMESTAMP + make_interval(mins => $4))",
        )
        .bind(user_id)
        .bind(kind)
        .bind(token::hash(raw))
        .bind(minutes)
        .execute(&mut *tx)
        .await
        .expect("token de test");
    }

    async fn state(tx: &mut PgConnection, user_id: i32, raw: &str) -> (bool, bool) {
        sqlx::query_as(
            "SELECT u.email_verified_at IS NOT NULL, t.used_at IS NOT NULL
             FROM users u
             JOIN email_tokens t ON t.user_id = u.id
             WHERE u.id = $1 AND t.token_hash = $2",
        )
        .bind(user_id)
        .bind(token::hash(raw))
        .fetch_one(&mut *tx)
        .await
        .expect("estado de verificación")
    }

    #[actix_web::test]
    async fn verificacion_marca_email_y_es_idempotente() {
        let mut conn = connect().await;
        let mut tx = conn.begin().await.expect("tx");
        let user_id = make_user(&mut tx).await;
        let raw = token::generate();
        give_token(&mut tx, user_id, "verify", &raw, 60).await;

        assert_eq!(
            verify_token(&mut tx, &raw).await.expect("verify"),
            VerifyOutcome::Verified
        );
        assert_eq!(state(&mut tx, user_id, &raw).await, (true, true));

        // Un segundo click del mismo enlace sigue siendo un éxito.
        assert_eq!(
            verify_token(&mut tx, &raw).await.expect("verify repetido"),
            VerifyOutcome::AlreadyVerified
        );

        tx.rollback().await.expect("rollback");
    }

    #[actix_web::test]
    async fn verificacion_vencida_no_marca() {
        let mut conn = connect().await;
        let mut tx = conn.begin().await.expect("tx");
        let user_id = make_user(&mut tx).await;
        let raw = token::generate();
        give_token(&mut tx, user_id, "verify", &raw, -1).await;

        assert_eq!(
            verify_token(&mut tx, &raw).await.expect("verify"),
            VerifyOutcome::Invalid
        );
        assert_eq!(state(&mut tx, user_id, &raw).await, (false, false));

        tx.rollback().await.expect("rollback");
    }

    #[actix_web::test]
    async fn verificacion_ignora_tokens_de_reset() {
        let mut conn = connect().await;
        let mut tx = conn.begin().await.expect("tx");
        let user_id = make_user(&mut tx).await;
        let raw = token::generate();
        give_token(&mut tx, user_id, "reset", &raw, 60).await;

        assert_eq!(
            verify_token(&mut tx, &raw).await.expect("verify"),
            VerifyOutcome::Invalid
        );
        assert_eq!(state(&mut tx, user_id, &raw).await, (false, false));

        tx.rollback().await.expect("rollback");
    }

    #[actix_web::test]
    async fn verificacion_rechaza_token_mal_formado() {
        let mut conn = connect().await;
        let mut tx = conn.begin().await.expect("tx");

        assert_eq!(
            verify_token(&mut tx, "no-es-un-token")
                .await
                .expect("verify"),
            VerifyOutcome::Invalid
        );

        tx.rollback().await.expect("rollback");
    }
}
