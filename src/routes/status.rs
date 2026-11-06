use rocket::serde::json::Json;

use rocket::{get, routes, Route};
use crate::utils::response::ApiResponse;
use crate::{Error, Result};
use serde_json::Value;
use std::env;

mod ncm;

// 获取代码时间统计（从 codetime.dev 代理返回原始 JSON）
#[get("/codetime")]
async fn codetime() -> Result<Json<ApiResponse<Value>>> {
    let session = env::var("CODETIME_SESSION").unwrap_or_default();
    if session.is_empty() {
        return Err(Error::Internal(
            "Missing environment variable CODETIME_SESSION".to_string(),
        ));
    }

    let client = reqwest::Client::new();
    let resp = client
        .get("https://api.codetime.dev/stats/latest")
        .header(
            reqwest::header::COOKIE,
            format!("CODETIME_SESSION={}", session),
        )
        .send()
        .await
        .map_err(|e| Error::Internal(format!("codetime request failed: {}", e)))?;

    if !resp.status().is_success() {
        return Err(Error::Internal(format!(
            "codetime status error: {}",
            resp.status()
        )));
    }

    let json: Value = resp
        .json()
        .await
        .map_err(|e| Error::Internal(format!("parse codetime json failed: {}", e)))?;

    if json.get("error").and_then(|v| if v.is_null() { None } else { Some(v) }).is_some() {
        return Ok(ApiResponse::error("500", "codetime service error"));
    }

    Ok(ApiResponse::success(json, "codetime"))
}


pub fn routes() -> Vec<Route> {
    routes![codetime, ncm::ncm]
}
