use crate::services::image_service::ImageService;
use crate::{Error, Result};
use image::ImageFormat;
use log::{debug, error, info};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::fs;
use tokio::io::{AsyncWriteExt, DuplexStream};
use tokio::sync::RwLock;

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

/// 获取当前时间戳（秒），系统时钟异常时回退到 0
fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs()
}

impl ImageProxyMetadata {
    fn new(url: String, format: String) -> Self {
        let now = now_secs();

        Self {
            url,
            last_success_time: now,
            last_check_time: now,
            legacy_mode: false,
            fail_count: 0,
            format,
        }
    }

    /// 检查缓存是否新鲜（2小时内）
    fn is_fresh(&self) -> bool {
        let now = now_secs();
        now.saturating_sub(self.last_check_time) < 2 * 60 * 60 // 2小时
    }

    /// 检查缓存是否过期（30天）
    fn is_expired(&self) -> bool {
        let now = now_secs();
        now.saturating_sub(self.last_success_time) > 30 * 24 * 60 * 60 // 30天
    }

    /// 标记为成功
    fn mark_success(&mut self) {
        let now = now_secs();
        self.last_success_time = now;
        self.last_check_time = now;
        self.fail_count = 0;
        self.legacy_mode = false;
    }

    /// 标记为失败
    fn mark_failure(&mut self) {
        let now = now_secs();
        self.last_check_time = now;
        self.fail_count += 1;

        // 连续失败3次进入 legacy 模式
        if self.fail_count >= 3 {
            self.legacy_mode = true;
        }
    }
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

    /// 首次拉取时，客户端支持源格式则流式透传，否则完成转码后返回。
    async fn download_or_stream(
        &self,
        url: &str,
        accept_header: &str,
        target_format: ImageFormat,
        cache_key: &str,
    ) -> Result<ImageProxyFetch> {
        let (mut response, initial_bytes, source_format) = self.begin_download(url).await?;

        if Self::accepts_format(accept_header, source_format) {
            let source_format_ext = ImageService::format_extension(source_format);
            let source_cache_key = self.get_cache_key(url, source_format_ext);
            let service = self.clone_for_background();
            let source_url = url.to_string();
            let (reader, mut writer) = tokio::io::duplex(64 * 1024);
            let (stream_sender, mut stream_receiver) =
                tokio::sync::mpsc::unbounded_channel::<bytes::Bytes>();

            tokio::spawn(async move {
                while let Some(chunk) = stream_receiver.recv().await {
                    if writer.write_all(&chunk).await.is_err() {
                        break;
                    }
                }
                let _ = writer.shutdown().await;
            });

            tokio::spawn(async move {
                let initial_chunk = bytes::Bytes::from(initial_bytes);
                let mut image_data = initial_chunk.to_vec();
                let _ = stream_sender.send(initial_chunk);
                let mut download_error = None;

                loop {
                    match response.chunk().await {
                        Ok(Some(chunk)) => {
                            if image_data.len().saturating_add(chunk.len())
                                > service.max_image_bytes
                            {
                                download_error = Some("源图超过允许的大小限制".to_string());
                                break;
                            }
                            image_data.extend_from_slice(&chunk);
                            // 向用户发送和填充缓存相互独立，客户端断开不影响缓存下载。
                            let _ = stream_sender.send(chunk);
                        }
                        Ok(None) => break,
                        Err(error) => {
                            download_error = Some(format!("读取响应失败: {error}"));
                            break;
                        }
                    }
                }

                drop(stream_sender);
                if let Some(error) = download_error {
                    error!("[ImageProxy] 流式拉取失败: {} - {}", source_url, error);
                    return;
                }

                if let Err(error) = service
                    .save_cache(
                        &source_cache_key,
                        &image_data,
                        &source_url,
                        source_format_ext,
                    )
                    .await
                {
                    error!("[ImageProxy] 流式缓存写入失败: {} - {}", source_url, error);
                }
            });

            info!(
                "[ImageProxy] 冷缓存流式透传: {} ({})",
                url, source_format_ext
            );
            return Ok(ImageProxyFetch::Stream {
                reader,
                format: source_format_ext.to_string(),
            });
        }

        let raw_bytes = self.finish_download(response, initial_bytes).await?;
        self.transcode_and_cache(url, raw_bytes, target_format, cache_key)
            .await
    }

