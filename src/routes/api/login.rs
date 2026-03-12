use actix_web::{HttpResponse, Responder, cookie::Cookie, post, web};
use bcrypt;
use serde::Deserialize;
use sqlx::PgPool;

const SESSION_COOKIE_NAME: &str = "rfinance_session";
const SESSION_DURATION_DAYS: i64 = 27;

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
                let cookie = Cookie::build(SESSION_COOKIE_NAME, &body.email)
                    .path("/")
                    .max_age(time::Duration::days(SESSION_DURATION_DAYS))
                    .http_only(true)
                    .finish();
                HttpResponse::Ok().cookie(cookie).body("Login exitoso")
            } else {
                HttpResponse::Unauthorized().body("Credenciales incorrectas")
            }
        }
        None => HttpResponse::Unauthorized().body("Credenciales incorrectas"),
    }
}
