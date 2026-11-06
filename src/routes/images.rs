use crate::config::settings::Config;
use crate::services::image_proxy_service::{ImageProxyFetch, ImageProxyService};
use crate::services::image_service::ImageService;
use crate::utils::custom_response::CustomResponse;
use crate::{Error, Result};
use image::ImageFormat;
use log::error;
use once_cell::sync::Lazy;
use rocket::http::{Accept, ContentType, Status};
use rocket::request::{FromRequest, Outcome, Request};
use rocket::response::{self, Responder, Response};
use rocket::{get, routes, Route, State}; // 导入 State
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::convert::Infallible;

#[derive(Debug, Deserialize, Default)]
struct BlurhashData {
    weight: HashMap<String, String>,
    height: HashMap<String, String>,
}

const BLURHASH_RAW: &str = include_str!("../../src/data/blurhash.json");

static BLURHASH: Lazy<BlurhashData> = Lazy::new(|| {
    serde_json::from_str(BLURHASH_RAW).unwrap_or_else(|e| {
        error!("Failed to parse embedded blurhash.json: {}", e);
        BlurhashData::default()
    })
});

/// 壁纸索引：启动时把 blurhash.json 的键名收成有序表，随机选取直接基于键名。
type WallpaperEntries = Vec<(String, String)>;

fn index_entries(map: &HashMap<String, String>) -> WallpaperEntries {
    map.iter()
        .map(|(filename, hash)| (filename.clone(), hash.clone()))
        .collect()
}

static WEIGHT_ENTRIES: Lazy<WallpaperEntries> = Lazy::new(|| index_entries(&BLURHASH.weight));

static HEIGHT_ENTRIES: Lazy<WallpaperEntries> = Lazy::new(|| index_entries(&BLURHASH.height));

/// 直接从操作系统熵源取字节做无偏抽样，不依赖任何进程内 PRNG 状态。
///
/// 用 64 位拒绝采样消除取模偏差：只接受落在 `bound` 整数倍区间内的值。
fn random_index(len: usize) -> Result<usize> {
    if len == 0 {
        return Err(Error::Internal("壁纸索引为空".to_string()));
    }
    let bound = len as u64;
    let zone = u64::MAX - (u64::MAX % bound);
    loop {
        let mut bytes = [0u8; 8];
        getrandom::fill(&mut bytes)
            .map_err(|e| Error::Internal(format!("操作系统熵源不可用: {e}")))?;
        let value = u64::from_le_bytes(bytes);
        if value < zone {
            return Ok((value % bound) as usize);
        }
    }
}

struct ImageProxyRequestSource(Option<String>);

enum ImageProxyResponse {
    Complete(CustomResponse),
    Stream {
        reader: tokio::io::DuplexStream,
        content_type: ContentType,
    },
}

impl<'r> Responder<'r, 'static> for ImageProxyResponse {
    fn respond_to(self, request: &'r Request<'_>) -> response::Result<'static> {
        match self {
            Self::Complete(response) => response.respond_to(request),
            Self::Stream {
                reader,
                content_type,
            } => Response::build()
                .status(Status::Ok)
                .header(content_type)
                .raw_header("Cache-Control", "public, max-age=3600, s-maxage=3600")
                .raw_header("Vary", "Accept, Origin, Referer")
                .raw_header("X-Cache-Message", "Cold cache streaming")
                .raw_header("server-cache", "MISS")
                .streamed_body(reader)
                .ok(),
        }
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for ImageProxyRequestSource {
    type Error = Infallible;

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let source = request
            .headers()
            .get_one("Origin")
            .or_else(|| request.headers().get_one("Referer"))
            .map(str::to_owned);
        Outcome::Success(Self(source))
    }
}