    async fn transcode_and_cache(
        &self,
        url: &str,
        raw_bytes: Vec<u8>,
        format: ImageFormat,
        cache_key: &str,
    ) -> Result<ImageProxyFetch> {
        info!("[ImageProxy] 下载完成: {} ({} 字节)", url, raw_bytes.len());

        // 智能转码（AVIF 等无法解码的格式会透传）
        let (final_bytes, final_format) =
            tokio::task::spawn_blocking(move || ImageService::smart_transcode(raw_bytes, format))
                .await
                .map_err(|e| Error::Internal(format!("Task join error: {}", e)))??;

        let format_ext = ImageService::format_extension(final_format);

        // 如果格式变了（如 AVIF 透传），需要用新的 cache_key
        let actual_cache_key = if final_format != format {
            info!(
                "[ImageProxy] 格式变更: {} -> {}",
                ImageService::format_extension(format),
                format_ext
            );
            self.get_cache_key(url, format_ext)
        } else {
            cache_key.to_string()
        };

        // 保存缓存
        self.save_cache(&actual_cache_key, &final_bytes, url, format_ext)
            .await?;

        info!(
            "[ImageProxy] 缓存已保存: {} ({} 字节, {})",
            url,
            final_bytes.len(),
            format_ext
        );
        Ok(ImageProxyFetch::Complete {
            data: final_bytes,
            format: format_ext.to_string(),
            cache_status: "miss".to_string(),
        })
    }

    /// 后台更新（SWR）
    async fn background_update(
        &self,
        url: &str,
        format: ImageFormat,
        cache_key: &str,
    ) -> Result<()> {
        // 防止并发重复更新
        {
            let mut updating = self.updating.write().await;
            if updating.contains(url) {
                debug!("[ImageProxy] 已在更新中，跳过: {}", url);
                return Ok(());
            }
            updating.insert(url.to_string());
        }

        info!("[ImageProxy] 后台更新开始: {}", url);

        // 执行更新
        let result = async {
            let raw_bytes = self.download_image(url).await?;
            info!(
                "[ImageProxy] 后台下载完成: {} ({} 字节)",
                url,
                raw_bytes.len()
            );

            // 智能转码
            let (final_bytes, final_format) = tokio::task::spawn_blocking(move || {
                ImageService::smart_transcode(raw_bytes, format)
            })
            .await
            .map_err(|e| Error::Internal(format!("Task join error: {}", e)))??;

            let final_format_ext = ImageService::format_extension(final_format);

            // 如果格式变了（如 AVIF 透传），需要用新的 cache_key
            if final_format != format {
                info!(
                    "[ImageProxy] 后台更新格式变更: {} -> {}",
                    ImageService::format_extension(format),
                    final_format_ext
                );
            }
            let actual_cache_key = self.get_cache_key(url, final_format_ext);

            self.save_cache(&actual_cache_key, &final_bytes, url, final_format_ext)
                .await?;
            info!(
                "[ImageProxy] 后台更新成功: {} ({} 字节, {})",
                url,
                final_bytes.len(),
                final_format_ext
            );
            Ok::<(), Error>(())
        }
        .await;

        // 处理失败情况
        if let Err(e) = result {
            error!("[ImageProxy] 后台更新失败: {} - {}", url, e);
            self.mark_update_failure(cache_key).await;
        }

        // 移除更新标记
        {
            let mut updating = self.updating.write().await;
            updating.remove(url);
        }

        Ok(())
    }

