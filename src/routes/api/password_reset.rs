use actix_session::Session;
use actix_web::{HttpResponse, Responder, web};
use serde::Deserialize;
use sqlx::PgPool;

use crate::mail::{Mailer, outbox, template, token};
use crate::routes::api::register::{is_valid_email_format, validate_password};

/// Respuesta uniforme: no revela si el email existe (misma política que el
/// login, que quema un bcrypt dummy para el camino inexistente).
const GENERIC_FORGOT_RESPONSE: &str =
    "Si el email está registrado, te enviamos un enlace para restablecer la contraseña.";
const RESET_TOKEN_MINUTES: i32 = 60;
const FORGOT_COOLDOWN_MINUTES: i32 = 2;

#[derive(Deserialize)]
pub struct ForgotPasswordRequest {
    email: String,
}

#[derive(Deserialize)]
pub struct ResetPasswordRequest {
    token: String,
    password: String,
}

enum ResetError {
    Invalid,
    Db(String),
}

/// POST /api/forgot-password. Responde 200 siempre, incluso si el email no
/// existe o si el envío falla: el detalle queda en el log, no en el cliente.
pub async fn forgot_password(
    body: web::Json<ForgotPasswordRequest>,
    pool: web::Data<PgPool>,
    mailer: web::Data<Mailer>,
) -> impl Responder {
    if let Err(e) = send_reset_link(&body.email, pool.get_ref(), mailer.as_ref()).await {
        log::error!("forgot-password: no se pudo encolar el reset: {e}");
    }
    HttpResponse::Ok().body(GENERIC_FORGOT_RESPONSE)
}

async fn send_reset_link(email: &str, pool: &PgPool, mailer: &Mailer) -> Result<(), String> {
    if !mailer.is_enabled() || !is_valid_email_format(email) {
        return Ok(());
    }

    let user: Option<(i32, String, Option<String>)> = sqlx::query_as(
        "SELECT u.id, u.email, ud.name
         FROM users u
         LEFT JOIN user_data ud ON ud.user_id = u.id
         WHERE u.email = $1 AND u.deleted_at IS NULL",
    )
    .bind(email)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    let Some((user_id, to, name)) = user else {
        return Ok(());
    };

    // Cooldown por usuario: aunque el rate limit sea por IP, un atacante
    // distribuido no debe poder inundar la casilla de una víctima.
    let recent: Option<(i32,)> = sqlx::query_as(
        "SELECT 1 FROM email_tokens
         WHERE user_id = $1 AND kind = 'reset' AND used_at IS NULL
           AND created_at > CURRENT_TIMESTAMP - make_interval(mins => $2)
         LIMIT 1",
    )
    .bind(user_id)
    .bind(FORGOT_COOLDOWN_MINUTES)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;
    if recent.is_some() {
        return Ok(());
    }

    let raw_token = token::generate();
    let token_hash = token::hash(&raw_token);
    let reset_url = format!("{}/reset-password?token={}", mailer.app_url, raw_token);
    let message = template::password_reset(
        &to,
        name.as_deref().unwrap_or(""),
        &reset_url,
        &mailer.app_url,
    )
    .map_err(|e| e.to_string())?;

    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    sqlx::query(
        "UPDATE email_tokens SET used_at = CURRENT_TIMESTAMP
         WHERE user_id = $1 AND kind = 'reset' AND used_at IS NULL",
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    sqlx::query(
        "INSERT INTO email_tokens (user_id, kind, token_hash, expires_at)
         VALUES ($1, 'reset', $2, CURRENT_TIMESTAMP + make_interval(mins => $3))",
    )
    .bind(user_id)
    .bind(&token_hash)
    .bind(RESET_TOKEN_MINUTES)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    outbox::enqueue(&mut tx, Some(user_id), &message)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

/// POST /api/reset-password. Token de un solo uso; al cambiar la contraseña
/// sube `session_epoch`, así que todas las cookies emitidas antes dejan de
/// valer en el próximo request.
pub async fn reset_password(
    body: web::Json<ResetPasswordRequest>,
    pool: web::Data<PgPool>,
    mailer: web::Data<Mailer>,
    session: Session,
) -> impl Responder {
    let raw_token = body.token.trim();
    if !token::is_well_formed(raw_token) {
        return HttpResponse::BadRequest().body("El enlace no es válido o venció");
    }
    if let Err(msg) = validate_password(&body.password) {
        return HttpResponse::BadRequest().body(msg);
    }

    let password_hash = match bcrypt::hash(&body.password, bcrypt::DEFAULT_COST) {
        Ok(hash) => hash,
        Err(_) => {
            return HttpResponse::InternalServerError().body("Error al hashear la contraseña");
        }
    };

    match apply_reset(pool.get_ref(), mailer.as_ref(), raw_token, &password_hash).await {
        Ok(()) => {
            // Si el reset se hizo con una sesión abierta, esa cookie también
            // queda invalidada por el epoch: limpio el estado local ya.
            session.clear();
            HttpResponse::Ok().body("Contraseña actualizada — ingresá con tu nueva contraseña.")
        }
        Err(ResetError::Invalid) => {
            HttpResponse::BadRequest().body("El enlace no es válido o venció")
        }
        Err(ResetError::Db(e)) => {
            log::error!("reset-password: {e}");
            HttpResponse::InternalServerError().body("Error de base de datos")
        }
    }
}

async fn apply_reset(
    pool: &PgPool,
    mailer: &Mailer,
    raw_token: &str,
    password_hash: &str,
) -> Result<(), ResetError> {
    let token_hash = token::hash(raw_token);
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| ResetError::Db(e.to_string()))?;

    let row: Option<(i32, String)> = sqlx::query_as(
        "SELECT t.user_id, u.email
         FROM email_tokens t
         JOIN users u ON u.id = t.user_id
         WHERE t.token_hash = $1 AND t.kind = 'reset'
           AND t.used_at IS NULL AND t.expires_at > CURRENT_TIMESTAMP
         FOR UPDATE OF t",
    )
    .bind(&token_hash)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| ResetError::Db(e.to_string()))?;

    let Some((user_id, email)) = row else {
        return Err(ResetError::Invalid);
    };

    sqlx::query(
        "UPDATE users
         SET password = $1, session_epoch = session_epoch + 1, updated_at = CURRENT_TIMESTAMP
         WHERE id = $2",
    )
    .bind(password_hash)
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| ResetError::Db(e.to_string()))?;

    // Un solo enlace activo por usuario: consumir este invalida los demás.
    sqlx::query(
        "UPDATE email_tokens SET used_at = CURRENT_TIMESTAMP
         WHERE user_id = $1 AND kind = 'reset' AND used_at IS NULL",
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| ResetError::Db(e.to_string()))?;

    if mailer.is_enabled() {
        let name: Option<(Option<String>,)> =
            sqlx::query_as("SELECT name FROM user_data WHERE user_id = $1")
                .bind(user_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| ResetError::Db(e.to_string()))?;
        let name = name.and_then(|(name,)| name).unwrap_or_default();
        let message = template::password_changed(&email, &name, &mailer.app_url)
            .map_err(|e| ResetError::Db(e.to_string()))?;
        outbox::enqueue(&mut tx, Some(user_id), &message)
            .await
            .map_err(|e| ResetError::Db(e.to_string()))?;
    }

    tx.commit()
        .await
        .map_err(|e| ResetError::Db(e.to_string()))?;
    Ok(())
}
