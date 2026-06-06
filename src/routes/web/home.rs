use actix_session::Session;
use actix_web::{HttpResponse, Responder, get};
use askama::Template;

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate<'a> {
    email: &'a str,
}

#[get("/")]
async fn index(session: Session) -> impl Responder {
    let email = session
        .get::<String>("user_email")
        .unwrap_or(None)
        .unwrap_or_default();
    let hello = IndexTemplate { email: &email };
    HttpResponse::Ok().body(hello.render().unwrap())
}