    /// 建立上游连接并读取足以识别图片格式的头部。
    async fn begin_download(&self, url: &str) -> Result<(reqwest::Response, Vec<u8>, ImageFormat)> {
        let mut current_url =
            url::Url::parse(url).map_err(|_| Error::BadRequest(format!("Invalid URL: {}", url)))?;
        let mut redirect_count = 0;
        let mut response = loop {
            Self::validate_url(current_url.as_str())?;
            Self::validate_resolved_url(&current_url).await?;
            debug!("[ImageProxy] 正在请求: {}", current_url);

            let response = self
                .client
                .get(current_url.clone())
                .header(
                    "User-Agent",
                    concat!("SpaceImageProxy/", env!("CARGO_PKG_VERSION")),
                )
                .send()
                .await
                .map_err(|e| Error::Internal(format!("请求失败: {}", e)))?;

            if !response.status().is_redirection() {
                break response;
            }
            if redirect_count >= 5 {
                return Err(Error::BadRequest("图片源重定向次数过多".to_string()));
            }

            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| Error::BadRequest("图片源返回了无效重定向".to_string()))?;
            current_url = current_url
                .join(location)
                .map_err(|_| Error::BadRequest("图片源返回了无效重定向".to_string()))?;
            redirect_count += 1;
        };

        let status = response.status();
        debug!("[ImageProxy] 响应状态: {}", status);

        if !status.is_success() {
            return Err(Error::NotFound(format!("图片未找到: HTTP {}", status)));
        }

        if response
            .content_length()
            .is_some_and(|length| length > self.max_image_bytes as u64)
        {
            return Err(Error::BadRequest("源图超过允许的大小限制".to_string()));
        }

        let mut initial_bytes = Vec::new();
        while initial_bytes.len() < 12 {
            let Some(chunk) = response
                .chunk()
                .await
                .map_err(|e| Error::Internal(format!("读取响应失败: {}", e)))?
            else {
                break;
            };
            if initial_bytes.len().saturating_add(chunk.len()) > self.max_image_bytes {
                return Err(Error::BadRequest("源图超过允许的大小限制".to_string()));
            }
            initial_bytes.extend_from_slice(&chunk);
        }

        let source_format = ImageService::detect_format(&initial_bytes)
            .ok_or_else(|| Error::BadRequest("目标 URL 未返回支持的图像格式".to_string()))?;

