use std::time::Duration;

use sqlx::{FromRow, PgConnection, PgPool};
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
    let mut conn = pool.acquire().await?;
    let rows = claim_batch(&mut conn, BATCH_SIZE).await?;
    drop(conn);

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
            Ok(()) => {
                let mut conn = pool.acquire().await?;
                mark_sent(&mut conn, row.id).await?;
            }
            Err(e) => {
                log::warn!(
                    "outbox: fallo al enviar {} ({}) intento {}: {}",
                    row.to_email,
                    row.kind,
                    row.attempts,
                    e.message
                );
                let error = truncate(&e.message);
                let mut conn = pool.acquire().await?;
                if e.permanent || row.attempts >= MAX_ATTEMPTS {
                    mark_failed(&mut conn, row.id, &error).await?;
                } else {
                    // 2, 4, 8, 16 minutos... con techo por MAX_ATTEMPTS.
                    let delay = 60_i32 * 2_i32.pow(row.attempts.clamp(1, 6) as u32);
                    mark_retry(&mut conn, row.id, delay, &error).await?;
                }
            }
        }
    }

    Ok(total)
}

/// Claim atómico: los pendientes vencidos y los envíos que quedaron
/// colgados por un reinicio (locked_at viejo) entran al lote.
async fn claim_batch(
    conn: &mut PgConnection,
    limit: i64,
) -> Result<Vec<PendingEmail>, sqlx::Error> {
    sqlx::query_as(
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
    .bind(limit)
    .fetch_all(&mut *conn)
    .await
}

async fn mark_sent(conn: &mut PgConnection, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE email_outbox
         SET status = 'sent', sent_at = CURRENT_TIMESTAMP, locked_at = NULL, last_error = NULL
         WHERE id = $1",
    )
    .bind(id)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

async fn mark_retry(
    conn: &mut PgConnection,
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
    .execute(&mut *conn)
    .await?;
    Ok(())
}

async fn mark_failed(conn: &mut PgConnection, id: i64, error: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE email_outbox
         SET status = 'failed', locked_at = NULL, last_error = $2
         WHERE id = $1",
    )
    .bind(id)
    .bind(error)
    .execute(&mut *conn)
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

#[cfg(test)]
mod tests {
    use super::{MAX_ERROR_LENGTH, claim_batch, mark_failed, mark_retry, mark_sent, truncate};
    use crate::mail::{MailMessage, outbox};
    use sqlx::{Connection, PgConnection};

    async fn connect() -> PgConnection {
        dotenvy::dotenv().ok();
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL para los tests");
        PgConnection::connect(&url).await.expect("conexión de test")
    }

    async fn enqueue(conn: &mut PgConnection, to: &str) -> i64 {
        let message = MailMessage {
            to: to.to_string(),
            subject: "prueba".into(),
            text: "texto".into(),
            html: "<p>texto</p>".into(),
            kind: "test".into(),
        };
        outbox::enqueue(conn, None, &message)
            .await
            .expect("encolar")
    }

    async fn state(conn: &mut PgConnection, id: i64) -> (String, i32, bool, bool) {
        sqlx::query_as(
            "SELECT status,
                    attempts,
                    scheduled_at > CURRENT_TIMESTAMP,
                    locked_at IS NULL
             FROM email_outbox WHERE id = $1",
        )
        .bind(id)
        .fetch_one(&mut *conn)
        .await
        .expect("estado del outbox")
    }

    #[actix_web::test]
    async fn claim_marca_sending_y_no_reclama_lo_reciente() {
        let mut conn = connect().await;
        let mut tx = conn.begin().await.expect("tx");
        let id = enqueue(&mut tx, "test-outbox-1@rfinance.local").await;

        let rows = claim_batch(&mut tx, 10).await.expect("claim");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, id);
        assert_eq!(rows[0].attempts, 1);

        // El lock reciente protege la fila de otro worker.
        assert!(claim_batch(&mut tx, 10).await.expect("claim").is_empty());

        tx.rollback().await.expect("rollback");
    }

    #[actix_web::test]
    async fn reclaim_lock_vencido_y_backoff() {
        let mut conn = connect().await;
        let mut tx = conn.begin().await.expect("tx");
        let id = enqueue(&mut tx, "test-outbox-2@rfinance.local").await;

        assert_eq!(claim_batch(&mut tx, 10).await.expect("claim").len(), 1);

        // Simulamos un proceso que murió a mitad de envío.
        sqlx::query(
            "UPDATE email_outbox
             SET locked_at = CURRENT_TIMESTAMP - make_interval(mins => 11)
             WHERE id = $1",
        )
        .bind(id)
        .execute(&mut *tx)
        .await
        .expect("lock viejo");

        let rows = claim_batch(&mut tx, 10).await.expect("reclaim");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].attempts, 2);

        mark_retry(&mut tx, id, 60, "timeout temporal")
            .await
            .expect("retry");
        let (status, attempts, future, unlocked) = state(&mut tx, id).await;
        assert_eq!(status, "pending");
        assert_eq!(attempts, 2);
        assert!(future);
        assert!(unlocked);

        // El backoff lo mantiene fuera de la cola hasta que venza.
        assert!(claim_batch(&mut tx, 10).await.expect("claim").is_empty());
        sqlx::query(
            "UPDATE email_outbox SET scheduled_at = CURRENT_TIMESTAMP - interval '1 second'
             WHERE id = $1",
        )
        .bind(id)
        .execute(&mut *tx)
        .await
        .expect("adelantar backoff");

        let rows = claim_batch(&mut tx, 10).await.expect("claim vencido");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].attempts, 3);

        tx.rollback().await.expect("rollback");
    }

    #[actix_web::test]
    async fn sent_y_failed_salen_de_la_cola() {
        let mut conn = connect().await;
        let mut tx = conn.begin().await.expect("tx");
        let sent = enqueue(&mut tx, "test-outbox-sent@rfinance.local").await;
        let failed = enqueue(&mut tx, "test-outbox-failed@rfinance.local").await;

        mark_sent(&mut tx, sent).await.expect("sent");
        mark_failed(&mut tx, failed, "rebote permanente")
            .await
            .expect("failed");

        assert!(claim_batch(&mut tx, 10).await.expect("claim").is_empty());

        let (sent_status, _, _, _) = state(&mut tx, sent).await;
        let (failed_status, _, _, unlocked) = state(&mut tx, failed).await;
        assert_eq!(sent_status, "sent");
        assert_eq!(failed_status, "failed");
        assert!(unlocked);

        tx.rollback().await.expect("rollback");
    }

    #[test]
    fn el_error_largo_se_corta() {
        let corto = "error corto";
        assert_eq!(truncate(corto), corto);

        let largo = "x".repeat(MAX_ERROR_LENGTH + 10);
        let recortado = truncate(&largo);
        assert_eq!(recortado.chars().count(), MAX_ERROR_LENGTH + 1);
        assert!(recortado.ends_with('…'));
    }
}
