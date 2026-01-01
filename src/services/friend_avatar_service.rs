use crate::{Error, Result};
use image::ImageFormat;
use log::{debug, error, info};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::fs;
use tokio::sync::RwLock;

/// 友链头像缓存元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AvatarMetadata {
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

impl AvatarMetadata {
    fn new(url: String, format: String) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

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
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        now - self.last_check_time < 2 * 60 * 60 // 2小时
    }

    /// 检查缓存是否过期（30天）
    fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        now - self.last_success_time > 30 * 24 * 60 * 60 // 30天
    }

    /// 标记为成功
    fn mark_success(&mut self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.last_success_time = now;
        self.last_check_time = now;
        self.fail_count = 0;
        self.legacy_mode = false;
    }

    /// 标记为失败
    fn mark_failure(&mut self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.last_check_time = now;
        self.fail_count += 1;

        // 连续失败3次进入 legacy 模式
        if self.fail_count >= 3 {
            self.legacy_mode = true;
        }
    }
}

pub struct FriendAvatarService {
    client: Client,
    cache_dir: PathBuf,
    /// 正在更新的 URL 集合（防止并发重复请求）
    updating: RwLock<std::collections::HashSet<String>>,
}

impl FriendAvatarService {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap(),
            cache_dir: PathBuf::from("cache/friend_avatars"),
            updating: RwLock::new(std::collections::HashSet::new()),
        }
    }

    /// 获取友链头像
    /// 
    /// 缓存策略：
    /// 1. 检查缓存是否新鲜（2小时内）-> 直接返回
    /// 2. 缓存过期但存在 -> 返回旧缓存，后台异步更新（SWR）
    /// 3. 无缓存 -> 同步下载并缓存
    /// 4. 下载失败且有旧缓存 -> 进入 legacy 模式，保留旧缓存
    pub async fn fetch_friend_avatar(
        &self,
        url: &str,
        accept_header: &str,
        force_refresh: bool,
    ) -> Result<(Vec<u8>, String, String)> {
        let format = self.get_preferred_format(accept_header);
        let format_ext = Self::format_extension(format);
        let cache_key = self.get_cache_key(url, format_ext);

        // 1. 读取元数据
        let metadata = self.load_metadata(&cache_key).await;

        // 2. 强制刷新
        if force_refresh {
            return self.download_and_cache(url, format, &cache_key).await;
        }

        // 3. 检查缓存新鲜度
        if let Some(ref meta) = metadata {
            if meta.is_fresh() {
                // 缓存新鲜，直接返回
                if let Some(data) = self.load_cache_data(&cache_key).await {
                    debug!("FriendAvatar fresh cache hit: {}", url);
                    let status = if meta.legacy_mode { "fallback" } else { "hit" };
                    return Ok((data, format_ext.to_string(), status.to_string()));
                }
            }

            // 4. 缓存过期但存在 -> SWR 策略
            if !meta.is_expired() {
                if let Some(data) = self.load_cache_data(&cache_key).await {
                    info!("FriendAvatar stale cache hit, triggering background update: {}", url);
                    
                    // 后台异步更新（不阻塞当前请求）
                    let service = self.clone_for_background();
                    let url_clone = url.to_string();
                    let cache_key_clone = cache_key.clone();
                    tokio::spawn(async move {
                        let _ = service.background_update(&url_clone, format, &cache_key_clone).await;
                    });

                    let status = if meta.legacy_mode { "fallback" } else { "stale" };
                    return Ok((data, format_ext.to_string(), status.to_string()));
                }
            }
        }

        // 5. 无缓存或缓存完全过期 -> 同步下载
        self.download_and_cache(url, format, &cache_key).await
    }

    /// 同步下载并缓存
    async fn download_and_cache(
        &self,
        url: &str,
        format: ImageFormat,
        cache_key: &str,
    ) -> Result<(Vec<u8>, String, String)> {
        info!("FriendAvatar downloading: {}", url);

        // 下载原图
        let raw_bytes = self.download_image(url).await?;

        // 编码为目标格式
        let format_ext = Self::format_extension(format);
        let encoded_bytes = tokio::task::spawn_blocking(move || {
            Self::encode_image_blocking(&raw_bytes, format)
        })
        .await
        .map_err(|e| Error::Internal(format!("Task join error: {}", e)))??;

        // 保存缓存
        self.save_cache(cache_key, &encoded_bytes, url, format_ext).await?;

        debug!("FriendAvatar cached: {} bytes ({})", encoded_bytes.len(), format_ext);
        Ok((encoded_bytes, format_ext.to_string(), "hit".to_string()))
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
                debug!("FriendAvatar already updating: {}", url);
                return Ok(());
            }
            updating.insert(url.to_string());
        }

        // 执行更新
        let result = async {
            let raw_bytes = self.download_image(url).await?;
            let format_ext = Self::format_extension(format);
            let encoded_bytes = tokio::task::spawn_blocking(move || {
                Self::encode_image_blocking(&raw_bytes, format)
            })
            .await
            .map_err(|e| Error::Internal(format!("Task join error: {}", e)))??;

            self.save_cache(cache_key, &encoded_bytes, url, format_ext).await?;
            info!("FriendAvatar background update success: {}", url);
            Ok::<(), Error>(())
        }
        .await;

        // 处理失败情况
        if let Err(e) = result {
            error!("FriendAvatar background update failed: {} - {}", url, e);
            self.mark_update_failure(cache_key).await;
        }

        // 移除更新标记
        {
            let mut updating = self.updating.write().await;
            updating.remove(url);
        }

        Ok(())
    }

    /// 下载原始图片
    async fn download_image(&self, url: &str) -> Result<Vec<u8>> {
        debug!("FriendAvatar fetching URL: {}", url);
        
        let response = self
            .client
            .get(url)
            .header("User-Agent", "Mozilla/5.0 (compatible; MaigoStarlightChecker/1.0; +mailto:tnxg@outlook.jp; ) AppleWebKit/99 (KHTML, like Gecko) Chrome/99 MyGO/5 (KiraKira/DokiDoki; Bananice/Protected) Giraffe/4.11 (Wakarimasu/; Haruhikage/Stop)")
            .send()
            .await
            .map_err(|e| Error::Internal(format!("Failed to fetch image: {}", e)))?;

        let status = response.status();
        debug!("FriendAvatar response status: {}", status);
        
        if !status.is_success() {
            return Err(Error::NotFound(format!(
                "Image not found: HTTP {}",
                status
            )));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| Error::Internal(format!("Failed to read image bytes: {}", e)))?;

        debug!("FriendAvatar downloaded {} bytes", bytes.len());
        Ok(bytes.to_vec())
    }

    /// 阻塞式图片编码
    fn encode_image_blocking(raw_bytes: &[u8], format: ImageFormat) -> Result<Vec<u8>> {
        let img = image::load_from_memory(raw_bytes)
            .map_err(|e| Error::Internal(format!("Failed to decode image: {}", e)))?;

        let mut output = Vec::new();
        img.write_to(&mut Cursor::new(&mut output), format)
            .map_err(|e| Error::Internal(format!("Failed to encode image: {}", e)))?;

        Ok(output)
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
        fs::write(&data_path, data)
            .await
            .map_err(|e| Error::Internal(format!("Failed to write cache data: {}", e)))?;

        // 保存元数据
        let mut metadata = AvatarMetadata::new(url.to_string(), format.to_string());
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
    async fn save_metadata(&self, cache_key: &str, metadata: &AvatarMetadata) -> Result<()> {
        let meta_path = self.cache_dir.join(format!("{}.meta", cache_key));
        let json = serde_json::to_string(metadata)
            .map_err(|e| Error::Internal(format!("Failed to serialize metadata: {}", e)))?;
        fs::write(&meta_path, json)
            .await
            .map_err(|e| Error::Internal(format!("Failed to write metadata: {}", e)))?;
        Ok(())
    }

    /// 加载元数据
    async fn load_metadata(&self, cache_key: &str) -> Option<AvatarMetadata> {
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
        if accept_header.contains("image/avif") {
            ImageFormat::Avif
        } else if accept_header.contains("image/webp") {
            ImageFormat::WebP
        } else {
            ImageFormat::Jpeg
        }
    }

    /// 格式扩展名
    fn format_extension(format: ImageFormat) -> &'static str {
        match format {
            ImageFormat::Avif => "avif",
            ImageFormat::WebP => "webp",
            ImageFormat::Png => "png",
            _ => "jpeg",
        }
    }

    /// 克隆用于后台任务（避免生命周期问题）
    fn clone_for_background(&self) -> Self {
        Self {
            client: self.client.clone(),
            cache_dir: self.cache_dir.clone(),
            updating: RwLock::new(std::collections::HashSet::new()),
        }
    }
}
