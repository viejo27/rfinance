use actix_session::Session;
use actix_web::{HttpResponse, Responder, delete, patch, post, web};
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::routes::api::snapshot::session_user_id;
use crate::routes::api::transactions::{
    MAX_DESCRIPTION_LENGTH, MAX_FINANCIAL_VALUE, is_valid_date,
};

/// Payload del form de captura de una transferencia. Las monedas no viajan:
/// se leen de las cuentas en la base, el cliente no las elige.
#[derive(Deserialize)]
pub struct TransferInput {
    pub entry_date: String,
    pub from_account_id: Uuid,
    pub to_account_id: Uuid,
    pub from_amount: f64,
    pub to_amount: f64,
    pub description: String,
}

/// Compara montos al centavo: la base es NUMERIC(14,2) y el form manda
/// enteros o decimales equivalentes (100 vs 100.00).
fn same_amount(a: f64, b: f64) -> bool {
    (a * 100.0).round() as i64 == (b * 100.0).round() as i64
}

fn validate_input(input: &TransferInput) -> Result<(), String> {
    if !is_valid_date(&input.entry_date) {
        return Err("La fecha debe ser un día real en formato AAAA-MM-DD".into());
    }
    if input.description.trim().is_empty() {
        return Err("El concepto es obligatorio".into());
    }
    if input.description.chars().count() > MAX_DESCRIPTION_LENGTH {
        return Err("El concepto no puede superar los 200 caracteres".into());
    }
    if !(input.from_amount > 0.0 && input.from_amount.is_finite()) {
        return Err("El monto enviado debe ser mayor que cero".into());
    }
    if !(input.to_amount > 0.0 && input.to_amount.is_finite()) {
        return Err("El monto recibido debe ser mayor que cero".into());
    }
    if input.from_amount > MAX_FINANCIAL_VALUE || input.to_amount > MAX_FINANCIAL_VALUE {
        return Err("El monto excede el máximo permitido".into());
    }
    if input.from_account_id == input.to_account_id {
        return Err("Las cuentas de origen y destino deben ser distintas".into());
    }
    Ok(())
}

/// Misma moneda en las dos puntas: el monto recibido tiene que ser igual al
/// enviado. Entre monedas distintas los montos son libres y la tasa queda
/// implícita en ellos.
fn validate_currencies(
    from_currency: &str,
    to_currency: &str,
    input: &TransferInput,
) -> Result<(), String> {
    if from_currency == to_currency && !same_amount(input.from_amount, input.to_amount) {
        return Err(
            "Entre cuentas de la misma moneda el monto recibido debe ser igual al enviado".into(),
        );
    }
    Ok(())
}

/// Resuelve el public_id de una cuenta a (id serial, moneda).
async fn resolve_account(
    conn: &mut sqlx::PgConnection,
    user_id: i32,
    public_id: Uuid,
) -> Result<(i32, String), String> {
    let row: Option<(i32, String)> =
        sqlx::query_as("SELECT id, currency FROM accounts WHERE public_id = $1 AND user_id = $2")
            .bind(public_id)
            .bind(user_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|_| "Error al buscar la cuenta".to_string())?;
    row.ok_or_else(|| "Cuenta no encontrada".to_string())
}

/// Alta de transferencia. Devuelve el public_id (UUID4) creado.
pub async fn create_transfer(
    conn: &mut sqlx::PgConnection,
    user_id: i32,
    input: &TransferInput,
) -> Result<Uuid, String> {
    validate_input(input)?;
    let description = input.description.trim();

    let (from_id, from_currency) =
        resolve_account(&mut *conn, user_id, input.from_account_id).await?;
    let (to_id, to_currency) = resolve_account(&mut *conn, user_id, input.to_account_id).await?;
    validate_currencies(&from_currency, &to_currency, input)?;

    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO transfers
            (user_id, from_account_id, to_account_id, from_amount, from_currency,
             to_amount, to_currency, entry_date, description)
        VALUES ($1, $2, $3, $4::numeric, $5, $6::numeric, $7, $8::date, $9)
        RETURNING public_id",
    )
    .bind(user_id)
    .bind(from_id)
    .bind(to_id)
    .bind(input.from_amount)
    .bind(&from_currency)
    .bind(input.to_amount)
    .bind(&to_currency)
    .bind(&input.entry_date)
    .bind(description)
    .fetch_one(&mut *conn)
    .await
    .map_err(|_| "Error al crear la transferencia".to_string())?;
    Ok(row.0)
}

