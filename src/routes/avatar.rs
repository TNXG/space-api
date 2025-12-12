use crate::services::image_service::ImageService;
use crate::utils::cache::{self, CACHE_BUCKET};
use crate::utils::custom_response::CustomResponse;
use crate::{Error, Result};
use image::ImageFormat;
use rocket::http::{Accept, ContentType, Status};
use rocket::{get, routes, Route};

// 简单的 Accept 协商：按优先级 avif > webp > png > jpeg
fn negotiate_format(accept: &str) -> (&'static str, ImageFormat, ContentType) {
    let a = accept.to_ascii_lowercase();
    if a.contains("image/avif") {
        ("avif", ImageFormat::Avif, ContentType::new("image", "avif"))
    } else if a.contains("image/webp") {
        ("webp", ImageFormat::WebP, ContentType::new("image", "webp"))
    } else if a.contains("image/png") {
        ("png", ImageFormat::Png, ContentType::PNG)
    } else {
        ("jpeg", ImageFormat::Jpeg, ContentType::JPEG)
    }
}

// 根据来源选择默认头像 URL
fn pick_source(source: &str) -> &str {
    match source.to_ascii_lowercase().as_str() {
        "qq" => "https://q1.qlogo.cn/g?b=qq&nk=2271225249&s=640",
        "github" | "gh" => "https://avatars.githubusercontent.com/u/69001561",
        _ => "https://cdn.tnxg.top/images/avatar/main/Texas.png",
    }
}

#[get("/?<s>&<source>")]
async fn get_avatar(
    s: Option<&str>,
    source: Option<&str>,
    accept: &Accept,
) -> Result<CustomResponse> {
    let src = s.or(source).unwrap_or("default");
    let accept_str = accept.to_string();

    if src.is_empty() {
        return Err(Error::BadRequest(
            "Missing required parameter: s or source".into(),
        ));
    }

    // Accept 头（如果通过查询参数未提供，则不用于协商）
    let (fmt_key, img_format, content_type) = negotiate_format(&accept_str);

    let origin_url = pick_source(src);
    let cache_key = format!("avatar:{}:{}", src, fmt_key);

    // 尝试缓存
    if let Some(cached) = cache::get(&CACHE_BUCKET, &cache_key).await {
        return Ok(CustomResponse::new(content_type, cached, Status::Ok)
            .with_header("Cache-Control", "public, max-age=259200, s-maxage=172800")
            .with_cache(true));
    }

    // 下载原始头像图像（使用专门的头像缓存策略）
    let image_service = ImageService::new();
    let (raw_bytes, origin_cache_hit) = image_service.fetch_avatar(origin_url).await?;
    let img = image::load_from_memory(&raw_bytes)
        .map_err(|e| Error::Internal(format!("Failed to decode avatar: {}", e)))?;

    let mut out: Vec<u8> = Vec::new();
    match img_format {
        ImageFormat::Avif | ImageFormat::WebP | ImageFormat::Jpeg => {
            img.write_to(&mut std::io::Cursor::new(&mut out), img_format)
                .map_err(|e| {
                    Error::Internal(format!("Failed to encode {:?}: {}", img_format, e))
                })?;
        }
        _ => return Err(Error::Internal("Unsupported target image format".into())),
    }

    // 写入缓存
    cache::put(&CACHE_BUCKET, cache_key.clone(), out.clone()).await;

    Ok(
        CustomResponse::new(content_type, out, Status::Ok)
            .with_header("Cache-Control", "public, max-age=259200, s-maxage=172800")
            .with_cache(origin_cache_hit), // 这里表示底层原始抓取是否命中
    )
}

pub fn routes() -> Vec<Route> {
    routes![get_avatar]
}
