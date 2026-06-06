use actix_session::SessionExt;
use actix_web::{
    Error, HttpResponse,
    body::BoxBody,
    dev::{ServiceRequest, ServiceResponse},
    middleware::Next,
};

pub async fn check_session(
    req: ServiceRequest,
    next: Next<BoxBody>,
) -> Result<ServiceResponse<BoxBody>, Error> {
    let path = req.path().to_string();

    if path == "/login"
        || path.starts_with("/api/")
        || path.starts_with("/css")
        || path.starts_with("/js")
    {
        return next
            .call(req)
            .await
            .map(ServiceResponse::map_into_boxed_body);
    }

    if path == "/register" {
        if req
            .get_session()
            .get::<String>("user_email")
            .unwrap_or(None)
            .is_some()
        {
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

    if req
        .get_session()
        .get::<String>("user_email")
        .unwrap_or(None)
        .is_none()
    {
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
