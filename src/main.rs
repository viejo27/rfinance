mod middleware;
mod role;
mod routes;

use actix_session::{SessionMiddleware, config::PersistentSession, storage::CookieSessionStore};
use actix_web::{App, HttpServer, cookie::Key, web};
use actix_web_grants::GrantsMiddleware;
use dotenvy::dotenv;
use routes::api::{api_login, api_logout, api_register};
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
            .service(api_login)
            .service(api_register)
            .service(api_logout)
            .service(actix_files::Files::new("/css", "static/css").show_files_listing())
            .service(actix_files::Files::new("/js", "static/js").show_files_listing())
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}

async fn ensure_admin_user(pool: &PgPool) {
    let admin_user = env::var("ADMIN_USER").expect("ADMIN_USER not set in .env");
    let admin_password = env::var("ADMIN_PASSWORD").expect("ADMIN_PASSWORD not set in .env");

    let user_email: Option<String> =
        sqlx::query_as::<_, (Option<String>,)>("SELECT email FROM users WHERE id = 1")
            .fetch_optional(pool)
            .await
            .unwrap()
            .map(|row| row.0)
            .unwrap_or(None);

    match user_email {
        Some(email) if email == admin_user => {}
        Some(_) => {
            let password_hashed = bcrypt::hash(&admin_password, bcrypt::DEFAULT_COST)
                .expect("Failed to hash password");
            sqlx::query("UPDATE users SET email = $1, password = $2 WHERE id = 1")
                .bind(&admin_user)
                .bind(&password_hashed)
                .execute(pool)
                .await
                .unwrap();
            sqlx::query(
                "INSERT INTO user_data (user_id, role) VALUES (1, 'admin')
                 ON CONFLICT (user_id) DO UPDATE SET role = 'admin'",
            )
            .execute(pool)
            .await
            .unwrap();
        }
        None => {
            let password_hashed = bcrypt::hash(&admin_password, bcrypt::DEFAULT_COST)
                .expect("Failed to hash password");
            sqlx::query("INSERT INTO users (email, password) VALUES ($1, $2)")
                .bind(&admin_user)
                .bind(&password_hashed)
                .execute(pool)
                .await
                .unwrap();
            sqlx::query(
                "INSERT INTO user_data (user_id, role) VALUES (1, 'admin')
                 ON CONFLICT (user_id) DO UPDATE SET role = 'admin'",
            )
            .execute(pool)
            .await
            .unwrap();
        }
    }
}
