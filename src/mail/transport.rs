use std::env;
use std::str::FromStr;
use std::time::Duration;

use lettre::message::{Mailbox, MultiPart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::transport::smtp::client::Tls;
use lettre::transport::smtp::client::TlsParameters;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

use crate::mail::MailMessage;

const DEFAULT_APP_URL: &str = "http://127.0.0.1:8080";
const SMTP_TIMEOUT: Duration = Duration::from_secs(20);

/// Error de envío. `permanent` distingue un rechazo definitivo (5xx,
/// destinatario inválido) de uno transitorio (timeout, 4xx, servidor caído),
/// que el worker reintenta con backoff.
#[derive(Debug, Clone)]
pub struct DeliveryError {
    pub message: String,
    pub permanent: bool,
}

impl DeliveryError {
    fn permanent(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            permanent: true,
        }
    }
}

#[derive(Clone)]
enum Transport {
    Smtp(AsyncSmtpTransport<Tokio1Executor>),
    Log,
}

/// Cliente de correo único del proceso. Con `MAIL_TRANSPORT=log` no toca la
/// red: imprime destinatario y asunto (el cuerpo queda en debug). Con
/// `disabled` los flujos que mandan correo quedan apagados.
#[derive(Clone)]
pub struct Mailer {
    transport: Option<Transport>,
    from: Mailbox,
    reply_to: Mailbox,
    pub app_url: String,
}

