use actix_web::{HttpResponse, Responder, get};
use askama::Template;

const SESSION_COOKIE_NAME: &str = "rfinance_session";

#[derive(Template)]
#[template(path = "login.html")]
struct LoginTemplate {}

#[get("/login")]
async fn login(req: actix_web::HttpRequest) -> impl Responder {
    if req.cookie(SESSION_COOKIE_NAME).is_some() {
        return HttpResponse::Found()
            .append_header(("Location", "/"))
            .finish();
    }

    let login = LoginTemplate {};
    HttpResponse::Ok().body(login.render().unwrap())
}
