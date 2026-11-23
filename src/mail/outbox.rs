use sqlx::{PgConnection, PgPool};

use crate::mail::MailMessage;

/// Encola un correo dentro de la transacción del flujo que lo origina:
/// si el alta o el reset fallan, el mail no queda huérfano. Devuelve el id
/// de la fila para auditoría y tests.
pub async fn enqueue(
    conn: &mut PgConnection,
    user_id: Option<i32>,
    msg: &MailMessage,
) -> Result<i64, sqlx::Error> {
    let (id,): (i64,) = sqlx::query_as(
        "INSERT INTO email_outbox (user_id, to_email, subject, body_text, body_html, kind)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING id",
    )
    .bind(user_id)
    .bind(&msg.to)
    .bind(&msg.subject)
    .bind(&msg.text)
    .bind(&msg.html)
    .bind(&msg.kind)
    .fetch_one(conn)
    .await?;
    Ok(id)
}

/// Variante para flujos que no abren transacción propia (por ejemplo el
/// mail de prueba del admin).
pub async fn enqueue_pool(
    pool: &PgPool,
    user_id: Option<i32>,
    msg: &MailMessage,
) -> Result<i64, sqlx::Error> {
    let mut conn = pool.acquire().await?;
    enqueue(&mut conn, user_id, msg).await
}
