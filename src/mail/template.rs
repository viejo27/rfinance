use askama::Template;

use crate::mail::MailMessage;

#[derive(Template)]
#[template(path = "email/verify.html")]
struct VerifyHtml<'a> {
    name: &'a str,
    verify_url: &'a str,
    app_url: &'a str,
    headline: &'a str,
    intro: &'a str,
}

#[derive(Template)]
#[template(path = "email/verify.txt")]
struct VerifyText<'a> {
    name: &'a str,
    verify_url: &'a str,
    headline: &'a str,
    intro: &'a str,
}

#[derive(Template)]
#[template(path = "email/password_reset.html")]
struct PasswordResetHtml<'a> {
    name: &'a str,
    reset_url: &'a str,
    app_url: &'a str,
}

#[derive(Template)]
#[template(path = "email/password_reset.txt")]
struct PasswordResetText<'a> {
    name: &'a str,
    reset_url: &'a str,
}

#[derive(Template)]
#[template(path = "email/password_changed.html")]
struct PasswordChangedHtml<'a> {
    name: &'a str,
    app_url: &'a str,
}

#[derive(Template)]
#[template(path = "email/password_changed.txt")]
struct PasswordChangedText<'a> {
    name: &'a str,
    forgot_url: &'a str,
}

#[derive(Template)]
#[template(path = "email/test.html")]
struct TestHtml<'a> {
    app_url: &'a str,
}

#[derive(Template)]
#[template(path = "email/test.txt")]
struct TestText;

fn message(to: &str, subject: &str, kind: &str, text: String, html: String) -> MailMessage {
    MailMessage {
        to: to.to_string(),
        subject: subject.to_string(),
        text,
        html,
        kind: kind.to_string(),
    }
}

pub fn welcome(
    to: &str,
    name: &str,
    verify_url: &str,
    app_url: &str,
) -> Result<MailMessage, askama::Error> {
    let headline = "TE DAMOS LA BIENVENIDA";
    let intro = "Tu cuenta en RFinance ya existe. Verificá tu email para completar el alta y poder recuperar tu contraseña si algún día la olvidás.";
    let html = VerifyHtml {
        name,
        verify_url,
        app_url,
        headline,
        intro,
    }
    .render()?;
    let text = VerifyText {
        name,
        verify_url,
        headline,
        intro,
    }
    .render()?;
    Ok(message(
        to,
        "Bienvenido a RFinance — verificá tu email",
        "welcome",
        text,
        html,
    ))
}

pub fn verification(
    to: &str,
    name: &str,
    verify_url: &str,
    app_url: &str,
) -> Result<MailMessage, askama::Error> {
    let headline = "VERIFICÁ TU EMAIL";
    let intro = "Pediste un enlace nuevo para verificar tu email. Usá el botón de abajo para confirmar la dirección.";
    let html = VerifyHtml {
        name,
        verify_url,
        app_url,
        headline,
        intro,
    }
    .render()?;
    let text = VerifyText {
        name,
        verify_url,
        headline,
        intro,
    }
    .render()?;
    Ok(message(
        to,
        "Verificá tu email en RFinance",
        "verify",
        text,
        html,
    ))
}

pub fn password_reset(
    to: &str,
    name: &str,
    reset_url: &str,
    app_url: &str,
) -> Result<MailMessage, askama::Error> {
    let html = PasswordResetHtml {
        name,
        reset_url,
        app_url,
    }
    .render()?;
    let text = PasswordResetText { name, reset_url }.render()?;
    Ok(message(
        to,
        "Restablecé tu contraseña — RFinance",
        "password_reset",
        text,
        html,
    ))
}

pub fn password_changed(to: &str, name: &str, app_url: &str) -> Result<MailMessage, askama::Error> {
    let html = PasswordChangedHtml { name, app_url }.render()?;
    let forgot_url = format!("{app_url}/forgot-password");
    let text = PasswordChangedText {
        name,
        forgot_url: &forgot_url,
    }
    .render()?;
    Ok(message(
        to,
        "Tu contraseña cambió — RFinance",
        "password_changed",
        text,
        html,
    ))
}

pub fn test(to: &str, app_url: &str) -> Result<MailMessage, askama::Error> {
    let html = TestHtml { app_url }.render()?;
    let text = TestText.render()?;
    Ok(message(
        to,
        "Prueba de correo — RFinance",
        "test",
        text,
        html,
    ))
}
