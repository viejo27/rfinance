use actix_web::{
    Error,
    body::BoxBody,
    dev::{ServiceRequest, ServiceResponse},
    http::header::{
        CONTENT_SECURITY_POLICY, HeaderValue, REFERRER_POLICY, X_CONTENT_TYPE_OPTIONS,
        X_FRAME_OPTIONS,
    },
    middleware::Next,
};

/// Cabeceras de seguridad para toda respuesta, incluyendo los 401/302 que
/// cortan temprano y por eso no las recibirían si el middleware estuviera
/// más adentro del stack.
///
/// La CSP puede ser estricta porque ningún template tiene scripts ni
/// estilos inline: el script de admin.html vive en /js/admin.js y los
/// estilos que estaban inline en login/register son clases de style.css.
pub async fn set_security_headers(
    req: ServiceRequest,
    next: Next<BoxBody>,
) -> Result<ServiceResponse<BoxBody>, Error> {
    let mut res = next.call(req).await?;
    {
        let headers = res.headers_mut();
        headers.insert(
            CONTENT_SECURITY_POLICY,
            HeaderValue::from_static("default-src 'self'; frame-ancestors 'none'"),
        );
        headers.insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
        headers.insert(X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
        headers.insert(
            REFERRER_POLICY,
            HeaderValue::from_static("strict-origin-when-cross-origin"),
        );
    }
    // Normalizo el body boxed: los wraps exteriores esperan BoxBody
    // (misma convención que check_session en auth.rs).
    Ok(res.map_into_boxed_body())
}
