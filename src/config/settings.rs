use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub mongo: MongoConfig,
    pub email: EmailConfig,
    pub oauth: OAuthConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MongoConfig {
    pub host: String,
    pub port: u16,
    pub user: Option<String>,
    pub password: Option<String>,
    pub database: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailConfig {
    pub smtp_server: String,
    pub smtp_port: u16,
    pub username: String,
    pub password: String,
    pub from_address: String,
    pub from_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthConfig {
    pub qq_app_id: String,
    pub qq_app_key: String,
    pub redirect_uri: String,
}

pub fn load_config() -> Config {
    Config {
        mongo: MongoConfig {
            host: env::var("MONGO_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
            port: env::var("MONGO_PORT")
                .unwrap_or_else(|_| "27017".to_string())
                .parse()
                .unwrap_or(27017),
            user: env::var("MONGO_USER").ok(),
            password: env::var("MONGO_PASSWORD").ok(),
            database: env::var("MONGO_DB").unwrap_or_else(|_| "space-api".to_string()),
        },
        email: EmailConfig {
            smtp_server: env::var("EMAIL_HOST").unwrap_or_else(|_| "smtp.example.com".to_string()),
            smtp_port: env::var("EMAIL_PORT")
                .unwrap_or_else(|_| "587".to_string())
                .parse()
                .unwrap_or(587),
            username: env::var("EMAIL_USER").unwrap_or_default(),
            password: env::var("EMAIL_PASS").unwrap_or_default(),
            from_address: env::var("EMAIL_USER").unwrap_or_else(|_| "noreply@example.com".to_string()),
            from_name: env::var("EMAIL_FROM_NAME").unwrap_or_else(|_| "天翔TNXG".to_string()),
        },
        oauth: OAuthConfig {
            qq_app_id: env::var("QQ_APP_ID")
                .or_else(|_| env::var("QQ_CLIENT_ID"))
                .unwrap_or_default(),
            qq_app_key: env::var("QQ_APP_KEY")
                .or_else(|_| env::var("QQ_CLIENT_SECRET"))
                .unwrap_or_default(),
            redirect_uri: env::var("QQ_REDIRECT_URI").unwrap_or_default(),
        },
    }
}