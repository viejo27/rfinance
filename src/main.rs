mod date;
mod middleware;
mod role;
mod routes;

use actix_governor::{Governor, GovernorConfigBuilder, KeyExtractor, SimpleKeyExtractionError};
use actix_session::{SessionMiddleware, config::PersistentSession, storage::CookieSessionStore};
use actix_web::{App, HttpServer, cookie::Key, dev::ServiceRequest, get, web};
use actix_web_grants::GrantsMiddleware;
use dotenvy::dotenv;
use routes::api::{
    account_create, account_delete, account_update, api_login, api_logout, api_recurrence_delete,
    api_register, api_snapshot, category_create, category_delete, category_update,
    transaction_create, transaction_delete, transaction_update,
};
use routes::web::{admin_handler, index_handler, login_handler, register_handler};
use sqlx::PgPool;
use std::env;
use std::net::IpAddr;

/// Clave del limiter: X-Real-IP con fallback a la IP del peer directo.
/// Detrás del proxy el peer es siempre 127.0.0.1: con la key por peer,
/// todos los usuarios compartirían la misma cubeta de rate limit (y un
/// solo spammer bloquearía a todo el mundo). El header solo lo puede
/// setear el proxy porque la app escucha en loopback.
#[derive(Debug, Clone, Copy, Default)]
struct RealIpKeyExtractor;

impl RealIpKeyExtractor {
    /// Misma reducción que el default de la crate: IPv6 se agrupa por
    /// prefijo, para que un solo usuario no ocupe el mapa con la mitad de
    /// un rango.
    fn key_of(ip: IpAddr) -> IpAddr {
        match ip {
            IpAddr::V6(v6) => {
                let mut octets = v6.octets();
                octets[7..16].fill(0);
                IpAddr::V6(octets.into())
            }
            v4 => v4,
        }
    }
}

impl KeyExtractor for RealIpKeyExtractor {
    type Key = IpAddr;
    type KeyExtractionError = SimpleKeyExtractionError<&'static str>;

    fn name(&self) -> &'static str {
        "IP real (X-Real-IP)"
    }

    fn key_name(&self, key: &Self::Key) -> Option<String> {
        Some(key.to_string())
    }

    fn extract(&self, req: &ServiceRequest) -> Result<Self::Key, Self::KeyExtractionError> {
        // Header parseado estricto: un X-Real-IP inválido no pasa como key.
        if let Some(valor) = req.headers().get("x-real-ip")
            && let Ok(ip) = valor.to_str().unwrap_or("").parse::<IpAddr>()
        {
            return Ok(Self::key_of(ip));
        }
        req.peer_addr()
            .map(|socket| socket.ip())
            .map(Self::key_of)
            .ok_or_else(|| SimpleKeyExtractionError::new("No se pudo determinar la IP"))
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();
    // El Logger de actix escribe vía la crate log: sin un logger activado
    // no sale nada. Con default info, access log + AUDIT van a stdout.
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL no está definida en .env");
    let db_pool = PgPool::connect(&database_url)
        .await
        .expect("Error al conectar a la base de datos");

    sqlx::migrate!()
        .run(&db_pool)
        .await
        .expect("Error al correr las migraciones");

    ensure_admin_user(&db_pool).await;

    // La cookie de sesión entera es un AEAD firmado/cifrado con esta clave:
    // si es corta o se filtra, se pueden forjar sesiones admin. El check
    // anticipa el panic críptico de Key::from con instrucción concreta.
    let session_key_str = env::var("SESSION_KEY").unwrap_or_else(|_| {
        panic!("SESSION_KEY no está definida en .env — generá una con: openssl rand -hex 32")
    });
    if session_key_str.len() < 32 {
        panic!(
            "SESSION_KEY demasiado corta ({} bytes): el cookie de sesión necesita al menos 32 bytes; generá una con: openssl rand -hex 32",
            session_key_str.len()
        );
    }
    let session_key = Key::from(session_key_str.as_bytes());
    let is_production = env::var("APP_ENV").unwrap_or_default() == "production";

    // Dirección de escucha: loopback por defecto (el proxy/uso local es la
    // norma). Para LAN se setea BIND_ADDR=0.0.0.0:8080; sin TLS eso deja
    // sesiones replayables en la red (ver SECURITY.md), solo red de confianza.
    let bind_addr = env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
    if bind_addr.parse::<std::net::SocketAddr>().is_err() {
        panic!("BIND_ADDR inválida ({bind_addr:?}): usá formato IP:PUERTO, p. ej. 127.0.0.1:8080");
    }

    // Rate limit de los endpoints anónimos más sensibles, por IP real
    // (X-Real-IP que setea el proxy): 5 logins y 3 registros por minuto,
    // con burst cobrado de entrada. Esta es la capa in-app; el proxy suma
    // su propio limit_req como segunda capa (deploy/nginx-rfinance.example.conf).
    let login_governor = GovernorConfigBuilder::default()
        .requests_per_minute(5)
        .burst_size(5)
        .key_extractor(RealIpKeyExtractor)
        .finish()
        .expect("config del rate limit de login");
    let register_governor = GovernorConfigBuilder::default()
        .requests_per_minute(3)
        .burst_size(3)
        .key_extractor(RealIpKeyExtractor)
        .finish()
        .expect("config del rate limit de register");

    HttpServer::new(move || {
        let session_middleware =
            SessionMiddleware::builder(CookieSessionStore::default(), session_key.clone())
                .cookie_name("rfinance_app".to_string())
                .cookie_secure(is_production)
                .session_lifecycle(
                    PersistentSession::default()
                        // 7 días en vez de 27: menos vida útil para un
                        // cookie robado. Sigue sin revocación server-side.
                        .session_ttl(actix_web::cookie::time::Duration::days(7)),
                )
                .build();

        let grants = GrantsMiddleware::with_extractor(middleware::grants::extract);

        // De afuera hacia adentro: Logger → headers → session → audit →
        // grants → check_session. El audit va DEBAJO del middleware de
        // sesión (que es quien parsea el cookie): así loguea el user_id.
        App::new()
            .app_data(web::Data::new(db_pool.clone()))
            .wrap(actix_web::middleware::from_fn(
                middleware::auth::check_session,
            ))
            .wrap(grants)
            .wrap(actix_web::middleware::from_fn(middleware::audit::audit))
            .wrap(session_middleware)
            .wrap(actix_web::middleware::from_fn(
                middleware::headers::set_security_headers,
            ))
            .wrap(actix_web::middleware::Logger::default())
            .service(admin_handler)
            .service(index_handler)
            .service(login_handler)
            .service(register_handler)
            .service(api_snapshot)
            // Login y registro con rate limit propio por endpoint: el
            // struct que genera `#[post]` no expone el Resource, así que
            // estos dos van con resource + route a mano.
            .service(
                web::resource("/api/login")
                    .wrap(Governor::new(&login_governor))
                    .route(web::post().to(api_login)),
            )
            .service(
                web::resource("/api/register")
                    .wrap(Governor::new(&register_governor))
                    .route(web::post().to(api_register)),
            )
            .service(api_logout)
            .service(account_create)
            .service(account_update)
            .service(account_delete)
            .service(category_create)
            .service(category_update)
            .service(category_delete)
            .service(transaction_create)
            .service(transaction_update)
            .service(transaction_delete)
            .service(api_recurrence_delete)
            // Sin show_files_listing: el listing regala el inventario de
            // /css y /js y no suma nada.
            .service(actix_files::Files::new("/css", "static/css"))
            .service(actix_files::Files::new("/js", "static/js"))
            .service(actix_files::Files::new("/fonts", "static/fonts"))
            .service(actix_files::Files::new("/textures", "static/textures"))
            .service(favicon_ico)
            .service(favicon_png)
    })
    .bind(bind_addr.as_str())?
    .run()
    .await
}

