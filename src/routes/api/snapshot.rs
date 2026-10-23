use actix_session::Session;
use actix_web::{HttpResponse, Responder, get, web};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::date;

/// Límite de asientos por snapshot: con el filtro por año el payload queda
/// acotado y el endpoint deja de releer todo el libro en cada carga.
const MAX_SNAPSHOT_TRANSACTIONS: i64 = 5000;

/// Espeja el rango de is_valid_date del alta de asientos: si un año nuevo
/// no puede tener asientos cargados, el snapshot tampoco lo atiende.
fn is_valid_snapshot_year(year: i64) -> bool {
    (2000..=2100).contains(&year)
}

/// Usuario de sesión o None: los handlers API devuelven 401 sin él.
pub fn session_user_id(session: &Session) -> Option<i32> {
    session.get::<i32>("user_id").unwrap_or(None)
}

#[derive(Deserialize)]
pub struct SnapshotQuery {
    /// Año del libro a leer; ausente = el corriente.
    pub year: Option<i64>,
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

/// Transferencia entre cuentas: movimiento propio, no un asiento. Cada monto
/// está en la moneda de su pata.
#[derive(Serialize, FromRow)]
pub struct TransferSummary {
    pub id: Uuid,
    pub entry_date: String,
    pub from_account: String,
    pub from_account_id: Uuid,
    pub to_account: String,
    pub to_account_id: Uuid,
    pub from_amount: f64,
    pub from_currency: String,
    pub to_amount: f64,
    pub to_currency: String,
    pub description: String,
}

#[derive(Serialize)]
pub struct Snapshot {
    /// Año pedido: los gráficos del dashboard rotan alrededor de este valor.
    pub year: i64,
    /// Mes corriente del servidor; si el año pedido es uno viejo, el mes
    /// sirve para vista inicial pero el libro queda en el año pedido.
    pub month: i64,
    pub today: String,
    pub cutoff: String,
    pub accounts: Vec<AccountSummary>,
    pub categories: Vec<CategorySummary>,
    pub transactions: Vec<TransactionSummary>,
    pub transfers: Vec<TransferSummary>,
    pub recurrences: Vec<RecurrenceSummary>,
}

#[get("/api/snapshot")]
pub async fn snapshot(
    session: Session,
    pool: web::Data<PgPool>,
    query: web::Query<SnapshotQuery>,
) -> impl Responder {
    let Some(user_id) = session_user_id(&session) else {
        return HttpResponse::Unauthorized().finish();
    };
    let year = match query.year {
        Some(y) if is_valid_snapshot_year(y) => y,
        Some(_) => return HttpResponse::BadRequest().body("El año debe estar entre 2000 y 2100"),
        None => date::today_iso()[0..4].parse().unwrap_or(2026),
    };
    let mut conn = match pool.get_ref().acquire().await {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Error de conexión"),
    };
    match build_snapshot(&mut conn, user_id, year).await {
        Ok(e) => HttpResponse::Ok().json(e),
        Err(_) => HttpResponse::InternalServerError().body("Error al leer el estado"),
    }
}

/// Estado del año pedido: saldos por cuenta, categorías, asientos y
/// transferencias del año, y recurrencias activas. Todo id expuesto es el
/// public_id. Los saldos de cuenta son de por vida (el libro completo y las
/// transferencias entran al balance); los movimientos del payload son solo
/// del año consultado.
pub async fn build_snapshot(
    conn: &mut sqlx::PgConnection,
    user_id: i32,
    year: i64,
) -> sqlx::Result<Snapshot> {
    let (today_iso, cutoff) = date::today();
    let month: i64 = today_iso[5..7].parse().unwrap_or(1);

    // Las transferencias se suman por pata (cada monto en la moneda de su
    // cuenta) con subconsultas escalares: evita fan-out de joins.
    let accounts: Vec<AccountSummary> = sqlx::query_as(
        "SELECT c.public_id AS id, c.name, c.currency,
            c.opening_balance::float8 AS opening_balance,
            c.opening_balance::float8
            + COALESCE(SUM(CASE
                WHEN m.currency = c.currency AND m.is_income THEN m.amount::float8
                WHEN m.currency = c.currency THEN -m.amount::float8
                ELSE 0 END), 0)
            - COALESCE((
                SELECT SUM(t.from_amount::float8) FROM transfers t
                WHERE t.from_account_id = c.id), 0)
            + COALESCE((
                SELECT SUM(t.to_amount::float8) FROM transfers t
                WHERE t.to_account_id = c.id), 0) AS balance
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

    let desde = format!("{year:04}-01-01");
    let hasta = format!("{}-01-01", year + 1);
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
            AND m.entry_date >= $2::date AND m.entry_date < $3::date
        ORDER BY m.entry_date DESC, m.id DESC
        LIMIT $4",
    )
    .bind(user_id)
    .bind(&desde)
    .bind(&hasta)
    .bind(MAX_SNAPSHOT_TRANSACTIONS)
    .fetch_all(&mut *conn)
    .await?;

    let transfers: Vec<TransferSummary> = sqlx::query_as(
        "SELECT t.public_id AS id, to_char(t.entry_date, 'YYYY-MM-DD') AS entry_date,
            fa.name AS from_account, fa.public_id AS from_account_id,
            ta.name AS to_account, ta.public_id AS to_account_id,
            t.from_amount::float8 AS from_amount, t.from_currency,
            t.to_amount::float8 AS to_amount, t.to_currency, t.description
        FROM transfers t
        JOIN accounts fa ON fa.id = t.from_account_id
        JOIN accounts ta ON ta.id = t.to_account_id
        WHERE t.user_id = $1
            AND t.entry_date >= $2::date AND t.entry_date < $3::date
        ORDER BY t.entry_date DESC, t.id DESC
        LIMIT $4",
    )
    .bind(user_id)
    .bind(&desde)
    .bind(&hasta)
    .bind(MAX_SNAPSHOT_TRANSACTIONS)
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
        transfers,
        recurrences,
    })
}

