use actix_session::{Session, SessionExt};
use actix_web::{
    Error, HttpResponse,
    body::BoxBody,
    dev::{ServiceRequest, ServiceResponse},
    middleware::Next,
    web,
};
use sqlx::PgPool;

/// Paths que corren sin sesión: login, recuperación de contraseña,
/// verificación de email, los endpoints públicos de la API y los assets.
fn is_public_path(path: &str) -> bool {
    path == "/login"
        || path == "/api/login"
        || path == "/api/register"
        || path == "/api/logout"
        || path == "/api/forgot-password"
        || path == "/api/reset-password"
        || path == "/forgot-password"
        || path == "/reset-password"
        || path == "/verify-email"
        || path.starts_with("/css")
        || path.starts_with("/js")
        || path.starts_with("/fonts")
        || path.starts_with("/textures")
        || path == "/favicon.ico"
        || path == "/favicon-512.png"
}

pub async fn check_session(
    req: ServiceRequest,
    next: Next<BoxBody>,
) -> Result<ServiceResponse<BoxBody>, Error> {
    let path = req.path().to_string();

    if is_public_path(&path) {
        return next
            .call(req)
            .await
            .map(ServiceResponse::map_into_boxed_body);
    }

    let has_session = req
        .get_session()
        .get::<String>("user_email")
        .unwrap_or(None)
        .is_some();
    let session_valid = if has_session {
        session_is_current(&req).await?
    } else {
        false
    };

    if path == "/register" {
        if session_valid {
            let (request, _) = req.into_parts();
            let response = HttpResponse::Found()
                .append_header(("Location", "/"))
                .finish();
            return Ok(ServiceResponse::new(request, response));
        }
        return next
            .call(req)
            .await
            .map(ServiceResponse::map_into_boxed_body);
    }

    // La API financiera responde 401 en vez de redirigir: los fetch del
    // frontend distinguen "sin sesión" del HTML de login.
    if path.starts_with("/api/") && !session_valid {
        let (request, _) = req.into_parts();
        let response = HttpResponse::Unauthorized().body("Sesión expirada — ingresa de nuevo");
        return Ok(ServiceResponse::new(request, response));
    }

    if !session_valid {
        let (request, _) = req.into_parts();
        let response = HttpResponse::Found()
            .append_header(("Location", "/login"))
            .finish();
        return Ok(ServiceResponse::new(request, response));
    }

    next.call(req)
        .await
        .map(ServiceResponse::map_into_boxed_body)
}

/// Valida el epoch de la cookie contra `users.session_epoch`. Un reset de
/// contraseña sube el epoch en la base: la cookie vieja deja de valer en el
/// siguiente request, aunque el cookie cifrado siga siendo legítimo.
async fn session_is_current(req: &ServiceRequest) -> Result<bool, Error> {
    let session: Session = req.get_session();
    let Some(user_id) = session.get::<i32>("user_id").unwrap_or(None) else {
        session.clear();
        return Ok(false);
    };
    let cookie_epoch = session
        .get::<i32>("user_epoch")
        .unwrap_or(None)
        .unwrap_or(0);

    let Some(pool) = req.app_data::<web::Data<PgPool>>() else {
        log::error!("auth: PgPool no disponible para validar la sesión");
        return Err(actix_web::error::ErrorInternalServerError(
            "Error al validar la sesión",
        ));
    };

    match sqlx::query_as::<_, (i32,)>("SELECT session_epoch FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(pool.get_ref())
        .await
    {
        Ok(Some((db_epoch,))) if db_epoch == cookie_epoch => Ok(true),
        Ok(_) => {
            // Usuario borrado o epoch desactualizado: la cookie ya no vale.
            session.clear();
            Ok(false)
        }
        Err(e) => {
            log::error!("auth: no se pudo validar el epoch de sesión: {e}");
            Err(actix_web::error::ErrorInternalServerError(
                "Error al validar la sesión",
            ))
        }
    }
}