/// Edición completa de la transferencia (mismos campos que el alta).
pub async fn update_transfer(
    conn: &mut sqlx::PgConnection,
    user_id: i32,
    transfer_id: i32,
    input: &TransferInput,
) -> Result<(), String> {
    validate_input(input)?;
    let description = input.description.trim();

    let (from_id, from_currency) =
        resolve_account(&mut *conn, user_id, input.from_account_id).await?;
    let (to_id, to_currency) = resolve_account(&mut *conn, user_id, input.to_account_id).await?;
    validate_currencies(&from_currency, &to_currency, input)?;

    let res = sqlx::query(
        "UPDATE transfers SET from_account_id = $3, to_account_id = $4,
            from_amount = $5::numeric, from_currency = $6,
            to_amount = $7::numeric, to_currency = $8,
            entry_date = $9::date, description = $10
        WHERE id = $1 AND user_id = $2",
    )
    .bind(transfer_id)
    .bind(user_id)
    .bind(from_id)
    .bind(to_id)
    .bind(input.from_amount)
    .bind(&from_currency)
    .bind(input.to_amount)
    .bind(&to_currency)
    .bind(&input.entry_date)
    .bind(description)
    .execute(&mut *conn)
    .await
    .map_err(|_| "Error al guardar la transferencia".to_string())?;
    if res.rows_affected() == 0 {
        return Err("Transferencia no encontrada".into());
    }
    Ok(())
}

/// Borrar la transferencia completa (no tiene patas sueltas).
pub async fn delete_transfer(
    conn: &mut sqlx::PgConnection,
    user_id: i32,
    transfer_id: i32,
) -> sqlx::Result<u64> {
    let res = sqlx::query("DELETE FROM transfers WHERE id = $1 AND user_id = $2")
        .bind(transfer_id)
        .bind(user_id)
        .execute(&mut *conn)
        .await?;
    Ok(res.rows_affected())
}

fn error_response(msg: String) -> HttpResponse {
    if msg.starts_with("Error al") {
        HttpResponse::InternalServerError().body(msg)
    } else {
        HttpResponse::BadRequest().body(msg)
    }
}

#[post("/api/transfers")]
pub async fn transfer_create(
    body: web::Json<TransferInput>,
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
    match create_transfer(&mut conn, user_id, &body).await {
        Ok(id) => HttpResponse::Created().json(serde_json::json!({ "id": id })),
        Err(msg) => error_response(msg),
    }
}

#[patch("/api/transfers/{id}")]
pub async fn transfer_update(
    path: web::Path<Uuid>,
    body: web::Json<TransferInput>,
    session: Session,
    pool: web::Data<PgPool>,
) -> impl Responder {
    let Some(user_id) = session_user_id(&session) else {
        return HttpResponse::Unauthorized().finish();
    };
    let public_id = path.into_inner();
    let mut conn = match pool.get_ref().acquire().await {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Error de conexión"),
    };
    let internal: Option<(i32,)> =
        sqlx::query_as("SELECT id FROM transfers WHERE public_id = $1 AND user_id = $2")
            .bind(public_id)
            .bind(user_id)
            .fetch_optional(&mut *conn)
            .await
            .unwrap_or(None);
    let Some((transfer_id,)) = internal else {
        return HttpResponse::NotFound().finish();
    };
    match update_transfer(&mut conn, user_id, transfer_id, &body).await {
        Ok(()) => HttpResponse::Ok().json(serde_json::json!({ "id": public_id })),
        Err(msg) => error_response(msg),
    }
}

