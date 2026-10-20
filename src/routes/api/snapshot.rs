use actix_session::Session;
use actix_web::{HttpResponse, Responder, get, web};
use serde::Serialize;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::date;

/// Usuario de sesión o None: los handlers API devuelven 401 sin él.
pub fn session_user_id(session: &Session) -> Option<i32> {
    session.get::<i32>("user_id").unwrap_or(None)
}

#[derive(Serialize, FromRow)]
pub struct AccountSummary {
    pub id: Uuid,
    pub name: String,
    pub currency: String,
    pub opening_balance: f64,
    pub balance: f64,
}

#[derive(Serialize, FromRow)]
pub struct CategorySummary {
    pub id: Uuid,
    pub name: String,
    pub budget_limit: Option<f64>,
    pub currency: String,
}

#[derive(Serialize, FromRow)]
pub struct TransactionSummary {
    pub id: Uuid,
    pub entry_date: String,
    pub account: String,
    pub account_id: Uuid,
    pub category: String,
    pub category_id: Option<Uuid>,
    pub description: String,
    pub value: f64,
    pub currency: String,
    pub recurrence_id: Option<Uuid>,
}

#[derive(Serialize, FromRow)]
pub struct RecurrenceSummary {
    pub id: Uuid,
    pub installment_count: Option<i16>,
    pub generated: i16,
}

#[derive(Serialize)]
pub struct Snapshot {
    pub year: i64,
    pub month: i64,
    pub today: String,
    pub cutoff: String,
    pub accounts: Vec<AccountSummary>,
    pub categories: Vec<CategorySummary>,
    pub transactions: Vec<TransactionSummary>,
    pub recurrences: Vec<RecurrenceSummary>,
}

#[get("/api/snapshot")]
pub async fn snapshot(session: Session, pool: web::Data<PgPool>) -> impl Responder {
    let Some(user_id) = session_user_id(&session) else {
        return HttpResponse::Unauthorized().finish();
    };
    let mut conn = match pool.get_ref().acquire().await {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Error de conexión"),
    };
    match build_snapshot(&mut conn, user_id).await {
        Ok(e) => HttpResponse::Ok().json(e),
        Err(_) => HttpResponse::InternalServerError().body("Error al leer el estado"),
    }
}

/// Estado completo del usuario: saldos por cuenta, categorías,
/// asientos y recurrencias activas. Todo id expuesto es el public_id.
pub async fn build_snapshot(conn: &mut sqlx::PgConnection, user_id: i32) -> sqlx::Result<Snapshot> {
    let (today_iso, cutoff) = date::today();
    let year: i64 = today_iso[0..4].parse().unwrap_or(2026);
    let month: i64 = today_iso[5..7].parse().unwrap_or(1);

    let accounts: Vec<AccountSummary> = sqlx::query_as(
        "SELECT c.public_id AS id, c.name, c.currency,
            c.opening_balance::float8 AS opening_balance,
            c.opening_balance::float8
            + COALESCE(SUM(CASE
                WHEN m.currency = c.currency AND m.is_income THEN m.amount::float8
                WHEN m.currency = c.currency THEN -m.amount::float8
                ELSE 0 END), 0) AS balance
        FROM accounts c
        LEFT JOIN transactions m ON m.account_id = c.id
        WHERE c.user_id = $1
        GROUP BY c.id
        ORDER BY c.id",
    )
    .bind(user_id)
    .fetch_all(&mut *conn)
    .await?;

    let categories: Vec<CategorySummary> = sqlx::query_as(
        "SELECT public_id AS id, name, budget_limit::float8 AS budget_limit, currency
        FROM categories WHERE user_id = $1 ORDER BY id",
    )
    .bind(user_id)
    .fetch_all(&mut *conn)
    .await?;

    let transactions: Vec<TransactionSummary> = sqlx::query_as(
        "SELECT m.public_id AS id, to_char(m.entry_date, 'YYYY-MM-DD') AS entry_date,
            a.name AS account, a.public_id AS account_id, m.category_label AS category,
            cat.public_id AS category_id, m.description,
            CASE WHEN m.is_income THEN m.amount::float8 ELSE -m.amount::float8 END AS value,
            m.currency, r.public_id AS recurrence_id
        FROM transactions m
        JOIN accounts a ON a.id = m.account_id
        LEFT JOIN categories cat ON cat.id = m.category_id
        LEFT JOIN recurrences r ON r.id = m.recurrence_id
        WHERE m.user_id = $1
        ORDER BY m.entry_date DESC, m.id DESC",
    )
    .bind(user_id)
    .fetch_all(&mut *conn)
    .await?;

    let recurrences: Vec<RecurrenceSummary> = sqlx::query_as(
        "SELECT public_id AS id, installment_count, installments_generated AS generated
        FROM recurrences WHERE user_id = $1 AND is_active ORDER BY public_id",
    )
    .bind(user_id)
    .fetch_all(&mut *conn)
    .await?;

    Ok(Snapshot {
        year,
        month,
        today: today_iso,
        cutoff,
        accounts,
        categories,
        transactions,
        recurrences,
    })
}
