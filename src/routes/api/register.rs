use actix_web::{HttpResponse, Responder, web};
use bcrypt;
use serde::Deserialize;
use sqlx::PgPool;

/// Largo de los campos de texto de user_data. Más chico que el VARCHAR(255)
/// de la base: es política de producto, no backstop de esquema.
pub const MAX_NAME_LENGTH: usize = 100;

#[derive(Deserialize)]
pub struct RegisterRequest {
    email: String,
    password: String,
    name: String,
    last_name: String,
}

/// Validación ligera de email: un @, local y dominio no vacíos, dominio con
/// punto sin extremos sueltos, sin espacios y ≤255. Suficiente para que no
/// entren strings arbitrarios como «cuenta de correo»; la validez real la
/// confirma el flag del mail.
fn is_valid_email_format(email: &str) -> bool {
    if email.len() > 255 || email.contains(char::is_whitespace) {
        return false;
    }
    let Some((local, domain)) = email.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && !domain.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !domain.contains("..")
}

/// Política de contraseñas del server. El front también valida, pero la API
/// es pública y no se puede confiar en lo que dice el navegador: sin esto,
/// un curl directo registra cuentas con contraseña vacía. El tope de 72
/// bytes es donde bcrypt corta en silencio.
pub fn validate_password(password: &str) -> Result<(), String> {
    if password.chars().count() < 10 {
        return Err("La contraseña debe tener al menos 10 caracteres".to_string());
    }
    if password.len() > 72 {
        return Err(
            "La contraseña no puede superar los 72 caracteres (límite de bcrypt)".to_string(),
        );
    }
    Ok(())
}

/// Devuelve HttpResponse para cortar temprano con 400 y texto claro.
fn validate(body: &RegisterRequest) -> Result<(), HttpResponse> {
    if !is_valid_email_format(&body.email) {
        return Err(HttpResponse::BadRequest().body("El email no es válido"));
    }
    if let Err(msg) = validate_password(&body.password) {
        return Err(HttpResponse::BadRequest().body(msg));
    }
    if body.name.trim().is_empty() {
        return Err(HttpResponse::BadRequest().body("El nombre es obligatorio"));
    }
    if body.name.chars().count() > MAX_NAME_LENGTH {
        return Err(
            HttpResponse::BadRequest().body("El nombre no puede superar los 100 caracteres")
        );
    }
    if body.last_name.trim().is_empty() {
        return Err(HttpResponse::BadRequest().body("El apellido es obligatorio"));
    }
    if body.last_name.chars().count() > MAX_NAME_LENGTH {
        return Err(
            HttpResponse::BadRequest().body("El apellido no puede superar los 100 caracteres")
        );
    }
    Ok(())
}

/// POST /api/register. Sin macro de ruta: igual que login, se registra a
/// mano en main.rs para colgar el Governor de registro en su Resource.
pub async fn register(body: web::Json<RegisterRequest>, pool: web::Data<PgPool>) -> impl Responder {
    if let Err(res) = validate(&body) {
        return res;
    }

    let existing: Option<(i32,)> = match sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind(&body.email)
        .fetch_optional(pool.get_ref())
        .await
    {
        Ok(r) => r,
        Err(_) => return HttpResponse::InternalServerError().body("Error de base de datos"),
    };

    if existing.is_some() {
        return HttpResponse::BadRequest().body("El email ya está registrado");
    }

    let password_hashed = match bcrypt::hash(&body.password, bcrypt::DEFAULT_COST) {
        Ok(h) => h,
        Err(_) => {
            return HttpResponse::InternalServerError().body("Error al hashear la contraseña");
        }
    };

    // Carrera TOCTOU del check previo: si otro registro ganó la fila, el
    // UNIQUE de users salva la consistencia y se traduce en 400 en vez de un
    // panic del handler.
    let user_id: (i32,) =
        match sqlx::query_as("INSERT INTO users (email, password) VALUES ($1, $2) RETURNING id")
            .bind(&body.email)
            .bind(&password_hashed)
            .fetch_one(pool.get_ref())
            .await
        {
            Ok(id) => id,
            Err(e) => {
                if let Some(db) = e.as_database_error()
                    && db.is_unique_violation()
                {
                    return HttpResponse::BadRequest().body("El email ya está registrado");
                }
                return HttpResponse::InternalServerError().body("Error al crear el usuario");
            }
        };

    match sqlx::query("INSERT INTO user_data (user_id, name, last_name) VALUES ($1, $2, $3)")
        .bind(user_id.0)
        .bind(body.name.trim())
        .bind(body.last_name.trim())
        .execute(pool.get_ref())
        .await
    {
        Ok(_) => HttpResponse::Ok().body("Registro exitoso"),
        Err(_) => HttpResponse::InternalServerError().body("Error al crear los datos del usuario"),
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_NAME_LENGTH, is_valid_email_format, validate_password};

    #[test]
    fn email_format_esperado() {
        assert!(is_valid_email_format("dueño@example.com.ar"));
        assert!(is_valid_email_format("a.b@sub.example.org"));
    }

    #[test]
    fn email_format_rechaza() {
        assert!(!is_valid_email_format(""));
        assert!(!is_valid_email_format("sin-arroba"));
        assert!(!is_valid_email_format("@example.com"));
        assert!(!is_valid_email_format("local@"));
        assert!(!is_valid_email_format("local@dominio-sin-punto"));
        assert!(!is_valid_email_format("local@.example.com"));
        assert!(!is_valid_email_format("local@example.com."));
        assert!(!is_valid_email_format("local@ejemplo..com"));
        assert!(!is_valid_email_format("a b@example.com"));
        let largo = "a".repeat(256) + "@example.com";
        assert!(!is_valid_email_format(&largo));
    }

    #[test]
    fn password_esta_serio() {
        // 72 bytes exactos está permitido; 73 ya no (límite de bcrypt).
        assert!(validate_password(&"a".repeat(72)).is_ok());
        // Multibyte: el tope es en bytes, no en caracteres.
        assert!(validate_password(&"ñ".repeat(36)).is_ok());
        assert!(validate_password(&"ñ".repeat(37)).is_err());
    }

    #[test]
    fn password_mensajes() {
        assert_eq!(
            validate_password("corta").err().unwrap(),
            "La contraseña debe tener al menos 10 caracteres"
        );
        let larga = "a".repeat(73);
        assert_eq!(
            validate_password(&larga).err().unwrap(),
            "La contraseña no puede superar los 72 caracteres (límite de bcrypt)"
        );
    }

    #[test]
    fn limites_del_nombre() {
        assert!(MAX_NAME_LENGTH == 100);
    }
}