        Ok((response, initial_bytes, source_format))
    }

    /// 完成剩余下载，并持续执行实际字节数限制。
    async fn finish_download(
        &self,
        mut response: reqwest::Response,
        mut bytes: Vec<u8>,
    ) -> Result<Vec<u8>> {
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|e| Error::Internal(format!("读取响应失败: {}", e)))?
        {
            if bytes.len().saturating_add(chunk.len()) > self.max_image_bytes {
                return Err(Error::BadRequest("源图超过允许的大小限制".to_string()));
            }
            bytes.extend_from_slice(&chunk);
        }

        Ok(bytes)
    }

    /// 下载完整源图，供转码和后台更新使用。
    async fn download_image(&self, url: &str) -> Result<Vec<u8>> {
        let (response, initial_bytes, _) = self.begin_download(url).await?;
        self.finish_download(response, initial_bytes).await
    }

    fn accepts_format(accept_header: &str, format: ImageFormat) -> bool {
        let media_type = match format {
            ImageFormat::Avif => "image/avif",
            ImageFormat::WebP => "image/webp",
            ImageFormat::Png => "image/png",
            ImageFormat::Jpeg => "image/jpeg",
            ImageFormat::Gif => "image/gif",
            _ => return false,
        };

        accept_header.split(',').any(|entry| {
            let mut parts = entry.split(';');
            let accepted_type = parts.next().unwrap_or_default().trim().to_ascii_lowercase();
            let is_rejected = parts.any(|parameter| {
                parameter
                    .trim()
                    .strip_prefix("q=")
                    .is_some_and(|quality| quality.trim() == "0")
            });
            !is_rejected
                && (accepted_type == media_type
                    || accepted_type == "image/*"
                    || accepted_type == "*/*")
        })
    }

    /// 保存缓存（数据 + 元数据）
    async fn save_cache(
        &self,
        cache_key: &str,
        data: &[u8],
        url: &str,
        format: &str,
    ) -> Result<()> {
        // 确保缓存目录存在
        fs::create_dir_all(&self.cache_dir)
            .await
            .map_err(|e| Error::Internal(format!("Failed to create cache dir: {}", e)))?;

        // 保存图片数据
        let data_path = self.cache_dir.join(format!("{}.img", cache_key));
        Self::write_atomic(&data_path, data).await?;

        // 保存元数据
        let mut metadata = ImageProxyMetadata::new(url.to_string(), format.to_string());
        metadata.mark_success();
        self.save_metadata(cache_key, &metadata).await?;

        Ok(())
    }

    /// 加载缓存数据
    async fn load_cache_data(&self, cache_key: &str) -> Option<Vec<u8>> {
        let data_path = self.cache_dir.join(format!("{}.img", cache_key));
        fs::read(&data_path).await.ok()
    }

    /// 保存元数据
    async fn save_metadata(&self, cache_key: &str, metadata: &ImageProxyMetadata) -> Result<()> {
        let meta_path = self.cache_dir.join(format!("{}.meta", cache_key));
        let json = serde_json::to_string(metadata)
            .map_err(|e| Error::Internal(format!("Failed to serialize metadata: {}", e)))?;
        Self::write_atomic(&meta_path, json.as_bytes()).await?;
        Ok(())
    }

    /// 先写临时文件再原子替换，进程中断时仍保留上一份完整缓存。
    async fn write_atomic(path: &Path, data: &[u8]) -> Result<()> {
        let temporary_path = path.with_extension(format!(
            "tmp-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ));
        fs::write(&temporary_path, data)
            .await
            .map_err(|e| Error::Internal(format!("Failed to write cache file: {}", e)))?;

        if let Err(error) = fs::rename(&temporary_path, path).await {
            let _ = fs::remove_file(&temporary_path).await;
            return Err(Error::Internal(format!(
                "Failed to replace cache file: {}",
                error
            )));
        }
        Ok(())
    }

    /// 加载元数据
    async fn load_metadata(&self, cache_key: &str) -> Option<ImageProxyMetadata> {
        let meta_path = self.cache_dir.join(format!("{}.meta", cache_key));
        let json = fs::read_to_string(&meta_path).await.ok()?;
        serde_json::from_str(&json).ok()
    }

    /// 标记更新失败
    async fn mark_update_failure(&self, cache_key: &str) {
        if let Some(mut metadata) = self.load_metadata(cache_key).await {
            metadata.mark_failure();
            let _ = self.save_metadata(cache_key, &metadata).await;
        }
    }

    /// 获取缓存 key（URL hash + format）
    fn get_cache_key(&self, url: &str, format: &str) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(url.as_bytes());
        let hash = format!("{:x}", hasher.finalize());
        format!("{}_{}", &hash[..16], format)
    }

    /// 根据 Accept 头确定最佳格式
    fn get_preferred_format(&self, accept_header: &str) -> ImageFormat {
        if Self::explicitly_accepts(accept_header, "image/avif") {
            ImageFormat::Avif
        } else if Self::explicitly_accepts(accept_header, "image/webp") {
            ImageFormat::WebP
        } else if Self::accepts_format(accept_header, ImageFormat::Jpeg) {
            ImageFormat::Jpeg
        } else if Self::accepts_format(accept_header, ImageFormat::Png) {
            ImageFormat::Png
        } else if Self::accepts_format(accept_header, ImageFormat::Gif) {
            ImageFormat::Gif
        } else {
            ImageFormat::Jpeg
        }
    }

    fn explicitly_accepts(accept_header: &str, media_type: &str) -> bool {
        accept_header.split(',').any(|entry| {
            let mut parts = entry.split(';');
            let accepted_type = parts.next().unwrap_or_default().trim();
            let is_rejected = parts.any(|parameter| {
                parameter
                    .trim()
                    .strip_prefix("q=")
                    .is_some_and(|quality| quality.trim() == "0")
            });
            !is_rejected && accepted_type.eq_ignore_ascii_case(media_type)
        })
    }

    /// SSRF 防护：校验 URL 是否安全
    fn validate_url(url: &str) -> Result<()> {
        let parsed =
            url::Url::parse(url).map_err(|_| Error::BadRequest(format!("Invalid URL: {}", url)))?;

        // 仅允许 http/https 协议
        match parsed.scheme() {
            "http" | "https" => {}
            scheme => {
                return Err(Error::BadRequest(format!(
                    "Unsupported URL scheme: {}",
                    scheme
                )));
            }
        }

        let host = parsed
            .host_str()
            .ok_or_else(|| Error::BadRequest("URL missing host".to_string()))?;

        // 拒绝 localhost 和常见本地别名
        let lower_host = host.to_ascii_lowercase();
        if lower_host == "localhost"
            || lower_host == "127.0.0.1"
            || lower_host == "[::1]"
            || lower_host == "0.0.0.0"
            || lower_host.ends_with(".local")
            || lower_host.ends_with(".internal")
        {
            return Err(Error::BadRequest(
                "Access to local addresses is not allowed".to_string(),
            ));
        }

        // 字面量 IP 可在发起 DNS/HTTP 请求前直接拒绝。
        if let Ok(ip) = host.parse::<IpAddr>() {
            if Self::is_private_or_reserved_ip(ip) {
                return Err(Error::BadRequest(
                    "Access to private/reserved IP addresses is not allowed".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// 域名解析结果也必须全部为公网地址，避免通过内网 DNS 绕过字面量校验。
    async fn validate_resolved_url(url: &url::Url) -> Result<()> {
        let host = url
            .host_str()
            .ok_or_else(|| Error::BadRequest("URL missing host".to_string()))?;
        let port = url
            .port_or_known_default()
            .ok_or_else(|| Error::BadRequest("URL missing port".to_string()))?;
        let addresses = tokio::net::lookup_host((host, port))
            .await
            .map_err(|_| Error::BadRequest("无法解析图片源域名".to_string()))?;

        for address in addresses {
            if Self::is_private_or_reserved_ip(address.ip()) {
                return Err(Error::BadRequest(
                    "Access to private/reserved IP addresses is not allowed".to_string(),
                ));
            }
        }
        Ok(())
    }

    fn is_private_or_reserved_ip(ip: IpAddr) -> bool {
        match ip {
            IpAddr::V4(address) => {
                let octets = address.octets();
                address.is_loopback()
                    || address.is_private()
                    || address.is_link_local()
                    || address.is_broadcast()
                    || address.is_unspecified()
                    || address.is_multicast()
                    || address.is_documentation()
                    || octets[0] == 100 && (octets[1] & 0xC0) == 64
                    || octets[0] >= 240
            }
            IpAddr::V6(address) => {
                address.is_loopback()
                    || address.is_unspecified()
                    || address.is_unique_local()
                    || address.is_unicast_link_local()
                    || address.is_multicast()
            }
        }
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
mod tests {
    use super::ImageProxyService;
    use image::ImageFormat;

    #[test]
    fn streams_when_source_format_is_accepted() {
        assert!(ImageProxyService::accepts_format(
            "image/avif,image/webp,image/*;q=0.8",
            ImageFormat::Png,
        ));
        assert!(ImageProxyService::accepts_format(
            "image/jpeg",
            ImageFormat::Jpeg,
        ));
    }

    #[test]
    fn transcodes_when_source_format_is_not_accepted() {
        assert!(!ImageProxyService::accepts_format(
            "image/avif,image/webp",
            ImageFormat::Png,
        ));
        assert!(!ImageProxyService::accepts_format(
            "image/png;q=0",
            ImageFormat::Png,
        ));
    }

    #[test]
    fn chooses_only_an_accepted_conversion_format() {
        let service = ImageProxyService::new(1024);

        assert_eq!(
            service.get_preferred_format("image/avif,image/webp"),
            ImageFormat::Avif
        );
        assert_eq!(service.get_preferred_format("image/gif"), ImageFormat::Gif);
        assert_eq!(service.get_preferred_format("image/*"), ImageFormat::Jpeg);
    }
}
