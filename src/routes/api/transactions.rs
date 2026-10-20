use actix_session::Session;
use actix_web::{HttpResponse, Responder, delete, patch, post, web};
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::routes::api::recurrences::materialize;
use crate::routes::api::snapshot::session_user_id;

#[derive(Deserialize)]
pub struct RecurrenceInput {
    pub installment_count: Option<i16>,
}

/// Alcance de la edición de un asiento que pertenece a una recurrencia.
#[derive(Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    /// Cambiar solo esta fila; la serie y sus futuros quedan intactos.
    Single,
    /// Cambiar esta fila y re-plantillar las que siguen después de ella.
    Following,
}

/// Payload del form de captura (alta y edición mandan el mismo shape).
#[derive(Deserialize)]
pub struct TransactionInput {
    pub entry_date: String,
    pub account_id: Uuid,
    pub category_id: Option<Uuid>,
    pub description: String,
    pub amount: f64,
    pub is_income: bool,
    pub currency: String,
    pub recurrence: Option<RecurrenceInput>,
    /// Solo lo mira el PATCH de asientos de recurrencia; ausente = Unica.
    #[serde(default)]
    pub scope: Option<Scope>,
}

fn is_valid_currency(currency: &str) -> bool {
    currency == "ARS" || currency == "USD"
}

/// Valida un ISO YYYY-MM-DD parseable y con mes/día en rango.
fn is_valid_date(entry_date: &str) -> bool {
    let b = entry_date.as_bytes();
    if entry_date.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    let (y, m, d) = (
        entry_date[0..4].parse::<i64>().unwrap_or(0),
        entry_date[5..7].parse::<i64>().unwrap_or(0),
        entry_date[8..10].parse::<i64>().unwrap_or(0),
    );
    y >= 2000 && (1..=12).contains(&m) && (1..=31).contains(&d)
}

fn day_of(entry_date: &str) -> i16 {
    entry_date[8..10].parse::<i16>().unwrap_or(1).min(28)
}

/// Meses civiles completos entre dos ISO YYYY-MM-DD (0 si caen en el mismo mes).
fn months_between(desde: &str, hasta: &str) -> i16 {
    let indice = |f: &str| -> i32 {
        let year: i32 = f[0..4].parse().unwrap_or(0);
        let month: i32 = f[5..7].parse().unwrap_or(1);
        year * 12 + month
    };
    (indice(hasta) - indice(desde)).max(0) as i16
}

fn validate_common(input: &TransactionInput) -> Result<(), String> {
    if !is_valid_date(&input.entry_date) {
        return Err("La fecha debe ser un día real en formato AAAA-MM-DD".into());
    }
    if input.description.trim().is_empty() {
        return Err("El concepto es obligatorio".into());
    }
    if !(input.amount > 0.0 && input.amount.is_finite()) {
        return Err("El monto debe ser mayor que cero".into());
    }
    if !is_valid_currency(&input.currency) {
        return Err("La moneda debe ser ARS o USD".into());
    }
    Ok(())
}

/// Resuelve el public_id (UUID4) de una cuenta al id serial interno.
async fn resolve_account(
    conn: &mut sqlx::PgConnection,
    user_id: i32,
    public_id: Uuid,
) -> Result<i32, String> {
    let row: Option<(i32,)> =
        sqlx::query_as("SELECT id FROM accounts WHERE public_id = $1 AND user_id = $2")
            .bind(public_id)
            .bind(user_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|_| "Error al buscar la cuenta".to_string())?;
    row.map(|(id,)| id)
        .ok_or_else(|| "Cuenta no encontrada".to_string())
}

/// Resuelve el public_id de una categoría (opcional) al id serial interno.
async fn resolve_category(
    conn: &mut sqlx::PgConnection,
    user_id: i32,
    public_id: Option<Uuid>,
) -> Result<Option<i32>, String> {
    let Some(public_id) = public_id else {
        return Ok(None);
    };
    let row: Option<(i32,)> =
        sqlx::query_as("SELECT id FROM categories WHERE public_id = $1 AND user_id = $2")
            .bind(public_id)
            .bind(user_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|_| "Error al buscar la categoría".to_string())?;
    row.map(|(id,)| Some(id))
        .ok_or_else(|| "Categoría no encontrada".to_string())
}