/// 校验调用方来源，域名白名单会自动匹配任意层级子域名。
fn validate_image_proxy_source(source: Option<&str>, config: &Config) -> Result<()> {
    // 部分代理会保留 Referer 头但传入空值，因此应与完全缺少该请求头使用同一配置策略。
    let source = source.map(str::trim).filter(|source| !source.is_empty());
    let Some(source) = source else {
        return if config.image_proxy.allow_missing_referrer {
            Ok(())
        } else {
            Err(Error::Forbidden("图像代理请求缺少来源信息".to_string()))
        };
    };

    let source_url = url::Url::parse(source)
        .map_err(|_| Error::Forbidden("图像代理请求来源无效".to_string()))?;
    let source_host = source_url
        .host_str()
        .ok_or_else(|| Error::Forbidden("图像代理请求来源无效".to_string()))?
        .trim_end_matches('.')
        .to_ascii_lowercase();

    let is_allowed = config
        .image_proxy
        .allowed_referrer_domains
        .iter()
        .any(|domain| {
            let allowed_domain = domain
                .trim()
                .trim_start_matches("*.")
                .trim_end_matches('.')
                .to_ascii_lowercase();
            !allowed_domain.is_empty()
                && (source_host == allowed_domain
                    || source_host.ends_with(&format!(".{allowed_domain}")))
        });

    if is_allowed {
        Ok(())
    } else {
        Err(Error::Forbidden("当前来源不允许使用图像代理".to_string()))
    }
}

/// 随机取一条 `(文件名, blurhash)`，二者来自同一记录，天然对应。
fn pick_entry(entries: &WallpaperEntries) -> Result<&(String, String)> {
    entries
        .get(random_index(entries.len())?)
        .ok_or_else(|| Error::Internal("壁纸索引越界".to_string()))
}

async fn serve_wallpaper(
    t: Option<String>,
    r#type: Option<String>,
    accept: &Accept,
    service: &State<ImageService>,
    entries: &WallpaperEntries,
    url_prefix: &str,
) -> Result<CustomResponse> {
    let req_type = r#type.or(t);

    // 随机的是索引位置，文件名与 blurhash 取自同一条键记录，二者必然对应。
    let (filename, blurhash) = pick_entry(entries)?;
    let cdn_url = format!("{}/{}", url_prefix, filename);

    match req_type.as_deref() {
        Some("cdn") => {
            // 302 跳转
            let resp = CustomResponse::new(ContentType::Plain, Vec::new(), Status::Found)
                .with_header("Location", cdn_url)
                .with_header("Cache-Control", "no-cache");
            Ok(resp)
        }
        Some("json") => {
            // JSON 返回
            let payload = json!({
                "code": "200",
                "status": "success",
                "data": {
                    "image": cdn_url,
                    "blurhash": blurhash,
                }
            });

            let body = serde_json::to_vec(&payload).unwrap_or_default();
            // JSON 不缓存
            let resp = CustomResponse::new(ContentType::JSON, body, Status::Ok)
                .with_header("Cache-Control", "no-cache");
            Ok(resp)
        }
        _ => {
            // 默认：代理图片，按格式缓存编码后的结果
            let accept_str = accept.to_string();

            match service.fetch_wallpaper(&cdn_url, &accept_str).await {
                Ok((encoded_data, format)) => {
                    let content_type = match format {
                        ImageFormat::Avif => ContentType::new("image", "avif"),
                        ImageFormat::WebP => ContentType::new("image", "webp"),
                        ImageFormat::Png => ContentType::PNG,
                        _ => ContentType::JPEG,
                    };

                    // 缓存 30s
                    let resp = CustomResponse::new(content_type, encoded_data, Status::Ok)
                        .with_header("Cache-Control", "public, max-age=30");
                    Ok(resp)
                }
                Err(e) => {
                    error!("Error fetching wallpaper [{}]: {}", cdn_url, e);
                    let payload = json!({
                        "code": "500",
                        "message": "Error fetching wallpaper source",
                        "status": "failed"
                    });
                    let body = serde_json::to_vec(&payload).unwrap_or_default();
                    let resp =
                        CustomResponse::new(ContentType::JSON, body, Status::InternalServerError);
                    Ok(resp)
                }
            }
        }
    }
}

#[get("/wallpaper?<t>&<type>")]
async fn wallpaper(
    t: Option<String>,
    r#type: Option<String>,
    accept: &Accept,
    service: &State<ImageService>,
) -> Result<CustomResponse> {
    serve_wallpaper(
        t,
        r#type,
        accept,
        service,
        &WEIGHT_ENTRIES,
        "https://cdn.tnxg.top/images/wallpaper",
    )
    .await
}

#[get("/wallpaper_height?<t>&<type>")]
async fn wallpaper_height(
    t: Option<String>,
    r#type: Option<String>,
    accept: &Accept,
    service: &State<ImageService>,
) -> Result<CustomResponse> {
    serve_wallpaper(
        t,
        r#type,
        accept,
        service,
        &HEIGHT_ENTRIES,
        "https://cdn.tnxg.top/images/wallpaper_height",
    )
    .await
}

