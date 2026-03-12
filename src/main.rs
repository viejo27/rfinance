mod routes {
    pub mod web {
        pub mod home;
        pub mod login;
    }
    pub mod api {
        pub mod login;
    }
}

use actix_web::{App, HttpServer, web};
use bcrypt;
use dotenvy::dotenv;
use routes::api::login::login as login_post;
use routes::web::home::index;
use routes::web::login::login;
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

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(db_pool.clone()))
            .service(index)
            .service(login)
            .service(login_post)
            .service(actix_files::Files::new("/css", "static/css").show_files_listing())
            .service(actix_files::Files::new("/js", "static/js").show_files_listing())
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}

async fn ensure_admin_user(pool: &PgPool) {
    let admin_user = env::var("ADMIN_USER")
        .expect("ADMIN_USER not set in .env");
    let admin_password = env::var("ADMIN_PASSWORD")
        .expect("ADMIN_PASSWORD not set in .env");

    let user_email: Option<String> = sqlx::query_as::<_, (Option<String>,)>("SELECT email FROM users WHERE id = 1")
        .fetch_optional(pool)
        .await
        .unwrap()
        .map(|row| row.0)
        .unwrap_or(None);

    match user_email {
        Some(email) if email == admin_user => {}
        Some(_) => {
            let password_hashed = bcrypt::hash(&admin_password, bcrypt::DEFAULT_COST).expect("Failed to hash password");
            sqlx::query("UPDATE users SET email = $1, password = $2 WHERE id = 1")
                .bind(&admin_user)
                .bind(&password_hashed)
                .execute(pool)
                .await
                .unwrap();
        }
        None => {
            let password_hashed = bcrypt::hash(&admin_password, bcrypt::DEFAULT_COST).expect("Failed to hash password");
            sqlx::query("INSERT INTO users (email, password) VALUES ($1, $2)")
                .bind(&admin_user)
                .bind(&password_hashed)
                .execute(pool)
                .await
                .unwrap();
        }
    }
}
