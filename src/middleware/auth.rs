use actix_web::{
    body::BoxBody,
    dev::{ServiceRequest, ServiceResponse},
    Error, HttpResponse, middleware::Next,
};

const SESSION_COOKIE_NAME: &str = "rfinance_session";

pub async fn check_session(
    req: ServiceRequest,
    next: Next<BoxBody>,
) -> Result<ServiceResponse<BoxBody>, Error> {
    let path = req.path().to_string();

    if path == "/login" || path.starts_with("/api/") || path.starts_with("/css") || path.starts_with("/js") {
        return next.call(req).await.map(ServiceResponse::map_into_boxed_body);
    }

    if req.cookie(SESSION_COOKIE_NAME).is_none() {
        let (request, _) = req.into_parts();
        let response = HttpResponse::Found()
            .append_header(("Location", "/login"))
            .finish();
        return Ok(ServiceResponse::new(request, response));
    }

    next.call(req).await.map(ServiceResponse::map_into_boxed_body)
}
