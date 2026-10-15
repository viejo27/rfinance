use actix_session::Session;
use actix_web::{HttpResponse, Responder, delete, web};
use sqlx::PgPool;

use crate::fecha;
use crate::routes::api::estado::usuario_de_sesion;

pub const MESES_CORTOS: [&str; 12] = [
    "ENE", "FEB", "MAR", "ABR", "MAY", "JUN", "JUL", "AGO", "SEP", "OCT", "NOV", "DIC",
];

/// Último día de un mes civil (con bisiesto).
pub fn ultimo_dia(anio: i64, mes: i64) -> i64 {
    let bisiesto = anio % 4 == 0 && (anio % 100 != 0 || anio % 400 == 0);
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
    por_mes[(mes - 1) as usize]
}

#[derive(sqlx::FromRow)]
struct RecActiva {
    id: i32,
    cuenta_id: i32,
    categoria_id: Option<i32>,
    categoria_txt: String,
    concepto: String,
    monto: f64,
    es_ingreso: bool,
    moneda: String,
    dia: i16,
    inicio: String,
    cuotas: Option<i16>,
    cuotas_generadas: i16,
}

/// Materializa los asientos futuros que falten para cada recurrencia
/// activa del usuario. Idempotente: el cursor es cuotas_generadas.
///
/// - Indefinida (cuotas NULL): genera los meses restantes del año
///   calendario en curso; el año siguiente rueda solo.
/// - Con cuotas: genera hasta agotar la cantidad total, cruzando años.
pub async fn materializar(
    conn: &mut sqlx::PgConnection,
    user_id: i32,
    hoy_iso: &str,
) -> sqlx::Result<()> {
    let recs: Vec<RecActiva> = sqlx::query_as(
        "SELECT id, cuenta_id, categoria_id, categoria_txt, concepto,
            monto::float8 AS monto, es_ingreso, moneda, dia,
            to_char(inicio, 'YYYY-MM-DD') AS inicio, cuotas, cuotas_generadas
        FROM recurrencias WHERE user_id = $1 AND activa",
    )
    .bind(user_id)
    .fetch_all(&mut *conn)
    .await?;

    let anio_hoy: i64 = hoy_iso[0..4].parse().unwrap_or(2026);

    for r in &recs {
        let anio_i: i64 = r.inicio[0..4].parse().unwrap_or(anio_hoy);
        let mes_i: i64 = r.inicio[5..7].parse().unwrap_or(1);
        let base = anio_i * 12 + (mes_i - 1);
        let dia_origen = i64::from(r.dia);

        let mut insertados = 0;
        let mut k = i64::from(r.cuotas_generadas) + 1;
        loop {
            if r.cuotas.is_some_and(|c| k > i64::from(c)) {
                break;
            }
            let objetivo = base + (k - 1);
            let anio = objetivo / 12;
            let mes = objetivo % 12 + 1;
            if r.cuotas.is_none() && anio > anio_hoy {
                break;
            }
            let dia = dia_origen.min(ultimo_dia(anio, mes));
            let fecha = format!("{anio:04}-{mes:02}-{dia:02}");
            let concepto = if r.cuotas.is_none() {
                format!("{} · {}", r.concepto, MESES_CORTOS[(mes - 1) as usize])
            } else {
                format!("Cuota {}/{} · {}", k, r.cuotas.unwrap_or(0), r.concepto)
            };
            sqlx::query(
                "INSERT INTO movimientos
                    (user_id, cuenta_id, categoria_id, categoria_txt, concepto,
                     monto, es_ingreso, moneda, fecha, rec_id)
                VALUES ($1, $2, $3, $4, $5, $6::numeric, $7, $8, $9::date, $10)",
            )
            .bind(user_id)
            .bind(r.cuenta_id)
            .bind(r.categoria_id)
            .bind(&r.categoria_txt)
            .bind(&concepto)
            .bind(r.monto)
            .bind(r.es_ingreso)
            .bind(&r.moneda)
            .bind(&fecha)
            .bind(r.id)
            .execute(&mut *conn)
            .await?;
            insertados += 1;
            k += 1;
        }
        if insertados > 0 {
            sqlx::query(
                "UPDATE recurrencias SET cuotas_generadas = cuotas_generadas + $1 WHERE id = $2",
            )
            .bind(i16::try_from(insertados).unwrap_or(i16::MAX))
            .bind(r.id)
            .execute(&mut *conn)
            .await?;
        }
    }
    Ok(())
}

