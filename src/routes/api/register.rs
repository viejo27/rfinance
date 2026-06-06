use actix_web::{HttpResponse, Responder, post, web};
use bcrypt;
use serde::Deserialize;
use sqlx::PgPool;

#[derive(Deserialize)]
struct RegisterRequest {
    email: String,
    password: String,
    name: String,
    last_name: String,
}

#[post("/api/register")]
async fn register(body: web::Json<RegisterRequest>, pool: web::Data<PgPool>) -> impl Responder {
    let existing: Option<(i32,)> = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind(&body.email)
        .fetch_optional(pool.get_ref())
        .await
        .expect("Error checking email");

    if existing.is_some() {
        return HttpResponse::BadRequest().body("El email ya está registrado");
    }

    let password_hashed =
        bcrypt::hash(&body.password, bcrypt::DEFAULT_COST).expect("Error al hashear la contraseña");

    let user_id: (i32,) =
        sqlx::query_as("INSERT INTO users (email, password) VALUES ($1, $2) RETURNING id")
            .bind(&body.email)
            .bind(&password_hashed)
            .fetch_one(pool.get_ref())
            .await
            .expect("Error al crear el usuario");

    sqlx::query("INSERT INTO user_data (user_id, name, last_name) VALUES ($1, $2, $3)")
        .bind(user_id.0)
        .bind(&body.name)
        .bind(&body.last_name)
        .execute(pool.get_ref())
        .await
        .expect("Error al crear los datos del usuario");

    HttpResponse::Ok().body("Registro exitoso")
}
