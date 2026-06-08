use actix_session::Session;
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
async fn login(
    body: web::Json<LoginRequest>,
    pool: web::Data<PgPool>,
    session: Session,
) -> impl Responder {
    let result: Option<(String,)> = sqlx::query_as("SELECT password FROM users WHERE email = $1")
        .bind(&body.email)
        .fetch_optional(pool.get_ref())
        .await
        .unwrap();

    match result {
        Some((hashed_password,)) => {
            if bcrypt::verify(&body.password, &hashed_password).unwrap_or(false) {
                session.insert("user_email", &body.email).unwrap();

                let role: Option<(String,)> = sqlx::query_as(
                    "SELECT ud.role FROM users u
                     LEFT JOIN user_data ud ON ud.user_id = u.id
                     WHERE u.email = $1",
                )
                .bind(&body.email)
                .fetch_optional(pool.get_ref())
                .await
                .unwrap();

                let user_role = role.map(|(r,)| r).unwrap_or_else(|| "user".to_string());
                session.insert("user_role", &user_role).unwrap();

                HttpResponse::Ok().body("Login exitoso")
            } else {
                HttpResponse::Unauthorized().body("Credenciales incorrectas")
            }
        }
        None => HttpResponse::Unauthorized().body("Credenciales incorrectas"),
    }
}
