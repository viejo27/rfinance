use sqlx::{PgConnection, PgPool};

use crate::mail::MailMessage;

/// Encola un correo dentro de la transacción del flujo que lo origina:
/// si el alta o el reset fallan, el mail no queda huérfano.
pub async fn enqueue(
    conn: &mut PgConnection,
    user_id: Option<i32>,
    msg: &MailMessage,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO email_outbox (user_id, to_email, subject, body_text, body_html, kind)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(user_id)
    .bind(&msg.to)
    .bind(&msg.subject)
    .bind(&msg.text)
    .bind(&msg.html)
    .bind(&msg.kind)
    .execute(conn)
    .await?;
    Ok(())
}

/// Variante para flujos que no abren transacción propia (por ejemplo el
/// mail de prueba del admin).
pub async fn enqueue_pool(
    pool: &PgPool,
    user_id: Option<i32>,
    msg: &MailMessage,
) -> Result<(), sqlx::Error> {
    let mut conn = pool.acquire().await?;
    enqueue(&mut conn, user_id, msg).await
}
