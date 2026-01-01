use crate::services::friend_avatar_service::FriendAvatarService;
use crate::utils::custom_response::CustomResponse;
use crate::Result;
use rocket::http::{Accept, ContentType, Status};
use rocket::{get, routes, Route, State};

/// 友链头像路由
/// 
/// 查询参数：
/// - url: 友链头像的原始 URL (必需)
/// - force: 强制刷新缓存 (可选，值为 "true" 时生效)
/// 
/// 示例：
/// - /friend-avatar?url=https://example.com/avatar.jpg
/// - /friend-avatar?url=https://example.com/avatar.jpg&force=true
#[get("/?<url>&<force>")]
async fn get_friend_avatar(
    url: &str,
    force: Option<&str>,
    accept: &Accept,
    service: &State<FriendAvatarService>,
) -> Result<CustomResponse> {
    let force_refresh = force.map(|f| f == "true").unwrap_or(false);
    let accept_str = accept.to_string();

    let (image_data, content_type, cache_status) = service
        .fetch_friend_avatar(url, &accept_str, force_refresh)
        .await?;

    let content_type = match content_type.as_str() {
        "avif" => ContentType::new("image", "avif"),
        "webp" => ContentType::new("image", "webp"),
        "png" => ContentType::PNG,
        _ => ContentType::JPEG,
    };

    // 根据缓存状态设置不同的 Cache-Control
    let cache_control = match cache_status.as_str() {
        "hit" => "public, max-age=7200, s-maxage=7200",     // 2小时（新鲜缓存）
        "stale" => "public, max-age=300, s-maxage=300",     // 5分钟（过期但正在更新）
        "fallback" => "public, max-age=600, s-maxage=600",  // 10分钟（链接失效降级）
        _ => "public, max-age=3600, s-maxage=3600",         // 默认1小时
    };

    let status_message = match cache_status.as_str() {
        "hit" => "Fresh cache hit",
        "stale" => "Stale cache, updating in background",
        "fallback" => "Fallback mode, source unavailable",
        _ => "Cache miss",
    };

    Ok(CustomResponse::new(content_type, image_data, Status::Ok)
        .with_header("Cache-Control", cache_control)
        .with_header("X-Cache-Status", cache_status)
        .with_header("X-Cache-Message", status_message))
}

pub fn routes() -> Vec<Route> {
    routes![get_friend_avatar]
}