async fn category_label_for(
    conn: &mut sqlx::PgConnection,
    user_id: i32,
    category_id: Option<i32>,
) -> Result<String, String> {
    match category_id {
        None => Ok("SIN CATEGORÍA".to_string()),
        Some(id) => {
            let row: Option<(String,)> =
                sqlx::query_as("SELECT name FROM categories WHERE id = $1 AND user_id = $2")
                    .bind(id)
                    .bind(user_id)
                    .fetch_optional(&mut *conn)
                    .await
                    .map_err(|_| "Error al buscar la categoría".to_string())?;
            row.map(|(n,)| n)
                .ok_or_else(|| "Categoría no encontrada".to_string())
        }
    }
}

/// Alta de asiento; con `rec` crea la recurrencia y materializa los
/// futuros del año. Devuelve (public_id del asiento, futuros generados).
pub async fn create_transaction(
    conn: &mut sqlx::PgConnection,
    user_id: i32,
    input: &TransactionInput,
) -> Result<(Uuid, i64), String> {
    validate_common(input)?;
    let description = input.description.trim().to_string();

    let account_id = resolve_account(&mut *conn, user_id, input.account_id).await?;
    let category_id = resolve_category(&mut *conn, user_id, input.category_id).await?;
    let category_label = category_label_for(&mut *conn, user_id, category_id).await?;

    let mut recurrence_id: Option<i32> = None;
    if input.recurrence.is_some() {
        let row: (i32,) = sqlx::query_as(
            "INSERT INTO recurrences
                (user_id, account_id, category_id, category_label, description,
                 amount, is_income, currency, day_of_month, start_date, installment_count, installments_generated)
            VALUES ($1, $2, $3, $4, $5, $6::numeric, $7, $8, $9, $10::date, $11, 1)
            RETURNING id",
        )
        .bind(user_id)
        .bind(account_id)
        .bind(category_id)
        .bind(&category_label)
        .bind(&description)
        .bind(input.amount)
        .bind(input.is_income)
        .bind(&input.currency)
        .bind(day_of(&input.entry_date))
        .bind(&input.entry_date)
        .bind(input.recurrence.as_ref().and_then(|r| r.installment_count))
        .fetch_one(&mut *conn)
        .await
        .map_err(|_| "Error al crear la recurrencia".to_string())?;
        recurrence_id = Some(row.0);
    }

    let row: (i32, Uuid) = sqlx::query_as(
        "INSERT INTO transactions
            (user_id, account_id, category_id, category_label, description,
             amount, is_income, currency, entry_date, recurrence_id)
        VALUES ($1, $2, $3, $4, $5, $6::numeric, $7, $8, $9::date, $10)
        RETURNING id, public_id",
    )
    .bind(user_id)
    .bind(account_id)
    .bind(category_id)
    .bind(&category_label)
    .bind(&description)
    .bind(input.amount)
    .bind(input.is_income)
    .bind(&input.currency)
    .bind(&input.entry_date)
    .bind(recurrence_id)
    .fetch_one(&mut *conn)
    .await
    .map_err(|_| "Error al crear el asiento".to_string())?;

    let generated = if let Some(rid) = recurrence_id {
        materialize(&mut *conn, user_id, &crate::date::today_iso())
            .await
            .map_err(|_| "Error al materializar la recurrencia".to_string())?;
        let (n,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM transactions WHERE recurrence_id = $1 AND id <> $2",
        )
        .bind(rid)
        .bind(row.0)
        .fetch_one(&mut *conn)
        .await
        .map_err(|_| "Error al contar los futuros".to_string())?;
        n
    } else {
        0
    };
    Ok((row.1, generated))
}

/// UPDATE de la fila del asiento, con el rec_id explícito (None = NULL).
/// Datos ya resueltos (ids internos) para el UPDATE de la fila del asiento.
struct RowUpdate<'a> {
    account_id: i32,
    category_id: Option<i32>,
    description: &'a str,
    category_label: &'a str,
    recurrence_id: Option<i32>,
}

