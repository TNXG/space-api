use crate::services::image_service::ImageService;
use crate::utils::custom_response::CustomResponse;
use crate::Result;
use image::ImageFormat;
use log::error;
use once_cell::sync::Lazy;
use rocket::http::{Accept, ContentType, Status};
use rocket::{get, routes, Route, State}; // 导入 State
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;

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

static MAX_WEIGHT_NUM: Lazy<u32> = Lazy::new(|| get_max_id(&BLURHASH.weight));

static MAX_HEIGHT_NUM: Lazy<u32> = Lazy::new(|| get_max_id(&BLURHASH.height));

fn get_max_id(map: &HashMap<String, String>) -> u32 {
    map.keys()
        .filter_map(|k| k.split('.').next().and_then(|n| n.parse::<u32>().ok()))
        .max()
        .unwrap_or(1)
}

async fn serve_wallpaper(
    t: Option<String>,
    r#type: Option<String>,
    accept: &Accept,
    service: &State<ImageService>,
    map: &HashMap<String, String>,
    max_num: u32,
    url_prefix: &str,
) -> Result<CustomResponse> {
    let req_type = r#type.or(t);

    let image_id = rand::random_range(1..=max_num);
    let image_id_str = image_id.to_string();
    let filename = format!("{}.jpg", image_id_str);

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
            let blurhash = map.get(&filename).cloned().unwrap_or_default();

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
                    let body = serde_json::to_vec(&payload).unwrap();
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
        &BLURHASH.weight,
        *MAX_WEIGHT_NUM,
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
        &BLURHASH.height,                        // 使用 height 数据
        *MAX_HEIGHT_NUM,                         // 使用 height 最大值
        "https://cdn.tnxg.top/images/wallpaper", // 如果竖屏图在不同目录，请修改这里
    )
    .await
}

pub fn routes() -> Vec<Route> {
    routes![wallpaper, wallpaper_height]
}
