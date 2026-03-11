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
