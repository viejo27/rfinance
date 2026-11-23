use std::time::Duration;

use sqlx::{FromRow, PgPool};
use tokio::time::{MissedTickBehavior, interval, sleep};

use crate::mail::{MailMessage, Mailer};

const TICK: Duration = Duration::from_secs(5);
const SEND_PAUSE: Duration = Duration::from_millis(200);
const BATCH_SIZE: i64 = 10;
const MAX_ATTEMPTS: i32 = 5;
const STALE_LOCK_MINUTES: i32 = 10;
const MAX_ERROR_LENGTH: usize = 500;

/// Arranca el único worker del outbox. Procesa de a un mail con una pausa
/// corta: el volumen transaccional no necesita más, y así una ráfaga de
/// altas no golpea el SMTP del hosting de golpe.
pub fn spawn(pool: PgPool, mailer: Mailer) {
    tokio::spawn(async move {
        let mut ticker = interval(TICK);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            match process_batch(&pool, &mailer).await {
                Ok(0) => {}
                Ok(n) => log::info!("outbox: {n} correo(s) procesado(s)"),
                Err(e) => log::error!("outbox: error al procesar el lote: {e}"),
            }
        }
    });
}

#[derive(FromRow)]
struct PendingEmail {
    id: i64,
    to_email: String,
    subject: String,
    body_text: String,
    body_html: String,
    kind: String,
    attempts: i32,
}

async fn process_batch(pool: &PgPool, mailer: &Mailer) -> Result<usize, sqlx::Error> {
    // Claim atómico: los pendientes vencidos y los envíos que quedaron
    // colgados por un reinicio (locked_at viejo) entran al lote.
    let rows: Vec<PendingEmail> = sqlx::query_as(
        "UPDATE email_outbox
         SET status = 'sending',
             attempts = attempts + 1,
             locked_at = CURRENT_TIMESTAMP
         WHERE id IN (
             SELECT id FROM email_outbox
             WHERE (status = 'pending' AND scheduled_at <= CURRENT_TIMESTAMP)
                OR (status = 'sending'
                    AND locked_at < CURRENT_TIMESTAMP - make_interval(mins => $1))
             ORDER BY id
             LIMIT $2
             FOR UPDATE SKIP LOCKED
         )
         RETURNING id, to_email, subject, body_text, body_html, kind, attempts",
    )
    .bind(STALE_LOCK_MINUTES)
    .bind(BATCH_SIZE)
    .fetch_all(pool)
    .await?;

    let total = rows.len();
    for (index, row) in rows.iter().enumerate() {
        if index > 0 {
            sleep(SEND_PAUSE).await;
        }

        let message = MailMessage {
            to: row.to_email.clone(),
            subject: row.subject.clone(),
            text: row.body_text.clone(),
            html: row.body_html.clone(),
            kind: row.kind.clone(),
        };

        match mailer.send(&message).await {
            Ok(()) => mark_sent(pool, row.id).await?,
            Err(e) => {
                log::warn!(
                    "outbox: fallo al enviar {} ({}) intento {}: {}",
                    row.to_email,
                    row.kind,
                    row.attempts,
                    e.message
                );
                let error = truncate(&e.message);
                if e.permanent || row.attempts >= MAX_ATTEMPTS {
                    mark_failed(pool, row.id, &error).await?;
                } else {
                    // 2, 4, 8, 16 minutos... con techo por MAX_ATTEMPTS.
                    let delay = 60_i32 * 2_i32.pow(row.attempts.clamp(1, 6) as u32);
                    mark_retry(pool, row.id, delay, &error).await?;
                }
            }
        }
    }

    Ok(total)
}

async fn mark_sent(pool: &PgPool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE email_outbox
         SET status = 'sent', sent_at = CURRENT_TIMESTAMP, locked_at = NULL, last_error = NULL
         WHERE id = $1",
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

async fn mark_retry(
    pool: &PgPool,
    id: i64,
    delay_seconds: i32,
    error: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE email_outbox
         SET status = 'pending',
             locked_at = NULL,
             last_error = $2,
             scheduled_at = CURRENT_TIMESTAMP + make_interval(secs => $3)
         WHERE id = $1",
    )
    .bind(id)
    .bind(error)
    .bind(delay_seconds)
    .execute(pool)
    .await?;
    Ok(())
}

async fn mark_failed(pool: &PgPool, id: i64, error: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE email_outbox
         SET status = 'failed', locked_at = NULL, last_error = $2
         WHERE id = $1",
    )
    .bind(id)
    .bind(error)
    .execute(pool)
    .await?;
    Ok(())
}

fn truncate(message: &str) -> String {
    if message.chars().count() <= MAX_ERROR_LENGTH {
        return message.to_string();
    }
    let mut cut: String = message.chars().take(MAX_ERROR_LENGTH).collect();
    cut.push('…');
    cut
}
