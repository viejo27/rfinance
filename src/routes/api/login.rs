use actix_web::{HttpResponse, Responder, post, web};
use bcrypt;
use serde::Deserialize;
use sqlx::PgPool;

#[derive(Deserialize)]
struct LoginRequest {
    email: String,
    password: String,
}

#[post("/api/login")]
async fn login(body: web::Json<LoginRequest>, pool: web::Data<PgPool>) -> impl Responder {
    let result: Option<(String,)> = sqlx::query_as("SELECT password FROM users WHERE email = $1")
        .bind(&body.email)
        .fetch_optional(pool.get_ref())
        .await
        .unwrap();

    match result {
        Some((hashed_password,)) => {
            if bcrypt::verify(&body.password, &hashed_password).unwrap_or(false) {
                HttpResponse::Ok()
            } else {
                HttpResponse::Unauthorized()
            }
        }
        None => HttpResponse::Unauthorized(),
    }
}
