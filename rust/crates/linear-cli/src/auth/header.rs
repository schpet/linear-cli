use crate::config::ConfigSecret;
use crate::graphql::transport::{ApiKey, ApiKeyError};

/// Fetch trims HTTP whitespace at header edges; preserve the stored secret.
pub fn to_api_key(secret: &ConfigSecret) -> Result<ApiKey, ApiKeyError> {
    let trimmed = secret.expose().trim_matches([' ', '\t', '\r', '\n']);
    ApiKey::new(trimmed.to_owned())
}
