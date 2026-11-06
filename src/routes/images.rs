use crate::config::settings::Config;
use crate::services::image_proxy_service::{ImageProxyFetch, ImageProxyService};
use crate::utils::custom_response::CustomResponse;
use crate::{Error, Result};
use rocket::http::{Accept, ContentType, Status};
use rocket::request::{FromRequest, Outcome, Request};
use rocket::response::{self, Responder, Response};
use rocket::{get, routes, Route, State}; // 导入 State
use std::convert::Infallible;

mod wallpaper;

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
    routes![wallpaper::wallpaper, wallpaper::wallpaper_height, image_proxy]
}

#[cfg(test)]
include!("../../tests/unit/images.rs");
