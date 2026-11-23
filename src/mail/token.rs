use sha2::{Digest, Sha256};

/// 32 bytes aleatorios en hex: 64 caracteres seguros para un enlace.
/// El valor en claro solo se manda por correo; en la base queda el hash.
pub const TOKEN_LENGTH: usize = 64;

pub fn generate() -> String {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).expect("no se pudo generar un token seguro");
    hex::encode(bytes)
}

pub fn hash(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

pub fn is_well_formed(token: &str) -> bool {
    token.len() == TOKEN_LENGTH && token.bytes().all(|b| b.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::{TOKEN_LENGTH, generate, hash, is_well_formed};

    #[test]
    fn el_token_es_hex_del_largo_esperado() {
        let token = generate();
        assert_eq!(token.len(), TOKEN_LENGTH);
        assert!(is_well_formed(&token));
        assert_ne!(token, generate());
    }

    #[test]
    fn el_hash_es_estable_y_no_reversible() {
        let token = "a".repeat(TOKEN_LENGTH);
        assert_eq!(hash(&token), hash(&token));
        assert_eq!(hash(&token).len(), TOKEN_LENGTH);
        assert_ne!(hash(&token), token);
    }

    #[test]
    fn rechaza_tokens_mal_formados() {
        assert!(!is_well_formed(""));
        assert!(!is_well_formed("abc"));
        assert!(!is_well_formed(&"g".repeat(TOKEN_LENGTH)));
        assert!(!is_well_formed(&"a".repeat(TOKEN_LENGTH + 1)));
    }
}
