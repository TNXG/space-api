use config::{Config as ConfigLoader, Environment, File};
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
    let config_path = env::var("CONFIG_PATH").unwrap_or_else(|_| "config.toml".to_string());

    let s = ConfigLoader::builder()
        // 1. 设置默认值 (可选，这里略过，依靠 Result 处理或 Serde default)
        // 2. 加载配置文件 (如果存在)
        .add_source(File::with_name(&config_path).required(false))
        // 3. 加载环境变量 (例如 SPACE_API_MONGO__HOST 覆盖 [mongo] host)
        .add_source(Environment::with_prefix("SPACE_API").separator("__"))
        .build()
        .unwrap_or_else(|e| panic!("Failed to build configuration: {}", e));

    s.try_deserialize()
        .unwrap_or_else(|e| panic!("Failed to deserialize configuration: {}", e))
}