/// 通用图像代理，提供格式协商、SWR 缓存和来源防盗链。
#[get("/proxy?<url>&<force>")]
async fn image_proxy(
    url: &str,
    force: Option<&str>,
    accept: &Accept,
    request_source: ImageProxyRequestSource,
    config: &State<Config>,
    service: &State<ImageProxyService>,
) -> Result<ImageProxyResponse> {
    validate_image_proxy_source(request_source.0.as_deref(), config.inner())?;

    let force_refresh = force.is_some_and(|value| value == "true");
    let fetch = service
        .fetch_image(url, &accept.to_string(), force_refresh)
        .await?;

    match fetch {
        ImageProxyFetch::Stream { reader, format } => Ok(ImageProxyResponse::Stream {
            reader,
            content_type: image_proxy_content_type(&format),
        }),
        ImageProxyFetch::Complete {
            data,
            format,
            cache_status,
        } => {
            let cache_control = match cache_status.as_str() {
                "hit" => "public, max-age=7200, s-maxage=7200",
                "stale" => "public, max-age=300, s-maxage=300",
                "fallback" => "public, max-age=600, s-maxage=600",
                _ => "public, max-age=3600, s-maxage=3600",
            };
            let cache_message = match cache_status.as_str() {
                "hit" => "Fresh cache hit",
                "stale" => "Stale cache, updating in background",
                "fallback" => "Fallback mode, source unavailable",
                _ => "Cache miss",
            };
            let response = CustomResponse::new(image_proxy_content_type(&format), data, Status::Ok)
                .with_header("Cache-Control", cache_control)
                .with_header("Vary", "Accept, Origin, Referer")
                .with_header("X-Cache-Message", cache_message)
                .with_cache(cache_status == "hit");
            Ok(ImageProxyResponse::Complete(response))
        }
    }
}

fn image_proxy_content_type(format: &str) -> ContentType {
    match format {
        "avif" => ContentType::new("image", "avif"),
        "webp" => ContentType::new("image", "webp"),
        "png" => ContentType::PNG,
        "gif" => ContentType::GIF,
        _ => ContentType::JPEG,
    }
}

pub fn routes() -> Vec<Route> {
    routes![wallpaper, wallpaper_height, image_proxy]
}

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

    #[test]
    fn random_index_stays_within_bounds() {
        for len in [1usize, 2, 3, 7, 348] {
            for _ in 0..500 {
                assert!(super::random_index(len).expect("os entropy available") < len);
            }
        }
        assert!(super::random_index(0).is_err());
    }

    #[test]
    fn random_index_covers_every_position() {
        const LEN: usize = 7;
        const DRAWS: usize = 10_000;
        let mut seen = [0usize; LEN];
        for _ in 0..DRAWS {
            seen[super::random_index(LEN).expect("os entropy available")] += 1;
        }
        let expected = DRAWS / LEN;
        assert!(
            seen.iter().all(|&c| c > expected - 400 && c < expected + 400),
            "skewed distribution: {seen:?}"
        );
    }

    #[test]
    fn pick_entry_returns_some_record_verbatim() {
        let entries = vec![
            ("a.jpg".to_string(), "hash-a".to_string()),
            ("b23_p0.png".to_string(), "hash-b".to_string()),
            ("c.webp".to_string(), "hash-c".to_string()),
        ];
        let mut seen = std::collections::HashSet::new();
        for _ in 0..2_000 {
            let (filename, hash) = super::pick_entry(&entries).expect("entries are non-empty");
            assert!(entries.contains(&(filename.clone(), hash.clone())));
            seen.insert(filename.clone());
        }
        assert_eq!(seen.len(), entries.len());
        assert!(super::pick_entry(&Vec::new()).is_err());
    }

    #[test]
    fn wallpaper_index_keeps_key_and_blurhash_in_sync() {
        let data = &*super::BLURHASH;
        for (entries, map) in [
            (&*super::WEIGHT_ENTRIES, &data.weight),
            (&*super::HEIGHT_ENTRIES, &data.height),
        ] {
            assert_eq!(entries.len(), map.len());
            for (filename, hash) in entries.iter() {
                assert_eq!(map.get(filename), Some(hash));
                assert!(!filename.is_empty());
            }
        }
    }
}
