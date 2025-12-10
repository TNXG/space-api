use rocket::http::Status;
use rocket::response::stream::{Event, EventStream};
use rocket::serde::json::Json;
use rocket::tokio::{
    select,
    time::{interval as tokio_interval, Duration as TokioDuration},
};
use rocket::{get, routes, Either, Route};

use crate::services::ncm_service;
use crate::utils::cache::{self, CACHE_BUCKET};
use crate::utils::response::ApiResponse;
use crate::{Error, Result};
use serde_json::Value;
use std::env;

// 占位型结构已不需要，移除

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

    if json.get("error").is_some() && !json.get("error").unwrap().is_null() {
        return Ok(ApiResponse::error("500", "codetime service error"));
    }

    Ok(ApiResponse::success(json, "codetime"))
}

#[get("/ncm?<q>&<query>&<sse>&<interval>&<i>")]
async fn ncm(
    q: Option<u64>,
    query: Option<u64>,
    sse: Option<&str>,
    interval: Option<u64>,
    i: Option<u64>,
) -> Result<Either<EventStream![], (Status, Json<ApiResponse<Value>>)>> {
    let user_id = q.or(query).unwrap_or(515522946);
    let use_sse = matches!(sse, Some(v) if v.eq_ignore_ascii_case("true"));
    if use_sse {
        let ival = interval.or(i).unwrap_or(5000);
        if ival < 1000 {
            // 返回与 Nitro 匹配的 400 错误响应
            let resp = Json(ApiResponse::<Value> {
                code: "400".into(),
                status: "failed".into(),
                message: "Invalid interval: must be at least 1000ms".into(),
                data: None,
            });
            return Ok(Either::Right((Status::BadRequest, resp)));
        }

        let user_id_copy = user_id; // move into async block
        let stream = EventStream! {
                let mut data_tick = tokio_interval(TokioDuration::from_millis(ival));
                let mut heartbeat_tick = tokio_interval(TokioDuration::from_secs(30));
                let mut last_song_id: Option<i64> = None;
                let mut last_active: Option<bool> = None;

                loop {
                    select! {
                        _ = data_tick.tick() => {
                            // 拉取当前数据
                            let now_iso = chrono::Utc::now().to_rfc3339();
                            let raw = match ncm_service::get_ncm_now_play(user_id_copy).await {
                                Ok(v) => v,
                                Err(_) => {
                                    // 静默跳过本次，继续下一轮
                                    continue;
                                }
                            };

                            if let Some(v) = raw.get("data") {
                                // 提取 song id
                                let current_song_id = extract_song_id(v);

                                let is_inactive = match handle_cache(user_id_copy as i64, current_song_id, &now_iso).await {
                                    Ok(b) => b,
                                    Err(_) => false,
                                };

                                let active = !is_inactive;

                                // 仅在歌曲 ID 或活跃状态变化时推送
                                if last_song_id != Some(current_song_id) || last_active != Some(active) {
                                    let mut result = build_base_result(v, user_id_copy as i64, active, &now_iso);

                                    if active {
                                        if let Some(song) = v.get("song") {
                                            let song_obj = build_song_obj(song);
                                            if let Some(obj) = result.as_object_mut() {
                                                obj.insert("song".to_string(), song_obj);
                                            }
                                        }
                                    }

                                    last_song_id = Some(current_song_id);
                                    last_active = Some(active);

                                    yield Event::data(result.to_string());
                                }
                            }
                        }
                        _ = heartbeat_tick.tick() => {
                            yield Event::comment("heartbeat");
                        }
                    }
                }
        };

        return Ok(Either::Left(stream));
    }

    // 原 JSON 路径
    let now = chrono::Utc::now().to_rfc3339();
    let raw = ncm_service::get_ncm_now_play(user_id)
        .await
        .map_err(|e| Error::Internal(format!("ncm request failed: {}", e)))?;

    let data = match raw.get("data") {
        Some(v) if !v.is_null() => v,
        _ => {
            let resp = Json(ApiResponse::<Value> {
                code: "404".into(),
                status: "failed".into(),
                message: "User not found".into(),
                data: None,
            });
            return Ok(Either::Right((Status::NotFound, resp)));
        }
    };

    // 提取当前 songId 用于活跃度判断
    let current_song_id = extract_song_id(data);

    let is_inactive = handle_cache(user_id as i64, current_song_id, &now).await?;

    // 组装返回结构
    let mut result = build_base_result(data, user_id as i64, !is_inactive, &now);

    if !is_inactive {
        // song 细节
        if let Some(song) = data.get("song") {
            let song_obj = build_song_obj(song);
            if let Some(obj) = result.as_object_mut() {
                obj.insert("song".to_string(), song_obj);
            }
        }
    }

    Ok(Either::Right((
        Status::Ok,
        ApiResponse::success(result, "Netease Music Now Playing Status"),
    )))
}