async fn update_transaction_row(
    conn: &mut sqlx::PgConnection,
    user_id: i32,
    transaction_id: i32,
    input: &TransactionInput,
    patch: &RowUpdate<'_>,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE transactions SET account_id = $3, category_id = $4, category_label = $5,
            description = $6, amount = $7::numeric, is_income = $8, currency = $9,
            entry_date = $10::date, recurrence_id = $11
        WHERE id = $1 AND user_id = $2",
    )
    .bind(transaction_id)
    .bind(user_id)
    .bind(patch.account_id)
    .bind(patch.category_id)
    .bind(patch.category_label)
    .bind(patch.description)
    .bind(input.amount)
    .bind(input.is_income)
    .bind(&input.currency)
    .bind(&input.entry_date)
    .bind(patch.recurrence_id)
    .execute(&mut *conn)
    .await
    .map_err(|_| "Error al guardar el asiento".to_string())?;
    Ok(())
}

/// Edición de asiento. Cuando la fila pertenece a una recurrencia decide
/// `alcance`: `Unica` (default si no viene) cambia solo esta fila sin tocar
/// la serie; `Siguientes` borra las posteriores a ESTA fila (en función de su
/// fecha, nunca de "hoy"), re-plantilla la recurrencia desde aquí y vuelve a
/// materializar los futuros. Con `Siguientes` y sin `rec`, la serie termina
/// acá. Para filas sin recurrencia se ignora `alcance`; con `rec` arranca
/// una serie nueva desde esta fila. Devuelve cuántos asientos futuros quedó
/// generando la serie (0 si no aplica).
pub async fn update_transaction(
    conn: &mut sqlx::PgConnection,
    user_id: i32,
    transaction_id: i32,
    input: &TransactionInput,
) -> Result<i64, String> {
    validate_common(input)?;
    let description = input.description.trim().to_string();

    let previous: Option<(Option<i32>, String)> = sqlx::query_as(
        "SELECT recurrence_id, to_char(entry_date, 'YYYY-MM-DD')
        FROM transactions WHERE id = $1 AND user_id = $2",
    )
    .bind(transaction_id)
    .bind(user_id)
    .fetch_optional(&mut *conn)
    .await
    .map_err(|_| "Error al buscar el asiento".to_string())?;
    let (previous_recurrence_id, original_date) =
        previous.ok_or_else(|| "Asiento no encontrado".to_string())?;

    let account_id = resolve_account(&mut *conn, user_id, input.account_id).await?;
    let category_id = resolve_category(&mut *conn, user_id, input.category_id).await?;
    let category_label = category_label_for(&mut *conn, user_id, category_id).await?;

    let Some(rid) = previous_recurrence_id else {
        let Some(recurrence_input) = input.recurrence.as_ref() else {
            // asiento simple editado como simple
            update_transaction_row(
                conn,
                user_id,
                transaction_id,
                input,
                &RowUpdate {
                    account_id,
                    category_id,
                    description: &description,
                    category_label: &category_label,
                    recurrence_id: None,
                },
            )
            .await?;
            return Ok(0);
        };
        // el asiento no tenía recurrencia: crearla desde esta fila y materializar
        let row: (i32,) = sqlx::query_as(
            "INSERT INTO recurrences
                (user_id, account_id, category_id, category_label, description,
                 amount, is_income, currency, day_of_month, start_date, installment_count, installments_generated)
            VALUES ($1, $2, $3, $4, $5, $6::numeric, $7, $8, $9, $10::date, $11, 1)
            RETURNING id",
        )
        .bind(user_id)
        .bind(account_id)
        .bind(category_id)
        .bind(&category_label)
        .bind(&description)
        .bind(input.amount)
        .bind(input.is_income)
        .bind(&input.currency)
        .bind(day_of(&input.entry_date))
        .bind(&input.entry_date)
        .bind(recurrence_input.installment_count)
        .fetch_one(&mut *conn)
        .await
        .map_err(|_| "Error al crear la recurrencia".to_string())?;
        update_transaction_row(
            conn,
            user_id,
            transaction_id,
            input,
            &RowUpdate {
                account_id,
                category_id,
                description: &description,
                category_label: &category_label,
                recurrence_id: Some(row.0),
            },
        )
        .await?;
        materialize(&mut *conn, user_id, &crate::date::today_iso())
            .await
            .map_err(|_| "Error al materializar la recurrencia".to_string())?;
        let (n,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM transactions WHERE recurrence_id = $1 AND user_id = $2 AND id <> $3",
        )
        .bind(row.0)
        .bind(user_id)
        .bind(transaction_id)
        .fetch_one(&mut *conn)
        .await
        .map_err(|_| "Error al contar los futuros".to_string())?;
        return Ok(n);
    };

    if input.scope != Some(Scope::Following) {
        // Unica: solo esta fila. La serie, sus futuros y las filas anteriores
        // quedan intactas (era el bug: antes se cortaba todo "desde hoy").
        update_transaction_row(
            conn,
            user_id,
            transaction_id,
            input,
            &RowUpdate {
                account_id,
                category_id,
                description: &description,
                category_label: &category_label,
                recurrence_id: Some(rid),
            },
        )
        .await?;
        return Ok(0);
    }

    // Siguientes: cortar lo estrictamente posterior a la fecha original de
    // esta fila, nunca relativo a hoy, para no borrar programados anteriores.
    sqlx::query(
        "DELETE FROM transactions
        WHERE recurrence_id = $1 AND user_id = $2 AND id <> $3 AND entry_date > $4::date",
    )
    .bind(rid)
    .bind(user_id)
    .bind(transaction_id)
    .bind(&original_date)
    .execute(&mut *conn)
    .await
    .map_err(|_| "Error al cortar los futuros".to_string())?;

    let Some(recurrence_input) = input.recurrence.as_ref() else {
        // la serie termina acá: cae la recurrencia y las filas previas
        // pierden el link por el FK SET NULL
        update_transaction_row(
            conn,
            user_id,
            transaction_id,
            input,
            &RowUpdate {
                account_id,
                category_id,
                description: &description,
                category_label: &category_label,
                recurrence_id: None,
            },
        )
        .await?;
        sqlx::query("DELETE FROM recurrences WHERE id = $1 AND user_id = $2")
            .bind(rid)
            .bind(user_id)
            .execute(&mut *conn)
            .await
            .map_err(|_| "Error al borrar la recurrencia".to_string())?;
        return Ok(0);
    };

    // re-plantillar desde esta fila: el CUOTAS del form es el total de la serie
    // original, así que desde esta cuota en adelante quedan total - transcurridas
    let template: Option<(String,)> = sqlx::query_as(
        "SELECT to_char(start_date, 'YYYY-MM-DD')
        FROM recurrences WHERE id = $1 AND user_id = $2",
    )
    .bind(rid)
    .bind(user_id)
    .fetch_optional(&mut *conn)
    .await
    .map_err(|_| "Error al buscar la recurrencia".to_string())?;
    let (start_date,) = template.ok_or_else(|| "Recurrencia no encontrada".to_string())?;
    let remaining_installments = recurrence_input
        .installment_count
        .map(|c| (c - months_between(&start_date, &original_date)).max(1));

    sqlx::query(
        "UPDATE recurrences SET account_id = $3, category_id = $4, category_label = $5,
            description = $6, amount = $7::numeric, is_income = $8, currency = $9,
            day_of_month = $10, start_date = $11::date, installment_count = $12, installments_generated = 1, is_active = true
        WHERE id = $1 AND user_id = $2",
    )
    .bind(rid)
    .bind(user_id)
    .bind(account_id)
    .bind(category_id)
    .bind(&category_label)
    .bind(&description)
    .bind(input.amount)
    .bind(input.is_income)
    .bind(&input.currency)
    .bind(day_of(&input.entry_date))
    .bind(&input.entry_date)
    .bind(remaining_installments)
    .execute(&mut *conn)
    .await
    .map_err(|_| "Error al guardar la recurrencia".to_string())?;
    update_transaction_row(
        conn,
        user_id,
        transaction_id,
        input,
        &RowUpdate {
            account_id,
            category_id,
            description: &description,
            category_label: &category_label,
            recurrence_id: Some(rid),
        },
    )
    .await?;
    materialize(&mut *conn, user_id, &crate::date::today_iso())
        .await
        .map_err(|_| "Error al materializar la recurrencia".to_string())?;
    let (n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM transactions
        WHERE recurrence_id = $1 AND user_id = $2 AND id <> $3 AND entry_date > $4::date",
    )
    .bind(rid)
    .bind(user_id)
    .bind(transaction_id)
    .bind(&original_date)
    .fetch_one(&mut *conn)
    .await
    .map_err(|_| "Error al contar los futuros".to_string())?;
    Ok(n)
}