#[cfg(test)]
mod tests {
    use super::{build_snapshot, is_valid_snapshot_year};
    use sqlx::Connection;
    use sqlx::PgConnection;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Misma convención que los tests de recurrencias: una transacción que
    /// se revierte al final, porque el rol de .env no tiene CREATEDB.
    async fn connect() -> PgConnection {
        dotenvy::dotenv().ok();
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL para los tests");
        PgConnection::connect(&url).await.expect("conexión de test")
    }

    async fn make_user(tx: &mut PgConnection) -> i32 {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as u64 + d.as_secs())
            .unwrap_or(1);
        let email = format!("test-snap-{nanos}@rfinance.local");
        let (id,): (i32,) =
            sqlx::query_as("INSERT INTO users (email, password) VALUES ($1, 'x') RETURNING id")
                .bind(&email)
                .fetch_one(&mut *tx)
                .await
                .expect("user de test");
        id
    }

    async fn make_account(tx: &mut PgConnection, user_id: i32) -> i32 {
        let (cid,): (i32,) = sqlx::query_as(
            "INSERT INTO accounts (user_id, name, currency) VALUES ($1, 'Banco', 'ARS') RETURNING id",
        )
        .bind(user_id)
        .fetch_one(&mut *tx)
        .await
        .expect("cuenta de test");
        cid
    }

    async fn make_transaction(
        tx: &mut PgConnection,
        user_id: i32,
        account_id: i32,
        entry_date: &str,
        description: &str,
        amount: f64,
    ) -> i32 {
        let (id,): (i32,) = sqlx::query_as(
            "INSERT INTO transactions
                (user_id, account_id, category_id, category_label, description,
                 amount, is_income, currency, entry_date)
            VALUES ($1, $2, NULL, 'SIN CATEGORÍA', $3, $4::numeric, false, 'ARS', $5::date)
            RETURNING id",
        )
        .bind(user_id)
        .bind(account_id)
        .bind(description)
        .bind(amount)
        .bind(entry_date)
        .fetch_one(&mut *tx)
        .await
        .expect("asiento de test");
        id
    }

    #[async_std::test]
    async fn snapshot_year_filter() {
        // El snapshot ya no trae el libro entero: pide un año y devuelve
        // solo ese año, aunque existan asientos de años vecinos.
        let mut conn = connect().await;
        let mut tx = conn.begin().await.expect("tx");
        let user = make_user(&mut tx).await;
        let cid = make_account(&mut tx, user).await;
        make_transaction(&mut tx, user, cid, "2025-08-01", "Vacaciones", 100.0).await;
        make_transaction(&mut tx, user, cid, "2026-03-05", "Café", 200.0).await;
        make_transaction(&mut tx, user, cid, "2026-04-01", "Alquiler", 50.0).await;

        let snapshot = build_snapshot(&mut tx, user, 2026)
            .await
            .expect("snapshot 2026");
        assert_eq!(snapshot.year, 2026);
        assert_eq!(snapshot.transactions.len(), 2);
        assert!(
            snapshot
                .transactions
                .iter()
                .all(|t| t.entry_date.starts_with("2026"))
        );

        let snapshot_2025 = build_snapshot(&mut tx, user, 2025)
            .await
            .expect("snapshot 2025");
        assert_eq!(snapshot_2025.transactions.len(), 1);

        tx.rollback().await.expect("rollback");
    }

    #[test]
    fn rango_de_años() {
        assert!(is_valid_snapshot_year(2000));
        assert!(is_valid_snapshot_year(2100));
        assert!(!is_valid_snapshot_year(1999));
        assert!(!is_valid_snapshot_year(2101));
    }
}