impl Mailer {
    /// Lee la configuración de `.env`. Es código de arranque: una variable
    /// mal puesta aborta el proceso con un mensaje concreto en vez de dejar
    /// el correo roto en silencio.
    pub fn from_env() -> Self {
        let is_production = env::var("APP_ENV").unwrap_or_default() == "production";

        let mode = env::var("MAIL_TRANSPORT")
            .unwrap_or_else(|_| {
                if is_production {
                    "smtp".to_string()
                } else {
                    "log".to_string()
                }
            })
            .trim()
            .to_ascii_lowercase();

        let app_url = env::var("APP_URL")
            .ok()
            .map(|url| url.trim().trim_end_matches('/').to_string())
            .filter(|url| !url.is_empty())
            .unwrap_or_else(|| {
                if is_production && mode != "disabled" {
                    panic!("APP_URL no está definida en .env (obligatoria con APP_ENV=production)");
                }
                DEFAULT_APP_URL.to_string()
            });
        if !app_url.starts_with("http://") && !app_url.starts_with("https://") {
            panic!("APP_URL inválida ({app_url:?}): usá http:// o https://");
        }

        // En producción el correo no puede quedar en modo simulado: si no
        // hay SMTP configurado, es mejor no arrancar que fingir envíos.
        let from = match env::var("MAIL_FROM") {
            Ok(value) if !value.trim().is_empty() => parse_mailbox(&value, "MAIL_FROM"),
            _ if mode == "smtp" => {
                panic!("MAIL_FROM no está definida en .env (obligatoria con MAIL_TRANSPORT=smtp)")
            }
            _ => parse_mailbox("no-reply@example.com", "MAIL_FROM"),
        };
        let reply_to = match env::var("MAIL_REPLY_TO") {
            Ok(value) if !value.trim().is_empty() => parse_mailbox(&value, "MAIL_REPLY_TO"),
            _ => from.clone(),
        };

        let transport = match mode.as_str() {
            "log" => Some(Transport::Log),
            "disabled" => None,
            "smtp" => Some(Transport::Smtp(build_smtp_transport())),
            other => panic!("MAIL_TRANSPORT inválida ({other:?}): usá smtp, log o disabled"),
        };

        Self {
            transport,
            from,
            reply_to,
            app_url,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.transport.is_some()
    }

    pub async fn send(&self, msg: &MailMessage) -> Result<(), DeliveryError> {
        let Some(transport) = &self.transport else {
            return Err(DeliveryError::permanent(
                "el envío de correo está deshabilitado",
            ));
        };
        let to: Mailbox = msg
            .to
            .parse()
            .map_err(|e| DeliveryError::permanent(format!("destinatario inválido: {e}")))?;

        let email = Message::builder()
            .from(self.from.clone())
            .reply_to(self.reply_to.clone())
            .to(to)
            .subject(&msg.subject)
            .multipart(MultiPart::alternative_plain_html(
                msg.text.clone(),
                msg.html.clone(),
            ))
            .map_err(|e| DeliveryError::permanent(format!("mensaje inválido: {e}")))?;

        match transport {
            Transport::Smtp(smtp) => {
                smtp.send(email)
                    .await
                    .map(|_| ())
                    .map_err(|e| DeliveryError {
                        message: e.to_string(),
                        permanent: e.is_permanent(),
                    })
            }
            Transport::Log => {
                log::info!("MAIL (log) → {} · {} · {}", msg.to, msg.kind, msg.subject);
                log::debug!("cuerpo del mail:\n{}", msg.text);
                Ok(())
            }
        }
    }
}

fn parse_mailbox(value: &str, key: &str) -> Mailbox {
    Mailbox::from_str(value.trim()).unwrap_or_else(|e| panic!("{key} inválida ({value:?}): {e}"))
}

fn build_smtp_transport() -> AsyncSmtpTransport<Tokio1Executor> {
    let host = env::var("SMTP_HOST")
        .unwrap_or_else(|_| panic!("SMTP_HOST no está definida en .env (MAIL_TRANSPORT=smtp)"));
    let port: u16 = env::var("SMTP_PORT")
        .unwrap_or_else(|_| "465".to_string())
        .trim()
        .parse()
        .unwrap_or_else(|_| panic!("SMTP_PORT inválido: debe ser un puerto numérico"));

    let tls = match env::var("SMTP_TLS")
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
    {
        Some(value) => value,
        None if port == 465 => "wrapper".to_string(),
        None => "starttls".to_string(),
    };
    if tls == "none" {
        let production = env::var("APP_ENV").unwrap_or_default() == "production";
        if production {
            panic!("SMTP_TLS=none no está permitido con APP_ENV=production");
        }
    }

    let builder = match tls.as_str() {
        "wrapper" => AsyncSmtpTransport::<Tokio1Executor>::relay(&host)
            .unwrap_or_else(|e| panic!("SMTP_HOST inválido ({host:?}): {e}")),
        "starttls" => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&host)
            .unwrap_or_else(|e| panic!("SMTP_HOST inválido ({host:?}): {e}")),
        "none" => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&host),
        other => panic!("SMTP_TLS inválida ({other:?}): usá wrapper, starttls o none"),
    };

    // `builder_dangerous` no habilita TLS por sí solo: lo dejo explícito
    // para que el modo none sea el único camino sin cifrado.
    let builder = if tls == "none" {
        builder.tls(Tls::None)
    } else if tls == "wrapper" {
        builder.tls(Tls::Wrapper(
            TlsParameters::new(host.clone())
                .unwrap_or_else(|e| panic!("SMTP_HOST inválido ({host:?}): {e}")),
        ))
    } else {
        builder.tls(Tls::Required(
            TlsParameters::new(host.clone())
                .unwrap_or_else(|e| panic!("SMTP_HOST inválido ({host:?}): {e}")),
        ))
    };

    let user = env::var("SMTP_USER").unwrap_or_default();
    let pass = env::var("SMTP_PASS").unwrap_or_default();
    let builder = match (user.trim().is_empty(), pass.is_empty()) {
        (true, true) => builder,
        (false, false) => builder.credentials(Credentials::new(user, pass)),
        _ => panic!("SMTP_USER y SMTP_PASS deben venir juntas (o ambas vacías)"),
    };

    builder.port(port).timeout(Some(SMTP_TIMEOUT)).build()
}
