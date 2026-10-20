use actix_session::Session;
use actix_web::{HttpResponse, Responder, web};
use bcrypt;
use serde::Deserialize;
use sqlx::PgPool;
use std::sync::OnceLock;

#[derive(Deserialize)]
pub struct LoginRequest {
    email: String,
    password: String,
}

/// Hash bcrypt de una contraseña aleatoria, generado una vez por proceso.
/// Sirve para que el camino de «email inexistente» tarte igual que el de
/// «contraseña incorrecta»: sin esto, el tiempo de respuesta dice qué
/// emails están registrados.
fn dummy_hash() -> &'static str {
    static DUMMY: OnceLock<String> = OnceLock::new();
    DUMMY.get_or_init(|| {
        let ruido = format!("login-dummy-{}", std::process::id());
        bcrypt::hash(&ruido, bcrypt::DEFAULT_COST).expect("hash dummy para login")
    })
}

/// POST /api/login. Sin macro de ruta: se registra a mano en main.rs
/// dentro de un Resource con el Governor propio, porque el struct que
/// genera `#[post]` no expone el Resource para envolver por endpoint.
pub async fn login(
    body: web::Json<LoginRequest>,
    pool: web::Data<PgPool>,
    session: Session,
) -> impl Responder {
    let result: Option<(String, i32)> =
        match sqlx::query_as("SELECT password, id FROM users WHERE email = $1")
            .bind(&body.email)
            .fetch_optional(pool.get_ref())
            .await
        {
            Ok(r) => r,
            Err(_) => return HttpResponse::InternalServerError().body("Error de base de datos"),
        };

    match result {
        Some((hashed_password, user_id)) => {
            if bcrypt::verify(&body.password, &hashed_password).unwrap_or(false) {
                if session.insert("user_email", &body.email).is_err()
                    || session.insert("user_id", user_id).is_err()
                {
                    return HttpResponse::InternalServerError().body("Error al crear la sesión");
                }

                let role: Option<(String,)> = match sqlx::query_as(
                    "SELECT ud.role FROM users u
                     LEFT JOIN user_data ud ON ud.user_id = u.id
                     WHERE u.email = $1",
                )
                .bind(&body.email)
                .fetch_optional(pool.get_ref())
                .await
                {
                    Ok(r) => r,
                    Err(_) => {
                        return HttpResponse::InternalServerError().body("Error de base de datos");
                    }
                };

                let user_role = role.map(|(r,)| r).unwrap_or_else(|| "user".to_string());
                if session.insert("user_role", &user_role).is_err() {
                    return HttpResponse::InternalServerError().body("Error al crear la sesión");
                }

                HttpResponse::Ok().body("Login exitoso")
            } else {
                HttpResponse::Unauthorized().body("Credenciales incorrectas")
            }
        }
        None => {
            // Email inexistente: se quema el mismo costo de bcrypt y se
            // responde con el mismo mensaje, sin filtrar cuentas.
            let _ = bcrypt::verify(&body.password, dummy_hash());
            HttpResponse::Unauthorized().body("Credenciales incorrectas")
        }
    }
}
