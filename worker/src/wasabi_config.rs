use worker::{Env, Result};

#[derive(Debug, Clone)]
pub struct WasabiConfig {
    pub access_key_id: String,
    pub secret_access_key: String,
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    pub prefix: Option<String>,
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

fn required_var(env: &Env, name: &str) -> Result<String> {
    let value = env.var(name)?.to_string();
    if value.trim().is_empty() {
        return Err(worker::Error::RustError(format!(
            "{name} must not be empty"
        )));
    }
    Ok(value)
}
