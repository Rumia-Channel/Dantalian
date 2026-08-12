use worker::{Env, Result};

#[derive(Clone)]
pub struct WasabiConfig {
    pub access_key_id: String,
    pub secret_access_key: String,
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    pub prefix: Option<String>,
}
impl std::fmt::Debug for WasabiConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WasabiConfig")
            .field("access_key_id", &"***")
            .field("secret_access_key", &"***")
            .field("endpoint", &self.endpoint)
            .field("region", &self.region)
            .field("bucket", &self.bucket)
            .field("prefix", &self.prefix)
            .finish()
    }
}

impl WasabiConfig {
    pub fn from_env(env: &Env) -> Result<Self> {
        Ok(Self {
            access_key_id: env.secret("WASABI_ACCESS_KEY_ID")?.to_string(),
            secret_access_key: env.secret("WASABI_SECRET_ACCESS_KEY")?.to_string(),
            endpoint: required_var(env, "WASABI_ENDPOINT")?,
            region: required_var(env, "WASABI_REGION")?,
            bucket: required_var(env, "WASABI_BUCKET")?,
            prefix: env
                .var("WASABI_PREFIX")
                .ok()
                .map(|value| value.to_string())
                .filter(|value| !value.trim().is_empty()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_does_not_expose_credentials() {
        let config = WasabiConfig {
            access_key_id: "access-key".to_string(),
            secret_access_key: "secret-key".to_string(),
            endpoint: "https://s3.example.test".to_string(),
            region: "us-east-1".to_string(),
            bucket: "private".to_string(),
            prefix: Some("production".to_string()),
        };
        let debug = format!("{config:?}");
        assert!(!debug.contains("access-key"));
        assert!(!debug.contains("secret-key"));
        assert!(debug.contains("endpoint"));
        assert!(debug.contains("***"));
    }
}

fn required_var(env: &Env, name: &str) -> Result<String> {
    let value = env.var(name)?.to_string();
    if value.trim().is_empty() {
        return Err(worker::Error::RustError(format!(
            "{name} must not be empty"
        )));
    }
    Ok(value)
}
