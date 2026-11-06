use crate::services::image_service::ImageService;
use crate::Result;
use image::ImageFormat;
use log::info;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::DuplexStream;
use tokio::sync::RwLock;

mod cache;
mod download;
mod request;

pub enum ImageProxyFetch {
    Complete {
        data: Vec<u8>,
        format: String,
        cache_status: String,
    },
    Stream {
        reader: DuplexStream,
        format: String,
    },
}

/// 通用图像代理缓存元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ImageProxyMetadata {
    /// 原始 URL
    url: String,
    /// 最后成功获取的时间戳（秒）
    last_success_time: u64,
    /// 最后检查的时间戳（秒）
    last_check_time: u64,
    /// 是否处于 legacy 模式（链接失效但保留旧缓存）
    legacy_mode: bool,
    /// 连续失败次数
    fail_count: u32,
    /// 图片格式
    format: String,
}

pub struct ImageProxyService {
    client: Client,
    cache_dir: PathBuf,
    max_image_bytes: usize,
    /// 正在更新的 URL 集合（防止并发重复请求）
    updating: Arc<RwLock<std::collections::HashSet<String>>>,
}

impl ImageProxyService {
    pub fn new(max_image_bytes: usize) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(10))
                // 每次跳转都需要重新执行 SSRF 校验，不能交给客户端自动跟随。
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .expect("Failed to create HTTP client for ImageProxyService"),
            cache_dir: PathBuf::from("cache/image_proxy"),
            max_image_bytes,
            updating: Arc::new(RwLock::new(std::collections::HashSet::new())),
        }
    }

    /// 获取并按客户端支持的格式返回代理图像
    ///
    /// 缓存策略（SWR - Stale While Revalidate）：
    /// 1. 有缓存 -> 立即返回，根据新鲜度决定是否后台更新
    /// 2. 无缓存且客户端支持源格式 -> 边下载边响应并填充缓存
    /// 3. 客户端不支持源格式 -> 完整下载、转码、缓存后响应
    pub async fn fetch_image(
        &self,
        url: &str,
        accept_header: &str,
        force_refresh: bool,
    ) -> Result<ImageProxyFetch> {
        let target_format = self.get_preferred_format(accept_header);
        let target_format_ext = ImageService::format_extension(target_format);

        // 仅查找客户端明确支持的缓存格式，目标转码格式优先。
        let mut formats_to_try = vec![target_format_ext];
        for (format_ext, format) in [
            ("avif", ImageFormat::Avif),
            ("webp", ImageFormat::WebP),
            ("png", ImageFormat::Png),
            ("jpeg", ImageFormat::Jpeg),
            ("gif", ImageFormat::Gif),
        ] {
            if Self::accepts_format(accept_header, format) && !formats_to_try.contains(&format_ext)
            {
                formats_to_try.push(format_ext);
            }
        }

        info!(
            "[ImageProxy] 请求: {} (目标格式: {})",
            url, target_format_ext
        );

        // 强制刷新：直接下载
        if force_refresh {
            info!("[ImageProxy] 强制刷新: {}", url);
            let cache_key = self.get_cache_key(url, target_format_ext);
            return self
                .download_or_stream(url, accept_header, target_format, &cache_key)
                .await;
        }

        // 尝试读取缓存（按格式优先级）
        for format_ext in formats_to_try {
            let cache_key = self.get_cache_key(url, format_ext);
            info!(
                "[ImageProxy] 尝试读取缓存: format={}, cache_key={}",
                format_ext, cache_key
            );
            let cached_data = self.load_cache_data(&cache_key).await;
            let metadata = self.load_metadata(&cache_key).await;

            match (&cached_data, &metadata) {
                (Some(_), Some(_)) => {
                    info!("[ImageProxy] 找到缓存文件: {}", format_ext);
                }
                (Some(_), None) => {
                    info!("[ImageProxy] 找到数据但无元数据: {}", format_ext);
                }
                (None, Some(_)) => {
                    info!("[ImageProxy] 找到元数据但无数据: {}", format_ext);
                }
                (None, None) => {
                    info!("[ImageProxy] 无缓存: {}", format_ext);
                }
            }

            if let (Some(data), Some(meta)) = (cached_data, metadata) {
                let is_fresh = meta.is_fresh();
                let is_expired = meta.is_expired();

                let status = if meta.legacy_mode {
                    "fallback"
                } else if is_fresh {
                    "hit"
                } else {
                    "stale"
                };

                info!(
                    "[ImageProxy] 缓存状态 [{}]: fresh={}, expired={}, legacy={}",
                    format_ext, is_fresh, is_expired, meta.legacy_mode
                );

                // 任何非新鲜的缓存都触发后台更新（包括过期的）
                if !is_fresh {
                    info!("[ImageProxy] 缓存不新鲜，触发后台更新: {}", url);
                    let service = self.clone_for_background();
                    let url_clone = url.to_string();
                    let cache_key_clone = cache_key.clone();
                    let target_format_clone = target_format;
                    tokio::spawn(async move {
                        info!("[ImageProxy] 后台任务已启动: {}", url_clone);
                        let _ = service
                            .background_update(&url_clone, target_format_clone, &cache_key_clone)
                            .await;
                    });
                }

                // 立即返回缓存数据
                info!("[ImageProxy] 返回缓存 [{}]: {}", status, url);
                return Ok(ImageProxyFetch::Complete {
                    data,
                    format: format_ext.to_string(),
                    cache_status: status.to_string(),
                });
            }
        }

        // 无缓存：根据源格式与 Accept 决定流式透传或转码后响应
        info!("[ImageProxy] 无缓存，开始下载: {}", url);
        let cache_key = self.get_cache_key(url, target_format_ext);
        self.download_or_stream(url, accept_header, target_format, &cache_key)
            .await
    }

    /// 克隆用于后台任务（共享 updating 集合）
    fn clone_for_background(&self) -> Self {
        Self {
            client: self.client.clone(),
            cache_dir: self.cache_dir.clone(),
            max_image_bytes: self.max_image_bytes,
            updating: Arc::clone(&self.updating),
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit/image_proxy_service.rs"]
mod tests;
