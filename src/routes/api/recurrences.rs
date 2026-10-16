use actix_session::Session;
use actix_web::{HttpResponse, Responder, delete, web};
use sqlx::PgPool;

use crate::date;
use crate::routes::api::snapshot::session_user_id;

pub const MONTH_ABBREVIATIONS: [&str; 12] = [
    "ENE", "FEB", "MAR", "ABR", "MAY", "JUN", "JUL", "AGO", "SEP", "OCT", "NOV", "DIC",
];

/// Último día de un mes civil (con bisiesto).
pub fn last_day_of_month(year: i64, month: i64) -> i64 {
    let bisiesto = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let por_mes = [
        31,
        if bisiesto { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    por_mes[(month - 1) as usize]
}

#[derive(sqlx::FromRow)]
struct ActiveRecurrence {
    id: i32,
    account_id: i32,
    category_id: Option<i32>,
    category_label: String,
    description: String,
    amount: f64,
    is_income: bool,
    currency: String,
    day_of_month: i16,
    start_date: String,
    installment_count: Option<i16>,
    installments_generated: i16,
}

/// Materializa los asientos futuros que falten para cada recurrencia
/// activa del usuario. Idempotente: el cursor es cuotas_generadas.
///
/// - Indefinida (cuotas NULL): genera los meses restantes del año
///   calendario en curso; el año siguiente rueda solo.
/// - Con cuotas: genera hasta agotar la cantidad total, cruzando años.
pub async fn materialize(
    conn: &mut sqlx::PgConnection,
    user_id: i32,
    today_iso: &str,
) -> sqlx::Result<()> {
    let recurrences: Vec<ActiveRecurrence> = sqlx::query_as(
        "SELECT id, account_id, category_id, category_label, description,
            amount::float8 AS amount, is_income, currency, day_of_month,
            to_char(start_date, 'YYYY-MM-DD') AS start_date, installment_count, installments_generated
        FROM recurrences WHERE user_id = $1 AND is_active",
    )
    .bind(user_id)
    .fetch_all(&mut *conn)
    .await?;

    let current_year: i64 = today_iso[0..4].parse().unwrap_or(2026);

    for r in &recurrences {
        let start_year: i64 = r.start_date[0..4].parse().unwrap_or(current_year);
        let start_month: i64 = r.start_date[5..7].parse().unwrap_or(1);
        let base = start_year * 12 + (start_month - 1);
        let source_day = i64::from(r.day_of_month);

        let mut inserted = 0;
        let mut k = i64::from(r.installments_generated) + 1;
        loop {
            if r.installment_count.is_some_and(|c| k > i64::from(c)) {
                break;
            }
            let target = base + (k - 1);
            let year = target / 12;
            let month = target % 12 + 1;
            if r.installment_count.is_none() && year > current_year {
                break;
            }
            let day_of_month = source_day.min(last_day_of_month(year, month));
            let entry_date = format!("{year:04}-{month:02}-{day_of_month:02}");
            let description = if r.installment_count.is_none() {
                format!(
                    "{} · {}",
                    r.description,
                    MONTH_ABBREVIATIONS[(month - 1) as usize]
                )
            } else {
                format!(
                    "Cuota {}/{} · {}",
                    k,
                    r.installment_count.unwrap_or(0),
                    r.description
                )
            };
            sqlx::query(
                "INSERT INTO transactions
                    (user_id, account_id, category_id, category_label, description,
                     amount, is_income, currency, entry_date, recurrence_id)
                VALUES ($1, $2, $3, $4, $5, $6::numeric, $7, $8, $9::date, $10)",
            )
            .bind(user_id)
            .bind(r.account_id)
            .bind(r.category_id)
            .bind(&r.category_label)
            .bind(&description)
            .bind(r.amount)
            .bind(r.is_income)
            .bind(&r.currency)
            .bind(&entry_date)
            .bind(r.id)
            .execute(&mut *conn)
            .await?;
            inserted += 1;
            k += 1;
        }
        if inserted > 0 {
            sqlx::query(
                "UPDATE recurrences SET installments_generated = installments_generated + $1 WHERE id = $2",
            )
            .bind(i16::try_from(inserted).unwrap_or(i16::MAX))
            .bind(r.id)
            .execute(&mut *conn)
            .await?;
        }
    }
    Ok(())
}

/// Borrar una recurrencia: corta los asientos futuros y conserva los ya
/// cargados (el FK con SET NULL les saca el rec_id al caer la rec).
#[delete("/api/recurrences/{id}")]
pub async fn recurrence_delete(
    path: web::Path<i32>,
    session: Session,
    pool: web::Data<PgPool>,
) -> impl Responder {
    let Some(user_id) = session_user_id(&session) else {
        return HttpResponse::Unauthorized().finish();
    };
    let mut conn = match pool.get_ref().acquire().await {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Error de conexión"),
    };
    match delete_recurrence(&mut conn, user_id, path.into_inner()).await {
        Ok((generated, _)) => {
            HttpResponse::Ok().json(serde_json::json!({ "deletedScheduled": generated }))
        }
        Err(_) => HttpResponse::InternalServerError().body("Error al borrar la recurrencia"),
    }
}

/// Lógica pura del borrado de recurrencia (testeable en transacción).
pub async fn delete_recurrence(
    conn: &mut sqlx::PgConnection,
    user_id: i32,
    recurrence_id: i32,
) -> sqlx::Result<(u64, u64)> {
    let today = date::today_iso();
    let generated = sqlx::query(
        "DELETE FROM transactions WHERE recurrence_id = $1 AND user_id = $2 AND entry_date > $3::date",
    )
    .bind(recurrence_id)
    .bind(user_id)
    .bind(&today)
    .execute(&mut *conn)
    .await?;
    let restante = sqlx::query("DELETE FROM recurrences WHERE id = $1 AND user_id = $2")
        .bind(recurrence_id)
        .bind(user_id)
        .execute(&mut *conn)
        .await?;
    Ok((generated.rows_affected(), restante.rows_affected()))
}

#[cfg(test)]
mod tests {
    use super::{MONTH_ABBREVIATIONS, last_day_of_month, materialize};
    use sqlx::Connection;
    use sqlx::PgConnection;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// El rol de la base de .env no tiene CREATEDB: #[sqlx::test] (una
    /// base por test) no es posible en esta máquina. Cada test corre en
    /// una transacción que se revierte al final; la base queda intacta.
    /// Si el dueño habilita ALTER ROLE ... CREATEDB, pueden migrar a
    /// #[sqlx::test] tal como sugiere AGENTS.md.
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
        let email = format!("test-rec-{nanos}@rfinance.local");
        let (id,): (i32,) =
            sqlx::query_as("INSERT INTO users (email, password) VALUES ($1, 'x') RETURNING id")
                .bind(&email)
                .fetch_one(&mut *tx)
                .await
                .expect("user de test");
        id
    }

    async fn seed_recurrence(
        tx: &mut PgConnection,
        user_id: i32,
        start_date: &str,
        installment_count: Option<i16>,
        generated: i16,
    ) -> i32 {
        let (cid,): (i32,) = sqlx::query_as(
            "INSERT INTO accounts (user_id, name, currency) VALUES ($1, 'Banco', 'ARS') RETURNING id",
        )
        .bind(user_id)
        .fetch_one(&mut *tx)
        .await
        .expect("cuenta de test");
        let (id,): (i32,) = sqlx::query_as(
            "INSERT INTO recurrences
                (user_id, account_id, category_id, category_label, description, amount,
                 is_income, currency, day_of_month, start_date, installment_count, installments_generated)
            VALUES ($1, $2, NULL, 'Alquiler', 'Alquiler', 320000, false, 'ARS', 12, $3::date, $4, $5)
            RETURNING id",
        )
        .bind(user_id)
        .bind(cid)
        .bind(start_date)
        .bind(installment_count)
        .bind(generated)
        .fetch_one(&mut *tx)
        .await
        .expect("recurrencia de test");
        // el origen ya cargado, como lo crea el POST de asientos
        sqlx::query(
            "INSERT INTO transactions
                (user_id, account_id, category_id, category_label, description,
                 amount, is_income, currency, entry_date, recurrence_id)
            VALUES ($1, $2, NULL, 'Alquiler', 'Alquiler', 320000::numeric, false, 'ARS', $3::date, $4)",
        )
        .bind(user_id)
        .bind(cid)
        .bind(start_date)
        .bind(id)
        .execute(&mut *tx)
        .await
        .expect("asiento origen de test");
        id
    }

    async fn count_for_recurrence(tx: &mut PgConnection, recurrence_id: i32) -> i64 {
        let (n,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM transactions WHERE recurrence_id = $1")
                .bind(recurrence_id)
                .fetch_one(&mut *tx)
                .await
                .expect("conteo");
        n
    }

    async fn generated_for(tx: &mut PgConnection, recurrence_id: i32) -> i16 {
        let (g,): (i16,) =
            sqlx::query_as("SELECT installments_generated FROM recurrences WHERE id = $1")
                .bind(recurrence_id)
                .fetch_one(&mut *tx)
                .await
                .expect("cursor");
        g
    }

    #[test]
    fn last_day_of_month_civil() {
        assert_eq!(last_day_of_month(2026, 2), 28);
        assert_eq!(last_day_of_month(2024, 2), 29);
        assert_eq!(last_day_of_month(2026, 12), 31);
        assert_eq!(MONTH_ABBREVIATIONS[11], "DIC");
    }

    #[async_std::test]
    async fn open_ended_generates_until_year_end() {
        let mut conn = connect().await;
        let mut tx = conn.begin().await.expect("tx");
        let user = make_user(&mut tx).await;
        let recurrence = seed_recurrence(&mut tx, user, "2026-10-12", None, 1).await;

        materialize(&mut tx, user, "2026-10-15")
            .await
            .expect("materializar");

        // OCT (origen) + NOV + DIC del año calendario 2026, y nada más
        assert_eq!(count_for_recurrence(&mut tx, recurrence).await, 3);
        assert_eq!(generated_for(&mut tx, recurrence).await, 3);

        // idempotente: otra llamada en el mismo año no agrega nada
        materialize(&mut tx, user, "2026-11-20")
            .await
            .expect("idempotente");
        assert_eq!(count_for_recurrence(&mut tx, recurrence).await, 3);
        assert_eq!(generated_for(&mut tx, recurrence).await, 3);

        tx.rollback().await.expect("rollback");
    }

    #[async_std::test]
    async fn installments_across_year() {
        let mut conn = connect().await;
        let mut tx = conn.begin().await.expect("tx");
        let user = make_user(&mut tx).await;
        let recurrence = seed_recurrence(&mut tx, user, "2026-12-12", Some(12), 1).await;

        materialize(&mut tx, user, "2026-10-15")
            .await
            .expect("materializar");

        // las 12 cuotas quedan programadas aunque crucen el año
        assert_eq!(count_for_recurrence(&mut tx, recurrence).await, 12);
        assert_eq!(generated_for(&mut tx, recurrence).await, 12);

        // agotada: otra llamada no agrega nada ni siquiera el año próximo
        materialize(&mut tx, user, "2027-02-01")
            .await
            .expect("agotada");
        assert_eq!(count_for_recurrence(&mut tx, recurrence).await, 12);

        tx.rollback().await.expect("rollback");
    }
}
