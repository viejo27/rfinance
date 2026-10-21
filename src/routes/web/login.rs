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
    HttpResponse::Ok()
        // Sin content-type el HTML servía como texto plano (el sniffing lo
        // disimulaba; nosniff lo expuso). Mismo patrón que home.rs.
        .content_type("text/html; charset=utf-8")
        .body(login.render().unwrap())
}