#[delete("/api/transfers/{id}")]
pub async fn transfer_delete(
    path: web::Path<Uuid>,
    session: Session,
    pool: web::Data<PgPool>,
) -> impl Responder {
    let Some(user_id) = session_user_id(&session) else {
        return HttpResponse::Unauthorized().finish();
    };
    let public_id = path.into_inner();
    let mut conn = match pool.get_ref().acquire().await {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Error de conexión"),
    };
    let internal: Option<(i32,)> =
        sqlx::query_as("SELECT id FROM transfers WHERE public_id = $1 AND user_id = $2")
            .bind(public_id)
            .bind(user_id)
            .fetch_optional(&mut *conn)
            .await
            .unwrap_or(None);
    let Some((transfer_id,)) = internal else {
        return HttpResponse::NotFound().finish();
    };
    match delete_transfer(&mut conn, user_id, transfer_id).await {
        Ok(0) => HttpResponse::NotFound().finish(),
        Ok(_) => HttpResponse::Ok().body("Transferencia borrada"),
        Err(_) => HttpResponse::InternalServerError().body("Error al borrar la transferencia"),
    }
}

#[cfg(test)]
mod tests {
    use super::{TransferInput, create_transfer, delete_transfer, update_transfer, validate_input};
    use crate::routes::api::snapshot::build_snapshot;
    use sqlx::Connection;
    use sqlx::PgConnection;
    use std::time::{SystemTime, UNIX_EPOCH};
    use uuid::Uuid;

    fn input(from: Uuid, to: Uuid, from_amount: f64, to_amount: f64) -> TransferInput {
        TransferInput {
            entry_date: "2026-10-07".into(),
            from_account_id: from,
            to_account_id: to,
            from_amount,
            to_amount,
            description: "dolares a pesos".into(),
        }
    }

    // ---------- validaciones (puras, sin base) ----------

    #[test]
    fn cuentas_distintas_y_montos() {
        let from = Uuid::new_v4();
        let to = Uuid::new_v4();
        assert!(validate_input(&input(from, to, 2500.0, 3_959_325.0)).is_ok());

        let same = input(from, from, 100.0, 100.0);
        assert_eq!(
            validate_input(&same).err().as_deref(),
            Some("Las cuentas de origen y destino deben ser distintas")
        );

        let mut cero = input(from, to, 0.0, 100.0);
        assert!(validate_input(&cero).is_err());
        cero.from_amount = -1.0;
        assert!(validate_input(&cero).is_err());
        cero.from_amount = f64::NAN;
        assert!(validate_input(&cero).is_err());

        let mut caro = input(from, to, 100.0, 100.0);
        caro.to_amount = 1_000_000_000_000.0;
        assert!(validate_input(&caro).is_err());

        let mut sin_concepto = input(from, to, 100.0, 100.0);
        sin_concepto.description = "   ".into();
        assert!(validate_input(&sin_concepto).is_err());
    }

    #[test]
    fn mismo_monto_al_centavo() {
        assert!(super::same_amount(100.0, 100.0));
        assert!(super::same_amount(100.0, 100.004));
        assert!(!super::same_amount(100.0, 100.01));
    }

