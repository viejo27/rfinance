mod date;
mod middleware;
mod role;
mod routes;

use actix_session::{SessionMiddleware, config::PersistentSession, storage::CookieSessionStore};
use actix_web::{App, HttpServer, cookie::Key, web};
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

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL not set in .env");
    let db_pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to the database");

    sqlx::migrate!()
        .run(&db_pool)
        .await
        .expect("Failed to run database migrations");

    ensure_admin_user(&db_pool).await;

    let session_key_str = env::var("SESSION_KEY").expect("SESSION_KEY not set in .env");
    let session_key = Key::from(session_key_str.as_bytes());
    let is_production = env::var("APP_ENV").unwrap_or_default() == "production";

    HttpServer::new(move || {
        let session_middleware =
            SessionMiddleware::builder(CookieSessionStore::default(), session_key.clone())
                .cookie_name("rfinance_app".to_string())
                .cookie_secure(is_production)
                .session_lifecycle(
                    PersistentSession::default()
                        .session_ttl(actix_web::cookie::time::Duration::days(27)),
                )
                .build();

        let grants = GrantsMiddleware::with_extractor(middleware::grants::extract);

        App::new()
            .app_data(web::Data::new(db_pool.clone()))
            .wrap(actix_web::middleware::from_fn(
                middleware::auth::check_session,
            ))
            .wrap(grants)
            .wrap(session_middleware)
            .service(admin_handler)
            .service(index_handler)
            .service(login_handler)
            .service(register_handler)
            .service(api_snapshot)
            .service(api_login)
            .service(api_register)
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
            .service(actix_files::Files::new("/css", "static/css").show_files_listing())
            .service(actix_files::Files::new("/js", "static/js").show_files_listing())
            .service(actix_files::Files::new("/fonts", "static/fonts"))
            .service(actix_files::Files::new("/textures", "static/textures"))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}

/// Make sure the admin account from `.env` exists and owns the `admin` role.
///
/// The admin is identified by email, not by a hardcoded id, so this works on a
/// fresh database as well as on one that already has users registered.
async fn ensure_admin_user(pool: &PgPool) {
    let admin_email = env::var("ADMIN_USER").expect("ADMIN_USER not set in .env");
    let admin_password = env::var("ADMIN_PASSWORD").expect("ADMIN_PASSWORD not set in .env");

    let existing: Option<(i32,)> = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind(&admin_email)
        .fetch_optional(pool)
        .await
        .expect("Failed to look up the admin user");

    let admin_id = match existing {
        Some((id,)) => id,
        None => {
            let password_hashed = bcrypt::hash(&admin_password, bcrypt::DEFAULT_COST)
                .expect("Failed to hash the admin password");

            let (id,): (i32,) =
                sqlx::query_as("INSERT INTO users (email, password) VALUES ($1, $2) RETURNING id")
                    .bind(&admin_email)
                    .bind(&password_hashed)
                    .fetch_one(pool)
                    .await
                    .expect("Failed to create the admin user");

            id
        }
    };

    sqlx::query(
        "INSERT INTO user_data (user_id, role) VALUES ($1, 'admin')
         ON CONFLICT (user_id) DO UPDATE SET role = 'admin'",
    )
    .bind(admin_id)
    .execute(pool)
    .await
    .expect("Failed to grant the admin role");
}
