use actix_session::SessionExt;
use actix_web::{
    Error,
    body::{BoxBody, EitherBody},
    dev::{ServiceRequest, ServiceResponse},
    http::Method,
    middleware::Next,
};

/// Auditoría de mutaciones: para cada POST/PATCH/DELETE de la API deja una
/// línea estructurada (método, ruta, estado, usuario de sesión). Sin bodies:
/// por acá nunca circulan credenciales ni datos financieros. Complementa el
/// log de acceso de actix, que no sabe de quién es la sesión.
pub async fn audit(
    req: ServiceRequest,
    next: Next<EitherBody<BoxBody>>,
) -> Result<ServiceResponse<BoxBody>, Error> {
    let method = req.method().clone();
    let path = req.path().to_string();
    let is_mutation = matches!(method, Method::POST | Method::PATCH | Method::DELETE)
        && path.starts_with("/api/");
    let user_id = if is_mutation {
        req.get_session().get::<i32>("user_id").ok().flatten()
    } else {
        None
    };

    let res = next
        .call(req)
        .await
        .map(ServiceResponse::map_into_boxed_body)?;

    if is_mutation {
        log::info!(
            "AUDIT method={} path={} status={} user_id={}",
            method,
            path,
            res.status(),
            user_id.map_or_else(|| "-".to_string(), |id| id.to_string())
        );
    }
    Ok(res)
}
