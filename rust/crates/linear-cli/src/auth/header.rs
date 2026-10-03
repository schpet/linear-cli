use crate::config::ConfigSecret;
use crate::graphql::transport::{ApiKey, ApiKeyError};

/// Trims surrounding whitespace, which is never part of a key; the stored
/// secret is left unchanged.
pub fn to_api_key(secret: &ConfigSecret) -> Result<ApiKey, ApiKeyError> {
    let trimmed = secret.expose().trim_matches([' ', '\t', '\r', '\n']);
    ApiKey::new(trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surrounding_whitespace_is_dropped_but_the_secret_is_unchanged() {
        let secret = ConfigSecret::new(" \t\r\nlin_api_fake\n ".to_owned());
        to_api_key(&secret).expect("header");
        assert_eq!(secret.expose(), " \t\r\nlin_api_fake\n ");
        let error =
            to_api_key(&ConfigSecret::new(" \t\r\n ".to_owned())).expect_err("empty after trim");
        assert_eq!(error, ApiKeyError::Empty);
        let secret = ConfigSecret::new("ab\tcdlin_api_fake".to_owned());
        let error = to_api_key(&secret).expect_err("interior tab");
        assert_eq!(error, ApiKeyError::InvalidByte { index: 2 });
        let error = to_api_key(&ConfigSecret::new(" \tab\tc".to_owned()))
            .expect_err("index is measured after edge whitespace is trimmed");
        assert_eq!(error, ApiKeyError::InvalidByte { index: 2 });
        assert!(!format!("{error:?} {error} {secret:?}").contains("lin_api_fake"));
    }
}
