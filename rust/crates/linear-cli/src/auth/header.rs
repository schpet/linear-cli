use crate::config::ConfigSecret;
use crate::graphql::transport::{ApiKey, ApiKeyError};

/// Trims surrounding whitespace, which is never part of a key; the stored
/// secret is left unchanged.
pub fn to_api_key(secret: &ConfigSecret) -> Result<ApiKey, ApiKeyError> {
    let trimmed = secret.expose().trim_matches([' ', '\t', '\r', '\n']);
    ApiKey::new(trimmed.to_owned())
}