/// Borrar una recurrencia: corta los asientos futuros y conserva los ya
/// cargados (el FK con SET NULL les saca el rec_id al caer la rec).
#[delete("/api/recurrencias/{id}")]
pub async fn rec_borrar(
    path: web::Path<i32>,
    session: Session,
    pool: web::Data<PgPool>,
) -> impl Responder {
    let Some(user_id) = usuario_de_sesion(&session) else {
        return HttpResponse::Unauthorized().finish();
    };
    let mut conn = match pool.get_ref().acquire().await {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Error de conexión"),
    };
    match borrar_recurrencia(&mut conn, user_id, path.into_inner()).await {
        Ok((futuros, _)) => HttpResponse::Ok().json(serde_json::json!({ "futurosBorrados": futuros })),
        Err(_) => HttpResponse::InternalServerError().body("Error al borrar la recurrencia"),
    }
}

/// Lógica pura del borrado de recurrencia (testeable en transacción).
pub async fn borrar_recurrencia(
    conn: &mut sqlx::PgConnection,
    user_id: i32,
    rec_id: i32,
) -> sqlx::Result<(u64, u64)> {
    let hoy = fecha::hoy_iso();
    let futuros = sqlx::query(
        "DELETE FROM movimientos WHERE rec_id = $1 AND user_id = $2 AND fecha > $3::date",
    )
    .bind(rec_id)
    .bind(user_id)
    .bind(&hoy)
    .execute(&mut *conn)
    .await?;
    let restante = sqlx::query("DELETE FROM recurrencias WHERE id = $1 AND user_id = $2")
        .bind(rec_id)
        .bind(user_id)
        .execute(&mut *conn)
        .await?;
    Ok((futuros.rows_affected(), restante.rows_affected()))
}

#[cfg(test)]
mod tests {
    use super::{MESES_CORTOS, materializar, ultimo_dia};
    use sqlx::Connection;
    use sqlx::PgConnection;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// El rol de la base de .env no tiene CREATEDB: #[sqlx::test] (una
    /// base por test) no es posible en esta máquina. Cada test corre en
    /// una transacción que se revierte al final; la base queda intacta.
    /// Si el dueño habilita ALTER ROLE ... CREATEDB, pueden migrar a
    /// #[sqlx::test] tal como sugiere AGENTS.md.
    async fn conectar() -> PgConnection {
        dotenvy::dotenv().ok();
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL para los tests");
        PgConnection::connect(&url).await.expect("conexión de test")
    }