// 处理简单缓存以判断活跃状态（5 分钟内同一首歌视为不活跃）
async fn handle_cache(user_id: i64, song_id: i64, now_iso: &str) -> Result<bool> {
    // 使用内置缓存（moka）替代数据库：键为 ncm_status:{user_id}，值为 JSON bytes
    let key = format!("ncm_status:{}", user_id);

    let mut is_inactive = false;

    if let Some(bytes) = cache::get(&*CACHE_BUCKET, &key).await {
        // 解析缓存内容
        if let Ok(text) = String::from_utf8(bytes.clone()) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                let last_song_id = json
                    .get("songId")
                    .and_then(|v| v.as_i64())
                    .unwrap_or_default();
                let last_ts = json
                    .get("timestamp")
                    .and_then(|v| v.as_str())
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&chrono::Utc));

                if let Some(last) = last_ts {
                    let diff = chrono::Utc::now() - last;
                    if diff.num_milliseconds() > 5 * 60 * 1000 && last_song_id == song_id {
                        is_inactive = true;
                    }
                }

                // 歌曲变更则更新缓存
                if last_song_id != song_id {
                    let new_json = serde_json::json!({
                        "userId": user_id,
                        "songId": song_id,
                        "timestamp": now_iso,
                    });
                    cache::put(&*CACHE_BUCKET, key, new_json.to_string().into_bytes()).await;
                }
            } else {
                // 解析失败则写入当前状态
                let new_json = serde_json::json!({
                    "userId": user_id,
                    "songId": song_id,
                    "timestamp": now_iso,
                });
                cache::put(&*CACHE_BUCKET, key, new_json.to_string().into_bytes()).await;
            }
        }
    } else {
        // 无缓存，写入当前状态
        let new_json = serde_json::json!({
            "userId": user_id,
            "songId": song_id,
            "timestamp": now_iso,
        });
        cache::put(&*CACHE_BUCKET, key, new_json.to_string().into_bytes()).await;
    }

    Ok(is_inactive)
}

// 提取当前播放的歌曲 ID
fn extract_song_id(data: &Value) -> i64 {
    data.get("song")
        .and_then(|s| s.get("id"))
        .and_then(|v| v.as_i64())
        .unwrap_or_default()
}

// 将毫秒时间戳转换为 RFC3339 字符串
fn ms_to_rfc3339(ms: i64) -> String {
    chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ms)
        .map(|d| d.to_rfc3339())
        .unwrap_or_default()
}

// 构建基础返回结构（不含 song）
fn build_base_result(
    data: &Value,
    user_id_fallback: i64,
    active: bool,
    last_update_iso: &str,
) -> Value {
    serde_json::json!({
        "id": data.get("id").and_then(|v| v.as_i64()).unwrap_or_default(),
        "user": {
            "id": data.get("userId").and_then(|v| v.as_i64()).unwrap_or(user_id_fallback),
            "avatar": data.get("avatar").and_then(|v| v.as_str()).unwrap_or_default(),
            "name": data.get("userName").and_then(|v| v.as_str()).unwrap_or_default(),
            "active": active,
        },
        "lastUpdate": last_update_iso,
    })
}

// 根据 TS 结构组装歌曲对象
fn build_song_obj(song: &Value) -> Value {
    let trans_names = song
        .get("transNames")
        .or_else(|| {
            song.get("extProperties")
                .and_then(|ep| ep.get("transNames"))
        })
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let alias = song
        .get("alias")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let artists = song
        .get("artists")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|a| {
                    serde_json::json!({
                        "id": a.get("id").and_then(|v| v.as_i64()).unwrap_or_default(),
                        "name": a.get("name").and_then(|v| v.as_str()).unwrap_or_default(),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let album = song.get("album").cloned().unwrap_or(Value::Null);
    let album_artists = album
        .get("artists")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|a| {
                    serde_json::json!({
                        "id": a.get("id").and_then(|v| v.as_i64()).unwrap_or_default(),
                        "name": a.get("name").and_then(|v| v.as_str()).unwrap_or_default(),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let publish_time_iso = album
        .get("publishTime")
        .and_then(|v| v.as_i64())
        .map(ms_to_rfc3339)
        .unwrap_or_default();

    serde_json::json!({
        "name": song.get("name").and_then(|v| v.as_str()).unwrap_or_default(),
        "transNames": trans_names,
        "alias": alias,
        "id": song.get("id").and_then(|v| v.as_i64()).unwrap_or_default(),
        "artists": artists,
        "album": {
            "name": album.get("name").and_then(|v| v.as_str()).unwrap_or_default(),
            "id": album.get("id").and_then(|v| v.as_i64()).unwrap_or_default(),
            "image": album.get("picUrl").and_then(|v| v.as_str()).unwrap_or_default(),
            "publishTime": publish_time_iso,
            "artists": album_artists,
        }
    })
}

pub fn routes() -> Vec<Route> {
    routes![codetime, ncm]
}
