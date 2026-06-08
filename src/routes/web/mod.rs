pub mod admin;
pub mod home;
pub mod login;
pub mod register;

pub use admin::admin as admin_handler;
pub use home::index as index_handler;
pub use login::login as login_handler;
pub use register::register as register_handler;