/// Borrar solo el asiento (la recurrencia se corta con su endpoint).
pub async fn delete_transaction(
    conn: &mut sqlx::PgConnection,
    user_id: i32,
    transaction_id: i32,
) -> sqlx::Result<u64> {
    let res = sqlx::query("DELETE FROM transactions WHERE id = $1 AND user_id = $2")
        .bind(transaction_id)
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

#[post("/api/transactions")]
pub async fn transaction_create(
    body: web::Json<TransactionInput>,
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
    match create_transaction(&mut conn, user_id, &body).await {
        Ok((id, generated)) => {
            HttpResponse::Created().json(serde_json::json!({ "id": id, "generated": generated }))
        }
        Err(msg) => error_response(msg),
    }
}

#[patch("/api/transactions/{id}")]
pub async fn transaction_update(
    path: web::Path<Uuid>,
    body: web::Json<TransactionInput>,
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
        sqlx::query_as("SELECT id FROM transactions WHERE public_id = $1 AND user_id = $2")
            .bind(public_id)
            .bind(user_id)
            .fetch_optional(&mut *conn)
            .await
            .unwrap_or(None);
    let Some((transaction_id,)) = internal else {
        return HttpResponse::NotFound().finish();
    };
    match update_transaction(&mut conn, user_id, transaction_id, &body).await {
        // mismo shape que el POST: el frontend resalta data.id y avisa data.generated
        Ok(generated) => {
            HttpResponse::Ok().json(serde_json::json!({ "id": public_id, "generated": generated }))
        }
        Err(msg) => error_response(msg),
    }
}

#[delete("/api/transactions/{id}")]
pub async fn transaction_delete(
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
        sqlx::query_as("SELECT id FROM transactions WHERE public_id = $1 AND user_id = $2")
            .bind(public_id)
            .bind(user_id)
            .fetch_optional(&mut *conn)
            .await
            .unwrap_or(None);
    let Some((transaction_id,)) = internal else {
        return HttpResponse::NotFound().finish();
    };
    match delete_transaction(&mut conn, user_id, transaction_id).await {
        Ok(0) => HttpResponse::NotFound().finish(),
        Ok(_) => HttpResponse::Ok().body("Asiento borrado"),
        Err(_) => HttpResponse::InternalServerError().body("Error al borrar el asiento"),
    }
}

#[cfg(test)]
mod tests {
    use super::{RecurrenceInput, Scope, TransactionInput, update_transaction};
    use crate::routes::api::recurrences::last_day_of_month;
    use sqlx::Connection;
    use sqlx::PgConnection;
    use std::time::{SystemTime, UNIX_EPOCH};
    use uuid::Uuid;

    /// Mismo esquema que los tests de recurrencias: una transacción que se
    /// revierte, porque el rol de .env no tiene CREATEDB.
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
        let email = format!("test-edit-{nanos}@rfinance.local");
        let (id,): (i32,) =
            sqlx::query_as("INSERT INTO users (email, password) VALUES ($1, 'x') RETURNING id")
                .bind(&email)
                .fetch_one(&mut *tx)
                .await
                .expect("user de test");
        id
    }

    /// Fecha del mes k (0 = el de `desde`) respetando el día y su recorte.
    fn month_date(desde: &str, k: i32, day_of_month: i16) -> String {
        let indice =
            desde[0..4].parse::<i32>().unwrap() * 12 + desde[5..7].parse::<i32>().unwrap() - 1 + k;
        let (year, month) = (indice / 12, indice % 12 + 1);
        let day_of_month =
            i64::from(day_of_month).min(last_day_of_month(i64::from(year), i64::from(month)));
        format!("{year:04}-{month:02}-{day_of_month:02}")
    }

    /// Siembra una serie de `filas` asientos mensuales starting en `inicio`,
    /// todos a 320000. Devuelve (rec_id, ids en orden cronológico, public_id
    /// de la cuenta).
    async fn seed_series(
        tx: &mut PgConnection,
        user_id: i32,
        start_date: &str,
        installment_count: Option<i16>,
        rows: i32,
    ) -> (i32, Vec<i32>, Uuid) {
        let (cid, account_public_id): (i32, Uuid) = sqlx::query_as(
            "INSERT INTO accounts (user_id, name, currency) VALUES ($1, 'Banco', 'ARS') RETURNING id, public_id",
        )
        .bind(user_id)
        .fetch_one(&mut *tx)
        .await
        .expect("cuenta de test");
        let (rid,): (i32,) = sqlx::query_as(
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
        .bind(i16::try_from(rows).unwrap())
        .fetch_one(&mut *tx)
        .await
        .expect("recurrencia de test");
        let mut ids = Vec::new();
        for k in 0..rows {
            let entry_date = month_date(start_date, k, 12);
            let (id,): (i32,) = sqlx::query_as(
                "INSERT INTO transactions
                    (user_id, account_id, category_id, category_label, description,
                     amount, is_income, currency, entry_date, recurrence_id)
                VALUES ($1, $2, NULL, 'Alquiler', 'Alquiler', 320000::numeric, false, 'ARS', $3::date, $4)
                RETURNING id",
            )
            .bind(user_id)
            .bind(cid)
            .bind(&entry_date)
            .bind(rid)
            .fetch_one(&mut *tx)
            .await
            .expect("asiento de test");
            ids.push(id);
        }
        (rid, ids, account_public_id)
    }

    fn make_input(
        account_id: Uuid,
        entry_date: &str,
        amount: f64,
        description: &str,
    ) -> TransactionInput {
        TransactionInput {
            entry_date: entry_date.into(),
            account_id,
            category_id: None,
            description: description.into(),
            amount,
            is_income: false,
            currency: "ARS".into(),
            recurrence: None,
            scope: None,
        }
    }

    async fn amounts_by_date(tx: &mut PgConnection, user_id: i32) -> Vec<(String, f64, bool)> {
        sqlx::query_as(
            "SELECT to_char(entry_date, 'YYYY-MM-DD'), amount::float8, recurrence_id IS NOT NULL
            FROM transactions WHERE user_id = $1 ORDER BY entry_date",
        )
        .bind(user_id)
        .fetch_all(&mut *tx)
        .await
        .expect("montos")
    }

    async fn fetch_recurrence(tx: &mut PgConnection, rid: i32) -> (String, Option<i16>, i16) {
        sqlx::query_as(
            "SELECT to_char(start_date, 'YYYY-MM-DD'), installment_count, installments_generated
            FROM recurrences WHERE id = $1",
        )
        .bind(rid)
        .fetch_one(&mut *tx)
        .await
        .expect("recurrencia")
    }

    // El bug original: editar un asiento de la serie con alcance Unica (o sin
    // alcance, clientes viejos) no debe borrar ni tocar ningún otro.
    #[async_std::test]
    async fn single_leaves_series_intact() {
        let mut conn = connect().await;
        let mut tx = conn.begin().await.expect("tx");
        let user = make_user(&mut tx).await;
        let (rid, ids, cid) = seed_series(&mut tx, user, "2026-10-12", Some(4), 4).await;

        let mut e = make_input(cid, "2026-12-12", 500.0, "Alquiler DIC");
        e.scope = Some(Scope::Single);
        let generated = update_transaction(&mut tx, user, ids[2], &e)
            .await
            .expect("editar");
        assert_eq!(generated, 0);

        let rows = amounts_by_date(&mut tx, user).await;
        assert_eq!(rows.len(), 4, "no debe borrar ni crear asientos");
        assert_eq!(rows[0].1, 320000.0);
        assert_eq!(rows[1].1, 320000.0);
        assert_eq!(rows[2].1, 500.0);
        assert_eq!(rows[3].1, 320000.0);
        assert!(
            rows.iter().all(|(_, _, linked)| *linked),
            "la serie sigue ligada"
        );
        assert_eq!(
            fetch_recurrence(&mut tx, rid).await,
            ("2026-10-12".into(), Some(4), 4)
        );

        tx.rollback().await.expect("rollback");
    }

    // Clientes sin el campo alcance: default seguro (Unica), nada desaparece.
    #[async_std::test]
    async fn no_scope_defaults_to_single() {
        let mut conn = connect().await;
        let mut tx = conn.begin().await.expect("tx");
        let user = make_user(&mut tx).await;
        let (rid, ids, cid) = seed_series(&mut tx, user, "2026-10-12", Some(4), 4).await;

        let e = make_input(cid, "2026-11-12", 500.0, "Alquiler NOV");
        update_transaction(&mut tx, user, ids[1], &e)
            .await
            .expect("editar");

        let rows = amounts_by_date(&mut tx, user).await;
        assert_eq!(rows.len(), 4);
        assert_eq!(
            fetch_recurrence(&mut tx, rid).await,
            ("2026-10-12".into(), Some(4), 4)
        );

        tx.rollback().await.expect("rollback");
    }

    // "Este y los siguientes": re-plantilla desde la fila editada; lo anterior
    // no se toca; las cuotas totales descuentan lo ya transcurrido.
    #[async_std::test]
    async fn following_replants_from_row() {
        let mut conn = connect().await;
        let mut tx = conn.begin().await.expect("tx");
        let user = make_user(&mut tx).await;
        let (rid, ids, cid) = seed_series(&mut tx, user, "2026-10-12", Some(4), 4).await;

        let mut e = make_input(cid, "2026-11-12", 500.0, "Alquiler");
        e.scope = Some(Scope::Following);
        e.recurrence = Some(RecurrenceInput {
            installment_count: Some(4),
        });
        let generated = update_transaction(&mut tx, user, ids[1], &e)
            .await
            .expect("editar");
        assert_eq!(generated, 2, "DIC y ENE regenerados");

        let rows = amounts_by_date(&mut tx, user).await;
        assert_eq!(rows.len(), 4, "OCT + NOV + DIC + ENE");
        assert_eq!(rows[0], ("2026-10-12".into(), 320000.0, true));
        for f in &rows[1..] {
            assert_eq!(f.1, 500.0);
            assert!(f.2);
        }
        assert_eq!(rows[1].0, "2026-11-12");
        assert_eq!(rows[2].0, "2026-12-12");
        assert_eq!(rows[3].0, "2027-01-12");
        // 4 totales desde el inicio original: 1 ya pasada (OCT), quedan 3 desde NOV
        assert_eq!(
            fetch_recurrence(&mut tx, rid).await,
            ("2026-11-12".into(), Some(3), 3)
        );

        tx.rollback().await.expect("rollback");
    }

    // "Este y los siguientes" sin rec en el body: la serie termina acá.
    #[async_std::test]
    async fn following_without_rec_ends_series() {
        let mut conn = connect().await;
        let mut tx = conn.begin().await.expect("tx");
        let user = make_user(&mut tx).await;
        let (_rid, ids, cid) = seed_series(&mut tx, user, "2026-10-12", Some(4), 4).await;

        let mut e = make_input(cid, "2026-11-12", 500.0, "Alquiler final");
        e.scope = Some(Scope::Following);
        let generated = update_transaction(&mut tx, user, ids[1], &e)
            .await
            .expect("editar");
        assert_eq!(generated, 0);

        let rows = amounts_by_date(&mut tx, user).await;
        assert_eq!(rows.len(), 2, "solo OCT y NOV, el resto se cortó");
        assert!(
            rows.iter().all(|(_, _, linked)| !*linked),
            "la serie cayó, quedan desligadas"
        );
        assert_eq!(rows[1].1, 500.0);

        tx.rollback().await.expect("rollback");
    }

    // Asiento simple (sin recurrencia): se edita la fila nomás.
    #[async_std::test]
    async fn simple_edits_in_peace() {
        let mut conn = connect().await;
        let mut tx = conn.begin().await.expect("tx");
        let user = make_user(&mut tx).await;
        let (cid, account_public_id): (i32, Uuid) = sqlx::query_as(
            "INSERT INTO accounts (user_id, name, currency) VALUES ($1, 'Banco', 'ARS') RETURNING id, public_id",
        )
        .bind(user)
        .fetch_one(&mut *tx)
        .await
        .expect("cuenta");
        let (mid,): (i32,) = sqlx::query_as(
            "INSERT INTO transactions
                (user_id, account_id, category_id, category_label, description,
                 amount, is_income, currency, entry_date, recurrence_id)
            VALUES ($1, $2, NULL, 'SIN CATEGORÍA', 'Café', 1000::numeric, false, 'ARS', '2026-11-03', NULL)
            RETURNING id",
        )
        .bind(user)
        .bind(cid)
        .fetch_one(&mut *tx)
        .await
        .expect("asiento");

        let e = make_input(account_public_id, "2026-11-03", 1500.0, "Café con leche");
        let generated = update_transaction(&mut tx, user, mid, &e)
            .await
            .expect("editar");
        assert_eq!(generated, 0);
        let rows = amounts_by_date(&mut tx, user).await;
        assert_eq!(rows, vec![("2026-11-03".to_string(), 1500.0, false)]);

        tx.rollback().await.expect("rollback");
    }
}
