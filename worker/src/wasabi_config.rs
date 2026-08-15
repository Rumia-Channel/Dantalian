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
    pub async fn from_env(env: &Env) -> Result<Self> {
        Ok(Self {
            access_key_id: required_secret(
                env,
                "WASABI_ACCESS_KEY_ID_STORE",
                &["WASABI_ACCESS_KEY_ID"],
            )
            .await?,
            secret_access_key: required_secret(
                env,
                "WASABI_SECRET_ACCESS_KEY_STORE",
                &["WASABI_SECRET_ACCESS_KEY"],
            )
            .await?,
            endpoint: required_secret(env, "WASABI_ENDPOINT_STORE", &["WASABI_ENDPOINT"]).await?,
            region: required_secret(env, "WASABI_REGION_STORE", &["WASABI_REGION"]).await?,
            bucket: required_secret(
                env,
                "WASABI_BUCKET_STORE",
                &["WASABI_BUCKET", "DANTALIAN_BUCKET"],
            )
            .await?,
            prefix: env
                .var("WASABI_PREFIX")
                .ok()
                .map(|value| value.to_string())
                .filter(|value| !value.trim().is_empty()),
        })
    }
}

async fn required_secret(
    env: &Env,
    store_binding: &str,
    direct_bindings: &[&str],
) -> Result<String> {
    if let Ok(store) = env.secret_store(store_binding) {
        if let Some(value) = store.get().await? {
            return non_empty_secret(store_binding, value);
        }
    }
    for binding in direct_bindings {
        if let Ok(value) = env.secret(binding) {
            return non_empty_secret(binding, value.to_string());
        }
    }
    Err(worker::Error::RustError(format!(
        "{} is not configured",
        direct_bindings[0]
    )))
}

fn non_empty_secret(binding: &str, value: String) -> Result<String> {
    if value.trim().is_empty() {
        return Err(worker::Error::RustError(format!(
            "{binding} must not be empty"
        )));
    }
    Ok(value)
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
