use rocket::{Route, get, routes};
use rocket::http::{ContentType, Status};
use crate::utils::custom_response::CustomResponse;
use crate::utils::cache::CACHE_BUCKET;

#[get("/sw.js")]
async fn sw_js() -> CustomResponse {
    // 缓存键
    let cache_key = "sw_js".to_string();

    // 先尝试从全局缓存读取
    if let Some(cached) = crate::utils::cache::get(&CACHE_BUCKET, &cache_key).await {
        return CustomResponse::new(ContentType::JavaScript, cached, Status::Ok).with_cache(true);
    }

    // 远程 URL
    let url = "https://mx.tnxg.top/api/v2/snippets/js/sw";

    let client = reqwest::Client::new();
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::USER_AGENT,
        reqwest::header::HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/114.0.0.0 Safari/537.36 Edg/114.0.1823.82"),
    );
    headers.insert(
        reqwest::header::CONTENT_TYPE,
        reqwest::header::HeaderValue::from_static("application/javascript; charset=utf-8"),
    );

    match client.get(url).headers(headers).send().await {
        Ok(resp) => {
            let status = resp.status();
            match resp.text().await {
                Ok(text) => {
                    if status.is_success() {
                        let bytes = text.into_bytes();
                        // 写入缓存，忽略返回值
                        let _ = crate::utils::cache::put(&CACHE_BUCKET, cache_key.clone(), bytes.clone()).await;
                        CustomResponse::new(ContentType::JavaScript, bytes, Status::Ok).with_cache(false)
                    } else {
                        let msg = format!("// Failed to load service worker script: HTTP status {}", status.as_u16());
                        CustomResponse::new(ContentType::JavaScript, msg.into_bytes(), Status::InternalServerError)
                    }
                }
                Err(e) => {
                    let msg = format!("// Failed to load service worker script: {}", e);
                    CustomResponse::new(ContentType::JavaScript, msg.into_bytes(), Status::InternalServerError)
                }
            }
        }
        Err(e) => {
            let msg = format!("// Failed to load service worker script: {}", e);
            CustomResponse::new(ContentType::JavaScript, msg.into_bytes(), Status::InternalServerError)
        }
    }
}

pub fn routes() -> Vec<Route> {
    routes![sw_js]
}