    async fn usuario(tx: &mut PgConnection) -> i32 {
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

    async fn sembrar_rec(
        tx: &mut PgConnection,
        user_id: i32,
        inicio: &str,
        cuotas: Option<i16>,
        generadas: i16,
    ) -> i32 {
        let (cid,): (i32,) = sqlx::query_as(
            "INSERT INTO cuentas (user_id, nombre, moneda) VALUES ($1, 'Banco', 'ARS') RETURNING id",
        )
        .bind(user_id)
        .fetch_one(&mut *tx)
        .await
        .expect("cuenta de test");
        let (id,): (i32,) = sqlx::query_as(
            "INSERT INTO recurrencias
                (user_id, cuenta_id, categoria_id, categoria_txt, concepto, monto,
                 es_ingreso, moneda, dia, inicio, cuotas, cuotas_generadas)
            VALUES ($1, $2, NULL, 'Alquiler', 'Alquiler', 320000, false, 'ARS', 12, $3::date, $4, $5)
            RETURNING id",
        )
        .bind(user_id)
        .bind(cid)
        .bind(inicio)
        .bind(cuotas)
        .bind(generadas)
        .fetch_one(&mut *tx)
        .await
        .expect("recurrencia de test");
        // el origen ya cargado, como lo crea el POST de asientos
        sqlx::query(
            "INSERT INTO movimientos
                (user_id, cuenta_id, categoria_id, categoria_txt, concepto,
                 monto, es_ingreso, moneda, fecha, rec_id)
            VALUES ($1, $2, NULL, 'Alquiler', 'Alquiler', 320000::numeric, false, 'ARS', $3::date, $4)",
        )
        .bind(user_id)
        .bind(cid)
        .bind(inicio)
        .bind(id)
        .execute(&mut *tx)
        .await
        .expect("asiento origen de test");
        id
    }

    async fn contar_de_rec(tx: &mut PgConnection, rec_id: i32) -> i64 {
        let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM movimientos WHERE rec_id = $1")
            .bind(rec_id)
            .fetch_one(&mut *tx)
            .await
            .expect("conteo");
        n
    }

    async fn generadas_de(tx: &mut PgConnection, rec_id: i32) -> i16 {
        let (g,): (i16,) =
            sqlx::query_as("SELECT cuotas_generadas FROM recurrencias WHERE id = $1")
                .bind(rec_id)
                .fetch_one(&mut *tx)
                .await
                .expect("cursor");
        g
    }

    #[test]
    fn ultimo_dia_civil() {
        assert_eq!(ultimo_dia(2026, 2), 28);
        assert_eq!(ultimo_dia(2024, 2), 29);
        assert_eq!(ultimo_dia(2026, 12), 31);
        assert_eq!(MESES_CORTOS[11], "DIC");
    }

    #[async_std::test]
    async fn indefinida_genera_hasta_fin_de_ano() {
        let mut conn = conectar().await;
        let mut tx = conn.begin().await.expect("tx");
        let user = usuario(&mut tx).await;
        let rec = sembrar_rec(&mut tx, user, "2026-10-12", None, 1).await;

        materializar(&mut tx, user, "2026-10-15")
            .await
            .expect("materializar");

        // OCT (origen) + NOV + DIC del año calendario 2026, y nada más
        assert_eq!(contar_de_rec(&mut tx, rec).await, 3);
        assert_eq!(generadas_de(&mut tx, rec).await, 3);

        // idempotente: otra llamada en el mismo año no agrega nada
        materializar(&mut tx, user, "2026-11-20")
            .await
            .expect("idempotente");
        assert_eq!(contar_de_rec(&mut tx, rec).await, 3);
        assert_eq!(generadas_de(&mut tx, rec).await, 3);

        tx.rollback().await.expect("rollback");
    }

    #[async_std::test]
    async fn cuotas_cruzando_ano() {
        let mut conn = conectar().await;
        let mut tx = conn.begin().await.expect("tx");
        let user = usuario(&mut tx).await;
        let rec = sembrar_rec(&mut tx, user, "2026-12-12", Some(12), 1).await;

        materializar(&mut tx, user, "2026-10-15")
            .await
            .expect("materializar");

        // las 12 cuotas quedan programadas aunque crucen el año
        assert_eq!(contar_de_rec(&mut tx, rec).await, 12);
        assert_eq!(generadas_de(&mut tx, rec).await, 12);

        // agotada: otra llamada no agrega nada ni siquiera el año próximo
        materializar(&mut tx, user, "2027-02-01")
            .await
            .expect("agotada");
        assert_eq!(contar_de_rec(&mut tx, rec).await, 12);

        tx.rollback().await.expect("rollback");
    }
}
