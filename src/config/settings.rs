use config::{Config as ConfigLoader, Environment, File};
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub mongo: MongoConfig,
    pub email: EmailConfig,
    pub oauth: OAuthConfig,
    #[serde(default)]
    pub image_proxy: ImageProxyConfig,
    #[serde(default)]
    pub memory: MemoryConfig,
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
    /// 允许的 return_url 域名白名单（为空则允许所有，但生产环境建议配置）
    #[serde(default)]
    pub allowed_return_domains: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageProxyConfig {
    /// 允许调用图像代理的来源域名，自动包含其子域名
    #[serde(default = "default_image_proxy_referrer_domains")]
    pub allowed_referrer_domains: Vec<String>,
    /// 是否允许不携带 Origin/Referer 的请求
    #[serde(default)]
    pub allow_missing_referrer: bool,
    /// 单张源图允许下载的最大字节数
    #[serde(default = "default_image_proxy_max_bytes")]
    pub max_image_bytes: usize,
}

impl Default for ImageProxyConfig {
    fn default() -> Self {
        Self {
            allowed_referrer_domains: default_image_proxy_referrer_domains(),
            allow_missing_referrer: false,
            max_image_bytes: default_image_proxy_max_bytes(),
        }
    }
}

fn default_image_proxy_referrer_domains() -> Vec<String> {
    vec!["tnxg.moe".to_string(), "tnxg.top".to_string()]
}

fn default_image_proxy_max_bytes() -> usize {
    10 * 1024 * 1024
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    /// 内存阈值（MB），超过此值将触发全局内存释放
    #[serde(default = "default_memory_threshold")]
    pub threshold_mb: u64,
    /// 内存监控检查间隔（秒）
    #[serde(default = "default_check_interval")]
    pub check_interval_secs: u64,
    /// 垃圾回收冷却时间（秒），避免频繁GC
    #[serde(default = "default_gc_cooldown")]
    pub gc_cooldown_secs: u64,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            threshold_mb: default_memory_threshold(),
            check_interval_secs: default_check_interval(),
            gc_cooldown_secs: default_gc_cooldown(),
        }
    }
}

fn default_memory_threshold() -> u64 {
    500
}

fn default_check_interval() -> u64 {
    30
}

fn default_gc_cooldown() -> u64 {
    30
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
