use std::str::FromStr;

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum Role {
    Admin,
    User,
}

impl FromStr for Role {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "admin" => Ok(Role::Admin),
            "user" => Ok(Role::User),
            _ => Err(()),
        }
    }
}