    // ---------- integración (se revierte) ----------

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
        let email = format!("test-transfer-{nanos}@rfinance.local");
        let (id,): (i32,) =
            sqlx::query_as("INSERT INTO users (email, password) VALUES ($1, 'x') RETURNING id")
                .bind(&email)
                .fetch_one(&mut *tx)
                .await
                .expect("user de test");
        id
    }

    async fn make_account(tx: &mut PgConnection, user_id: i32, name: &str, currency: &str) -> Uuid {
        let (id,): (Uuid,) = sqlx::query_as(
            "INSERT INTO accounts (user_id, name, currency)
            VALUES ($1, $2, $3) RETURNING public_id",
        )
        .bind(user_id)
        .bind(name)
        .bind(currency)
        .fetch_one(&mut *tx)
        .await
        .expect("cuenta de test");
        id
    }

    #[async_std::test]
    async fn transferencia_mueve_saldos_y_no_es_asiento() {
        let mut conn = connect().await;
        let mut tx = conn.begin().await.expect("tx");
        let user = make_user(&mut tx).await;
        let usd = make_account(&mut tx, user, "Wallbit", "USD").await;
        let ars = make_account(&mut tx, user, "Galicia", "ARS").await;

        let public_id = create_transfer(&mut tx, user, &input(usd, ars, 2500.0, 3_959_325.0))
            .await
            .expect("alta de transferencia");
        assert!(!public_id.is_nil());

        let snapshot = build_snapshot(&mut tx, user, 2026)
            .await
            .expect("snapshot 2026");
        assert_eq!(snapshot.transfers.len(), 1);
        assert_eq!(snapshot.transfers[0].from_currency, "USD");
        assert_eq!(snapshot.transfers[0].to_currency, "ARS");
        // La transferencia no contamina el libro de asientos.
        assert!(snapshot.transactions.is_empty());

        let usd_row = snapshot
            .accounts
            .iter()
            .find(|a| a.id == usd)
            .expect("cuenta USD");
        let ars_row = snapshot
            .accounts
            .iter()
            .find(|a| a.id == ars)
            .expect("cuenta ARS");
        assert!((usd_row.balance - -2500.0).abs() < 0.001);
        assert!((ars_row.balance - 3_959_325.0).abs() < 0.001);

        tx.rollback().await.expect("rollback");
    }

    #[async_std::test]
    async fn misma_moneda_exige_montos_iguales() {
        let mut conn = connect().await;
        let mut tx = conn.begin().await.expect("tx");
        let user = make_user(&mut tx).await;
        let uno = make_account(&mut tx, user, "Brubank", "ARS").await;
        let dos = make_account(&mut tx, user, "Galicia", "ARS").await;

        let err = create_transfer(&mut tx, user, &input(uno, dos, 100.0, 90.0))
            .await
            .expect_err("misma moneda con montos distintos");
        assert!(err.contains("misma moneda"));

        // Con montos iguales entra.
        create_transfer(&mut tx, user, &input(uno, dos, 90.0, 90.0))
            .await
            .expect("misma moneda con montos iguales");

        tx.rollback().await.expect("rollback");
    }

    #[async_std::test]
    async fn cuenta_ajena_no_se_resuelve() {
        let mut conn = connect().await;
        let mut tx = conn.begin().await.expect("tx");
        let user = make_user(&mut tx).await;
        let propia = make_account(&mut tx, user, "Wallbit", "USD").await;
        let ajena = Uuid::new_v4();

        let err = create_transfer(&mut tx, user, &input(propia, ajena, 100.0, 100.0))
            .await
            .expect_err("cuenta inexistente");
        assert_eq!(err, "Cuenta no encontrada");

        tx.rollback().await.expect("rollback");
    }

    #[async_std::test]
    async fn editar_y_borrar_transferencia() {
        let mut conn = connect().await;
        let mut tx = conn.begin().await.expect("tx");
        let user = make_user(&mut tx).await;
        let usd = make_account(&mut tx, user, "Wallbit", "USD").await;
        let ars = make_account(&mut tx, user, "Galicia", "ARS").await;

        let public_id = create_transfer(&mut tx, user, &input(usd, ars, 2500.0, 3_959_325.0))
            .await
            .expect("alta");
        let (internal_id,): (i32,) =
            sqlx::query_as("SELECT id FROM transfers WHERE public_id = $1")
                .bind(public_id)
                .fetch_one(&mut *tx)
                .await
                .expect("id interno");

        update_transfer(
            &mut tx,
            user,
            internal_id,
            &input(usd, ars, 2000.0, 3_000_000.0),
        )
        .await
        .expect("edición");
        let (from_amount, to_amount): (f64, f64) = sqlx::query_as(
            "SELECT from_amount::float8, to_amount::float8 FROM transfers WHERE id = $1",
        )
        .bind(internal_id)
        .fetch_one(&mut *tx)
        .await
        .expect("montos guardados");
        assert!((from_amount - 2000.0).abs() < 0.001);
        assert!((to_amount - 3_000_000.0).abs() < 0.001);

        assert_eq!(
            delete_transfer(&mut tx, user, internal_id)
                .await
                .expect("borrar"),
            1
        );
        assert_eq!(
            delete_transfer(&mut tx, user, internal_id)
                .await
                .expect("borrar de nuevo"),
            0
        );

        tx.rollback().await.expect("rollback");
    }
}
