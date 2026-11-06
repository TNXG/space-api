#[cfg(test)]
mod tests {
    use super::validate_image_proxy_source;
    use crate::config::settings::{Config, ImageProxyConfig};

    fn config_with_image_proxy(image_proxy: ImageProxyConfig) -> Config {
        // 该测试只关心图像代理配置，因此从示例配置加载其余必填字段并覆盖目标部分。
        let mut config: Config = config::Config::builder()
            .add_source(config::File::with_name("config.example.toml"))
            .build()
            .expect("example config should load")
            .try_deserialize()
            .expect("example config should deserialize");
        config.image_proxy = image_proxy;
        config
    }

    #[test]
    fn allows_configured_domains_and_nested_subdomains() {
        let config = config_with_image_proxy(ImageProxyConfig::default());

        assert!(validate_image_proxy_source(Some("https://tnxg.top/page"), &config).is_ok());
        assert!(
            validate_image_proxy_source(Some("https://img.cdn.tnxg.moe/page"), &config).is_ok()
        );
    }

    #[test]
    fn rejects_unlisted_or_missing_sources() {
        let config = config_with_image_proxy(ImageProxyConfig::default());

        assert!(validate_image_proxy_source(Some("https://example.com"), &config).is_err());
        assert!(validate_image_proxy_source(None, &config).is_err());
        assert!(validate_image_proxy_source(Some(""), &config).is_err());
    }

    #[test]
    fn allows_configured_localhost_with_any_port() {
        let config = config_with_image_proxy(ImageProxyConfig {
            allowed_referrer_domains: vec!["Localhost".to_string()],
            ..ImageProxyConfig::default()
        });

        assert!(validate_image_proxy_source(Some("http://localhost:3000/page"), &config).is_ok());
        assert!(validate_image_proxy_source(Some("https://LOCALHOST:5173"), &config).is_ok());
    }

    #[test]
    fn allows_empty_referrer_when_missing_referrer_is_enabled() {
        let config = config_with_image_proxy(ImageProxyConfig {
            allow_missing_referrer: true,
            ..ImageProxyConfig::default()
        });

        assert!(validate_image_proxy_source(None, &config).is_ok());
        assert!(validate_image_proxy_source(Some(""), &config).is_ok());
        assert!(validate_image_proxy_source(Some("   "), &config).is_ok());
    }

}
