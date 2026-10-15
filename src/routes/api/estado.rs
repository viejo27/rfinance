use actix_session::Session;
use actix_web::{HttpResponse, Responder, get, web};
use serde::Serialize;
use sqlx::{FromRow, PgPool};

use crate::fecha;

/// Usuario de sesión o None: los handlers API devuelven 401 sin él.
pub fn usuario_de_sesion(session: &Session) -> Option<i32> {
    session.get::<i32>("user_id").unwrap_or(None)
}

#[derive(Serialize, FromRow)]
pub struct EstadoCuenta {
    pub id: i32,
    pub nombre: String,
    pub moneda: String,
    pub saldo_inicial: f64,
    pub saldo: f64,
}

#[derive(Serialize, FromRow)]
pub struct EstadoCategoria {
    pub id: i32,
    pub nombre: String,
    pub limite: Option<f64>,
    pub moneda: String,
}

#[derive(Serialize, FromRow)]
pub struct EstadoMovimiento {
    pub id: i32,
    pub fecha: String,
    pub cuenta: String,
    pub cuenta_id: i32,
    pub categoria: String,
    pub categoria_id: Option<i32>,
    pub concepto: String,
    pub valor: f64,
    pub moneda: String,
    pub rec_id: Option<i32>,
}

#[derive(Serialize, FromRow)]
pub struct EstadoRecurrencia {
    pub id: i32,
    pub cuotas: Option<i16>,
    pub generadas: i16,
}

#[derive(Serialize)]
pub struct Estado {
    pub anio: i64,
    pub mes: i64,
    pub hoy: String,
    pub corte: String,
    pub cuentas: Vec<EstadoCuenta>,
    pub categorias: Vec<EstadoCategoria>,
    pub movimientos: Vec<EstadoMovimiento>,
    pub recurrencias: Vec<EstadoRecurrencia>,
}

#[get("/api/estado")]
pub async fn estado(session: Session, pool: web::Data<PgPool>) -> impl Responder {
    let Some(user_id) = usuario_de_sesion(&session) else {
        return HttpResponse::Unauthorized().finish();
    };
    let mut conn = match pool.get_ref().acquire().await {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Error de conexión"),
    };
    match construir(&mut conn, user_id).await {
        Ok(e) => HttpResponse::Ok().json(e),
        Err(_) => HttpResponse::InternalServerError().body("Error al leer el estado"),
    }
}

/// Estado completo del usuario: saldos por cuenta, categorías,
/// asientos y recurrencias activas.
pub async fn construir(conn: &mut sqlx::PgConnection, user_id: i32) -> sqlx::Result<Estado> {
    let (hoy_iso, corte) = fecha::hoy();
    let anio: i64 = hoy_iso[0..4].parse().unwrap_or(2026);
    let mes: i64 = hoy_iso[5..7].parse().unwrap_or(1);

    let cuentas: Vec<EstadoCuenta> = sqlx::query_as(
        "SELECT c.id, c.nombre, c.moneda, c.saldo_inicial::float8 AS saldo_inicial,
            c.saldo_inicial::float8
            + COALESCE(SUM(CASE
                WHEN m.moneda = c.moneda AND m.es_ingreso THEN m.monto::float8
                WHEN m.moneda = c.moneda THEN -m.monto::float8
                ELSE 0 END), 0) AS saldo
        FROM cuentas c
        LEFT JOIN movimientos m ON m.cuenta_id = c.id
        WHERE c.user_id = $1
        GROUP BY c.id
        ORDER BY c.id",
    )
    .bind(user_id)
    .fetch_all(&mut *conn)
    .await?;

    let categorias: Vec<EstadoCategoria> = sqlx::query_as(
        "SELECT id, nombre, limite::float8 AS limite, moneda
        FROM categorias WHERE user_id = $1 ORDER BY id",
    )
    .bind(user_id)
    .fetch_all(&mut *conn)
    .await?;

    let movimientos: Vec<EstadoMovimiento> = sqlx::query_as(
        "SELECT m.id, to_char(m.fecha, 'YYYY-MM-DD') AS fecha, c.nombre AS cuenta,
            m.cuenta_id, m.categoria_txt AS categoria, m.categoria_id, m.concepto,
            CASE WHEN m.es_ingreso THEN m.monto::float8 ELSE -m.monto::float8 END AS valor,
            m.moneda, m.rec_id
        FROM movimientos m
        JOIN cuentas c ON c.id = m.cuenta_id
        WHERE m.user_id = $1
        ORDER BY m.fecha DESC, m.id DESC",
    )
    .bind(user_id)
    .fetch_all(&mut *conn)
    .await?;

    let recurrencias: Vec<EstadoRecurrencia> = sqlx::query_as(
        "SELECT id, cuotas, cuotas_generadas AS generadas
        FROM recurrencias WHERE user_id = $1 AND activa ORDER BY id",
    )
    .bind(user_id)
    .fetch_all(&mut *conn)
    .await?;

    Ok(Estado {
        anio,
        mes,
        hoy: hoy_iso,
        corte,
        cuentas,
        categorias,
        movimientos,
        recurrencias,
    })
}
