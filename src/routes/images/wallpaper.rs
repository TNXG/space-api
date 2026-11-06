use crate::services::image_service::ImageService;
use crate::utils::custom_response::CustomResponse;
use crate::{Error, Result};
use log::error;
use std::sync::LazyLock;
use rocket::http::{ContentType, Status};
use rocket::request::Request;
use rocket::response::{self, Responder, Response};
use rocket::{get, State};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;

#[derive(Debug, Deserialize, Default)]
struct BlurhashData {
    weight: HashMap<String, String>,
    height: HashMap<String, String>,
}

const BLURHASH_RAW: &str = include_str!("../../data/blurhash.json");

static BLURHASH: LazyLock<BlurhashData> = LazyLock::new(|| {
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

static WEIGHT_ENTRIES: LazyLock<WallpaperEntries> = LazyLock::new(|| index_entries(&BLURHASH.weight));

static HEIGHT_ENTRIES: LazyLock<WallpaperEntries> = LazyLock::new(|| index_entries(&BLURHASH.height));

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

/// 随机取一条 `(文件名, blurhash)`，二者来自同一记录，天然对应。
fn pick_entry(entries: &WallpaperEntries) -> Result<&(String, String)> {
    entries
        .get(random_index(entries.len())?)
        .ok_or_else(|| Error::Internal("壁纸索引越界".to_string()))
}

#[derive(rocket::FromForm)]
pub(super) struct WallpaperQuery {
    mode: Option<String>,
    source: Option<String>,
    format: Option<String>,
    m: Option<String>,
    s: Option<String>,
    f: Option<String>,
    #[field(name = "type")]
    legacy_type: Option<String>,
    t: Option<String>,
}

pub(super) enum WallpaperResponse {
    Complete(CustomResponse),
    Stream {
        reader: tokio::io::DuplexStream,
        content_type: ContentType,
        length: Option<u64>,
    },
}

impl<'r> Responder<'r, 'static> for WallpaperResponse {
    fn respond_to(self, request: &'r Request<'_>) -> response::Result<'static> {
        match self {
            Self::Complete(response) => response.respond_to(request),
            Self::Stream { reader, content_type, length } => {
                let mut response = Response::build();
                response.header(content_type).raw_header("Cache-Control", "no-store");
                if let Some(length) = length {
                    response.raw_header("Content-Length", length.to_string());
                }
                response.streamed_body(reader).ok()
            }
        }
    }
}

async fn serve_wallpaper(
    query: WallpaperQuery,
    service: &State<ImageService>,
    entries: &WallpaperEntries,
    directory: &str,
) -> Result<WallpaperResponse> {
    let legacy = query.legacy_type.as_deref().or(query.t.as_deref());
    let explicit_mode = query.mode.as_deref().or(query.m.as_deref());
    let legacy_json = explicit_mode.is_none() && legacy == Some("json");
    let mode = match explicit_mode {
        Some("proxy" | "p") => "proxy",
        Some("redirect" | "r") => "redirect",
        Some("json" | "j") => "json",
        Some(_) => return Err(Error::BadRequest("mode/m 仅支持 proxy/p、redirect/r、json/j".into())),
        None => match legacy {
            Some("cdn") => "redirect",
            Some("json") => "json",
            None => "proxy",
            _ => return Err(Error::BadRequest("type 仅支持 cdn/json".into())),
        },
    };
    let source = match query.source.as_deref().or(query.s.as_deref()).unwrap_or("cdn") {
        "cdn" | "c" => "cdn",
        "cdcn" | "cn" => "cdcn",
        _ => return Err(Error::BadRequest("source/s 仅支持 cdn/c、cdcn/cn".into())),
    };
    let format = match query.format.as_deref().or(query.f.as_deref()).unwrap_or("original") {
        "original" | "o" => "original",
        "jxl" | "j" => "jxl",
        _ => return Err(Error::BadRequest("format/f 仅支持 original/o、jxl/j".into())),
    };

    let (filename, blurhash) = pick_entry(entries)?;
    let stem = filename.rsplit_once('.').map_or(filename.as_str(), |(stem, _)| stem);
    if mode == "json" {
        let original_cdn = format!("https://cdn.tnxg.top/images/{directory}/{filename}");
        let original_cdcn = format!("https://cdcn.tnxg.top/images/{directory}/{filename}");
        let jxl_cdn = format!("https://cdn.tnxg.top/images/{directory}/{stem}.jxl");
        let jxl_cdcn = format!("https://cdcn.tnxg.top/images/{directory}/{stem}.jxl");
        let mut data = json!({
            "filename": stem,
            "blurhash": blurhash,
            "original": { "cdn": original_cdn, "cdcn": original_cdcn },
            "jxl": { "cdn": jxl_cdn, "cdcn": jxl_cdcn },
        });
        // 旧 type=json 的 image 字段保留，始终指向 CDN 原图。
        if legacy_json {
            data["image"] = json!(original_cdn);
        }
        let body = serde_json::to_vec(&json!({
            "code": "200", "status": "success", "data": data,
        })).map_err(|e| Error::Internal(e.to_string()))?;
        return Ok(WallpaperResponse::Complete(
            CustomResponse::new(ContentType::JSON, body, Status::Ok)
                .with_header("Cache-Control", "no-store"),
        ));
    }
    let url = if format == "jxl" {
        format!("https://{source}.tnxg.top/images/{directory}/{stem}.jxl")
    } else {
        format!("https://{source}.tnxg.top/images/{directory}/{filename}")
    };
    if mode == "redirect" {
        return Ok(WallpaperResponse::Complete(
            CustomResponse::new(ContentType::Plain, Vec::new(), Status::Found)
                .with_header("Location", url)
                .with_header("Cache-Control", "no-store"),
        ));
    }

    let mut upstream = service.open_wallpaper(&url).await?;
    let content_type = if format == "jxl" {
        ContentType::new("image", "jxl")
    } else {
        filename.rsplit_once('.')
            .and_then(|(_, ext)| ContentType::from_extension(ext))
            .unwrap_or(ContentType::Binary)
    };
    let length = upstream.content_length();
    // 有界缓冲提供背压，客户端断开时停止上游读取。
    let (mut writer, reader) = tokio::io::duplex(64 * 1024);
    tokio::spawn(async move {
        use tokio::io::AsyncWriteExt;
        loop {
            match upstream.chunk().await {
                Ok(Some(chunk)) => {
                    if writer.write_all(&chunk).await.is_err() { break; }
                }
                Ok(None) => break,
                Err(error) => {
                    error!("Wallpaper upstream stream failed: {error}");
                    break;
                }
            }
        }
    });
    Ok(WallpaperResponse::Stream { reader, content_type, length })
}

#[get("/wallpaper?<query..>")]
pub(super) async fn wallpaper(
    query: WallpaperQuery,
    service: &State<ImageService>,
) -> Result<WallpaperResponse> {
    serve_wallpaper(query, service, &WEIGHT_ENTRIES, "wallpaper").await
}

#[get("/wallpaper_height?<query..>")]
pub(super) async fn wallpaper_height(
    query: WallpaperQuery,
    service: &State<ImageService>,
) -> Result<WallpaperResponse> {
    serve_wallpaper(query, service, &HEIGHT_ENTRIES, "wallpaper_height").await
}

#[cfg(test)]
include!("../../../tests/unit/wallpaper.rs");
