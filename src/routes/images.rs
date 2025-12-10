use crate::services::image_service::ImageService;
use crate::utils::custom_response::CustomResponse;
use crate::utils::response::ApiResponse;
use crate::Result;
use image::ImageFormat;
use once_cell::sync::Lazy;
use rocket::http::{Accept, ContentType, Status};
use rocket::serde::json::Json;
use rocket::{get, routes, Route};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Serialize)]
pub struct WallpaperInfo {
    width: u32,
    height: u32,
    format: String,
    size_kb: f64,
}

#[derive(Debug, Deserialize, Default)]
struct BlurhashData {
    weight: HashMap<String, String>,
    #[allow(dead_code)]
    height: Option<HashMap<String, String>>,
}

fn blurhash_json_path() -> PathBuf {
    // 可执行时当前目录通常为 space-api-rs；向上一级定位到 Node 项目的 src/data/blurhash.json
    // 路径: space-api-rs/../src/data/blurhash.json
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("../src/data/blurhash.json");
    p
}

static BLURHASH: Lazy<BlurhashData> = Lazy::new(|| {
    let path = blurhash_json_path();
    match std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str::<BlurhashData>(&s).ok())
    {
        Some(data) => data,
        None => {
            eprintln!(
                "[images] Failed to load blurhash.json from {:?}. Fallback to empty map.",
                path
            );
            BlurhashData::default()
        }
    }
});

static MAX_WALLPAPER_NUM: Lazy<u32> = Lazy::new(|| {
    BLURHASH
        .weight
        .keys()
        .filter_map(|k| k.split('.').next().and_then(|n| n.parse::<u32>().ok()))
        .max()
        .unwrap_or(1)
});

// 获取壁纸信息
#[get("/wallpaper_height")]
async fn wallpaper_height() -> Json<ApiResponse<WallpaperInfo>> {
    // 模拟壁纸信息，实际实现中应该从配置或数据库获取
    let info = WallpaperInfo {
        width: 1920,
        height: 1080,
        format: "jpg".to_string(),
        size_kb: 1024.5,
    };

    ApiResponse::success(info, "Wallpaper info retrieved successfully")
}

// 获取壁纸图像（复刻 TS 逻辑：随机选择、type/t 参数、Accept 协商、JSON/302/图片返回）
#[get("/wallpaper?<t>")]
async fn wallpaper(t: Option<String>, accept: &Accept) -> Result<CustomResponse> {
    // 计算随机 imageId
    let max_num = *MAX_WALLPAPER_NUM;
    let image_id: u32 = rand::random_range(1..=max_num);
    let image_id_str = image_id.to_string();

    let cdn_url = format!(
        "https://cdn.tnxg.top/images/wallpaper/{}.jpg",
        image_id_str
    );

    // 统一读取 type / t 参数
    let req_type = t.as_deref();

    // 处理分支：cdn/json/默认图片
    match req_type {
        Some("cdn") => {
            // 302 跳转到 CDN
            let resp = CustomResponse::new(ContentType::Plain, Vec::new(), Status::Found)
                .with_header("Location", cdn_url);
            return Ok(resp);
        }
        Some("json") => {
            // 返回 JSON（带 blurhash 和缓存头）
            let key = format!("{}.jpg", image_id_str);
            let blurhash = BLURHASH.weight.get(&key).cloned().unwrap_or_default();
            let payload = json!({
                "code": "200",
                "status": "success",
                "data": {
                    "image": cdn_url,
                    "blurhash": blurhash,
                }
            });
            let body = payload.to_string().into_bytes();
            let resp = CustomResponse::new(ContentType::JSON, body, Status::Ok)
                .with_header("Cache-Control", "public, max-age=30");
            return Ok(resp);
        }
        _ => {}
    }

    // 默认：取图并按 Accept 协商格式返回（webp > png > jpeg）
    let image_service = ImageService::new();
    let accept_str = accept.to_string();

    // 拉取源 JPG
    match image_service.fetch_image(&cdn_url).await {
        Ok((image_data, cache_hit)) => {
            let format = image_service.get_preferred_image_format(&accept_str);
            let processed = image_service
                .process_image(image_data, None, None, format)
                .await?;

            let content_type = match format {
                ImageFormat::Jpeg => ContentType::JPEG,
                ImageFormat::Png => ContentType::PNG,
                ImageFormat::WebP => ContentType::new("image", "webp"),
                _ => ContentType::JPEG,
            };

            let resp = CustomResponse::new(content_type, processed, Status::Ok).with_cache(cache_hit);
            Ok(resp)
        }
        Err(e) => {
            eprintln!("Error fetching wallpaper: {}", e);
            let payload = json!({
                "code": "500",
                "message": "Error fetching wallpaper",
                "status": "failed"
            });
            let body = payload.to_string().into_bytes();
            let resp = CustomResponse::new(ContentType::JSON, body, Status::InternalServerError);
            Ok(resp)
        }
    }
}

pub fn routes() -> Vec<Route> {
    routes![wallpaper_height, wallpaper]
}
