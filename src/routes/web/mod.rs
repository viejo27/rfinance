pub mod admin;
pub mod email_verification;
pub mod home;
pub mod login;
pub mod password_reset;
pub mod portfolio;
pub mod register;

pub use admin::admin as admin_handler;
pub use admin::mail_test as admin_mail_test;
pub use email_verification::verify_email as verify_email_handler;
pub use home::index as index_handler;
pub use login::login as login_handler;
pub use password_reset::forgot_password as forgot_password_handler;
pub use password_reset::reset_password as reset_password_handler;
pub use portfolio::portfolio as portfolio_handler;
pub use register::register as register_handler;
