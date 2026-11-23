pub mod outbox;
pub mod template;
pub mod token;
pub mod transport;
pub mod worker;

pub use transport::Mailer;

/// Un correo listo para encolar: texto plano + HTML. El `kind` es la
/// etiqueta de auditoría que queda en `email_outbox`.
#[derive(Debug, Clone)]
pub struct MailMessage {
    pub to: String,
    pub subject: String,
    pub text: String,
    pub html: String,
    pub kind: String,
}