/// Asegura que la cuenta admin de `.env` exista y tenga el rol `admin`.
///
/// El admin se identifica por email, no por id hardcodeado, así funciona en
/// una base nueva como en una con usuarios ya registrados.
async fn ensure_admin_user(pool: &PgPool) {
    let admin_email = env::var("ADMIN_USER").expect("ADMIN_USER no está definida en .env");
    let admin_password =
        env::var("ADMIN_PASSWORD").expect("ADMIN_PASSWORD no está definida en .env");

    let existing: Option<(i32,)> = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind(&admin_email)
        .fetch_optional(pool)
        .await
        .expect("Error al buscar el usuario admin");

    let admin_id = match existing {
        Some((id,)) => id,
        None => {
            let password_hashed = bcrypt::hash(&admin_password, bcrypt::DEFAULT_COST)
                .expect("Error al hashear la contraseña del admin");

            let (id,): (i32,) =
                sqlx::query_as("INSERT INTO users (email, password) VALUES ($1, $2) RETURNING id")
                    .bind(&admin_email)
                    .bind(&password_hashed)
                    .fetch_one(pool)
                    .await
                    .expect("Error al crear el usuario admin");

            id
        }
    };

    let _ = sqlx::query(
        "INSERT INTO user_data (user_id, role) VALUES ($1, 'admin')
         ON CONFLICT (user_id) DO UPDATE SET role = 'admin'",
    )
    .bind(admin_id)
    .execute(pool)
    .await
    .expect("Error al otorgar el rol de admin");
}

/// Favicon: archivo único servido con su content-type (`image/x-icon`).
#[get("/favicon.ico")]
async fn favicon_ico() -> actix_web::Result<actix_files::NamedFile> {
    Ok(actix_files::NamedFile::open_async("static/favicon.ico").await?)
}

/// Master del favicon (512 px, con la proveniencia embebida).
#[get("/favicon-512.png")]
async fn favicon_png() -> actix_web::Result<actix_files::NamedFile> {
    Ok(actix_files::NamedFile::open_async("static/favicon-512.png").await?)
}
