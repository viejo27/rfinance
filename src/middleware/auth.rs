use actix_web::{dev::ServiceRequest, Error, HttpMessage};
use actix_web::error::ErrorUnauthorized;

const SESSION_COOKIE_NAME: &str = "rfinance_session";

pub async fn check_session(req: ServiceRequest, _credentials: ()) -> Result<ServiceRequest, (Error, ServiceRequest)> {
    let session_cookie = req.cookie(SESSION_COOKIE_NAME);
    
    match session_cookie {
        Some(cookie) => {
            let email = cookie.value();
            if !email.is_empty() {
                req.extensions_mut().insert(email.to_string());
                Ok(req)
            } else {
                Err((ErrorUnauthorized("Unauthorized"), req))
            }
        }
        None => Err((ErrorUnauthorized("Unauthorized"), req)),
    }
}
