use std::env;

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub psp_base_url: String,
    pub psp_timeout_ms: u64,
    pub webhook_timeout_ms: u64,
    pub webhook_max_retries: i32,
    pub api_key_pepper: String,
    pub dev_api_key_id: String,
    pub dev_api_key_secret: String,
    pub bind_address: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/invoices".into()),
            psp_base_url: env::var("PSP_BASE_URL")
                .unwrap_or_else(|_| "http://mock-psp:8081".into()),
            psp_timeout_ms: env::var("PSP_TIMEOUT_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(2_000),
            webhook_timeout_ms: env::var("WEBHOOK_TIMEOUT_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(2_000),
            webhook_max_retries: env::var("WEBHOOK_MAX_RETRIES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(5),
            api_key_pepper: env::var("API_KEY_PEPPER")
                .unwrap_or_else(|_| "development-pepper".into()),
            dev_api_key_id: env::var("DEV_API_KEY_ID").unwrap_or_else(|_| "dev_key".into()),
            dev_api_key_secret: env::var("DEV_API_KEY_SECRET")
                .unwrap_or_else(|_| "dev_secret".into()),
            bind_address: env::var("API_BIND_ADDRESS").unwrap_or_else(|_| "0.0.0.0:8080".into()),
        }
    }
}
