use actix_session::SessionExt;
use actix_web::{
    Error, HttpResponse,
    body::BoxBody,
    dev::{ServiceRequest, ServiceResponse},
    middleware::Next,
};

/// Paths que corren sin sesión: página de login, los tres endpoints
/// públicos de la API y los assets estáticos.
fn is_public_path(path: &str) -> bool {
    path == "/login"
        || path == "/api/login"
        || path == "/api/register"
        || path == "/api/logout"
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

    if path == "/register" {
        if has_session {
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
    if path.starts_with("/api/") && !has_session {
        let (request, _) = req.into_parts();
        let response = HttpResponse::Unauthorized().body("Sesión expirada — ingresa de nuevo");
        return Ok(ServiceResponse::new(request, response));
    }

    if !has_session {
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
