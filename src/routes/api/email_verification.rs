use actix_session::Session;
use actix_web::{HttpResponse, Responder, web};
use sqlx::PgPool;

use crate::mail::{Mailer, outbox, template, token};

const VERIFY_TOKEN_HOURS: i32 = 48;
const RESEND_COOLDOWN_MINUTES: i32 = 5;

/// POST /api/resend-verification. Autenticado y con cooldown por usuario:
/// el rate limit por IP no alcanza si el usuario abre varias pestañas.
pub async fn resend_verification(
    session: Session,
    pool: web::Data<PgPool>,
    mailer: web::Data<Mailer>,
) -> impl Responder {
    let Some(user_id) = session.get::<i32>("user_id").unwrap_or(None) else {
        return HttpResponse::Unauthorized().body("Sesión expirada — ingresá de nuevo");
    };
    if !mailer.is_enabled() {
        return HttpResponse::ServiceUnavailable().body("El envío de correo no está configurado");
    }

    let user: Option<(String, Option<String>, bool)> = match sqlx::query_as(
        "SELECT u.email, ud.name, u.email_verified_at IS NOT NULL
         FROM users u
         LEFT JOIN user_data ud ON ud.user_id = u.id
         WHERE u.id = $1 AND u.deleted_at IS NULL",
    )
    .bind(user_id)
    .fetch_optional(pool.get_ref())
    .await
    {
        Ok(user) => user,
        Err(e) => {
            log::error!("resend-verification: {e}");
            return HttpResponse::InternalServerError().body("Error de base de datos");
        }
    };

    let Some((email, name, verified)) = user else {
        return HttpResponse::Unauthorized().body("Sesión expirada — ingresá de nuevo");
    };
    if verified {
        return HttpResponse::BadRequest().body("Tu email ya está verificado");
    }

    let recent: Option<(i32,)> = match sqlx::query_as(
        "SELECT 1 FROM email_tokens
         WHERE user_id = $1 AND kind = 'verify' AND used_at IS NULL
           AND created_at > CURRENT_TIMESTAMP - make_interval(mins => $2)
         LIMIT 1",
    )
    .bind(user_id)
    .bind(RESEND_COOLDOWN_MINUTES)
    .fetch_optional(pool.get_ref())
    .await
    {
        Ok(recent) => recent,
        Err(e) => {
            log::error!("resend-verification: {e}");
            return HttpResponse::InternalServerError().body("Error de base de datos");
        }
    };
    if recent.is_some() {
        return HttpResponse::TooManyRequests()
            .body("Ya te enviamos un enlace hace poco — revisá tu casilla o esperá unos minutos.");
    }

    let raw_token = token::generate();
    let token_hash = token::hash(&raw_token);
    let verify_url = format!("{}/verify-email?token={}", mailer.app_url, raw_token);
    let message = match template::verification(
        &email,
        name.as_deref().unwrap_or(""),
        &verify_url,
        &mailer.app_url,
    ) {
        Ok(message) => message,
        Err(e) => {
            log::error!("resend-verification: template: {e}");
            return HttpResponse::InternalServerError().body("Error al preparar el correo");
        }
    };

    let mut tx = match pool.begin().await {
        Ok(tx) => tx,
        Err(e) => {
            log::error!("resend-verification: {e}");
            return HttpResponse::InternalServerError().body("Error de base de datos");
        }
    };

    let result = async {
        sqlx::query(
            "UPDATE email_tokens SET used_at = CURRENT_TIMESTAMP
             WHERE user_id = $1 AND kind = 'verify' AND used_at IS NULL",
        )
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO email_tokens (user_id, kind, token_hash, expires_at)
             VALUES ($1, 'verify', $2, CURRENT_TIMESTAMP + make_interval(hours => $3))",
        )
        .bind(user_id)
        .bind(&token_hash)
        .bind(VERIFY_TOKEN_HOURS)
        .execute(&mut *tx)
        .await?;
        outbox::enqueue(&mut tx, Some(user_id), &message).await?;
        tx.commit().await
    }
    .await;

    match result {
        Ok(()) => HttpResponse::Ok().body("Te enviamos un enlace nuevo — revisá tu casilla."),
        Err(e) => {
            log::error!("resend-verification: {e}");
            HttpResponse::InternalServerError().body("Error de base de datos")
        }
    }
}
