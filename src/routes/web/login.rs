use actix_session::Session;
use actix_web::{HttpResponse, Responder, get};
use askama::Template;

#[derive(Template)]
#[template(path = "login.html")]
struct LoginTemplate {}

#[get("/login")]
async fn login(session: Session) -> impl Responder {
    if session
        .get::<String>("user_email")
        .unwrap_or(None)
        .is_some()
    {
        return HttpResponse::Found()
            .append_header(("Location", "/"))
            .finish();
    }

    let login = LoginTemplate {};
    HttpResponse::Ok().body(login.render().unwrap())
}
