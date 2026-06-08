use actix_session::SessionExt;
use actix_web::{Error, dev::ServiceRequest};
use std::collections::HashSet;

use crate::role::Role;

pub async fn extract(req: &ServiceRequest) -> Result<HashSet<Role>, Error> {
    let session = req.get_session();

    let mut roles = HashSet::new();

    if let Some(role_str) = session.get::<String>("user_role").ok().flatten() {
        if let Ok(role) = role_str.parse::<Role>() {
            roles.insert(role);
        }
    }

    Ok(roles)
}
