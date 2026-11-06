use super::ImageProxyService;
use crate::{Error, Result};
use image::ImageFormat;
use std::net::IpAddr;

impl ImageProxyService {
    pub(super) fn accepts_format(accept_header: &str, format: ImageFormat) -> bool {
        let media_type = match format {
            ImageFormat::Avif => "image/avif",
            ImageFormat::WebP => "image/webp",
            ImageFormat::Png => "image/png",
            ImageFormat::Jpeg => "image/jpeg",
            ImageFormat::Gif => "image/gif",
            _ => return false,
        };

        accept_header.split(',').any(|entry| {
            let mut parts = entry.split(';');
            let accepted_type = parts.next().unwrap_or_default().trim().to_ascii_lowercase();
            let is_rejected = parts.any(|parameter| {
                parameter
                    .trim()
                    .strip_prefix("q=")
                    .is_some_and(|quality| quality.trim() == "0")
            });
            !is_rejected
                && (accepted_type == media_type
                    || accepted_type == "image/*"
                    || accepted_type == "*/*")
        })
    }

    /// 根据 Accept 头确定最佳格式
    pub(super) fn get_preferred_format(&self, accept_header: &str) -> ImageFormat {
        if Self::explicitly_accepts(accept_header, "image/avif") {
            ImageFormat::Avif
        } else if Self::explicitly_accepts(accept_header, "image/webp") {
            ImageFormat::WebP
        } else if Self::accepts_format(accept_header, ImageFormat::Jpeg) {
            ImageFormat::Jpeg
        } else if Self::accepts_format(accept_header, ImageFormat::Png) {
            ImageFormat::Png
        } else if Self::accepts_format(accept_header, ImageFormat::Gif) {
            ImageFormat::Gif
        } else {
            ImageFormat::Jpeg
        }
    }

    fn explicitly_accepts(accept_header: &str, media_type: &str) -> bool {
        accept_header.split(',').any(|entry| {
            let mut parts = entry.split(';');
            let accepted_type = parts.next().unwrap_or_default().trim();
            let is_rejected = parts.any(|parameter| {
                parameter
                    .trim()
                    .strip_prefix("q=")
                    .is_some_and(|quality| quality.trim() == "0")
            });
            !is_rejected && accepted_type.eq_ignore_ascii_case(media_type)
        })
    }

    /// SSRF 防护：校验 URL 是否安全
    pub(super) fn validate_url(url: &str) -> Result<()> {
        let parsed =
            url::Url::parse(url).map_err(|_| Error::BadRequest(format!("Invalid URL: {}", url)))?;

        // 仅允许 http/https 协议
        match parsed.scheme() {
            "http" | "https" => {}
            scheme => {
                return Err(Error::BadRequest(format!(
                    "Unsupported URL scheme: {}",
                    scheme
                )));
            }
        }

        let host = parsed
            .host_str()
            .ok_or_else(|| Error::BadRequest("URL missing host".to_string()))?;

        // 拒绝 localhost 和常见本地别名
        let lower_host = host.to_ascii_lowercase();
        if lower_host == "localhost"
            || lower_host == "127.0.0.1"
            || lower_host == "[::1]"
            || lower_host == "0.0.0.0"
            || lower_host.ends_with(".local")
            || lower_host.ends_with(".internal")
        {
            return Err(Error::BadRequest(
                "Access to local addresses is not allowed".to_string(),
            ));
        }

        // 字面量 IP 可在发起 DNS/HTTP 请求前直接拒绝。
        if let Ok(ip) = host.parse::<IpAddr>() {
            if Self::is_private_or_reserved_ip(ip) {
                return Err(Error::BadRequest(
                    "Access to private/reserved IP addresses is not allowed".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// 域名解析结果也必须全部为公网地址，避免通过内网 DNS 绕过字面量校验。
    pub(super) async fn validate_resolved_url(url: &url::Url) -> Result<()> {
        let host = url
            .host_str()
            .ok_or_else(|| Error::BadRequest("URL missing host".to_string()))?;
        let port = url
            .port_or_known_default()
            .ok_or_else(|| Error::BadRequest("URL missing port".to_string()))?;
        let addresses = tokio::net::lookup_host((host, port))
            .await
            .map_err(|_| Error::BadRequest("无法解析图片源域名".to_string()))?;

        for address in addresses {
            if Self::is_private_or_reserved_ip(address.ip()) {
                return Err(Error::BadRequest(
                    "Access to private/reserved IP addresses is not allowed".to_string(),
                ));
            }
        }
        Ok(())
    }

    fn is_private_or_reserved_ip(ip: IpAddr) -> bool {
        match ip {
            IpAddr::V4(address) => {
                let octets = address.octets();
                address.is_loopback()
                    || address.is_private()
                    || address.is_link_local()
                    || address.is_broadcast()
                    || address.is_unspecified()
                    || address.is_multicast()
                    || address.is_documentation()
                    || octets[0] == 100 && (octets[1] & 0xC0) == 64
                    || octets[0] >= 240
            }
            IpAddr::V6(address) => {
                address.is_loopback()
                    || address.is_unspecified()
                    || address.is_unique_local()
                    || address.is_unicast_link_local()
                    || address.is_multicast()
            }
        }
    }
}
