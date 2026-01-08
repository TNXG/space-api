use crate::services::image_service::ImageService;
use crate::{Error, Result};
use image::ImageFormat;
use log::{debug, error, info};
use reqwest::Client;
use serde::{Deserialize, Serialize};
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
    /// 缓存策略（SWR - Stale While Revalidate）：
    /// 1. 有缓存 -> 立即返回，根据新鲜度决定是否后台更新
    /// 2. 无缓存 -> 同步下载
    /// 3. 强制刷新 -> 同步下载
    pub async fn fetch_friend_avatar(
        &self,
        url: &str,
        accept_header: &str,
        force_refresh: bool,
    ) -> Result<(Vec<u8>, String, String)> {
        let target_format = self.get_preferred_format(accept_header);
        let target_format_ext = ImageService::format_extension(target_format);
        
        // 尝试多种格式的缓存（优先目标格式，其次 avif/webp/jpeg）
        let formats_to_try = [target_format_ext, "avif", "webp", "jpeg"];
        
        info!("[友链头像] 请求: {} (目标格式: {})", url, target_format_ext);
        
        // 强制刷新：直接下载
        if force_refresh {
            info!("[友链头像] 强制刷新: {}", url);
            let cache_key = self.get_cache_key(url, target_format_ext);
            return self.download_and_cache(url, target_format, &cache_key).await;
        }

        // 尝试读取缓存（按格式优先级）
        for format_ext in &formats_to_try {
            let cache_key = self.get_cache_key(url, format_ext);
            info!("[友链头像] 尝试读取缓存: format={}, cache_key={}", format_ext, cache_key);
            let cached_data = self.load_cache_data(&cache_key).await;
            let metadata = self.load_metadata(&cache_key).await;

            match (&cached_data, &metadata) {
                (Some(_), Some(_)) => {
                    info!("[友链头像] 找到缓存文件: {}", format_ext);
                }
                (Some(_), None) => {
                    info!("[友链头像] 找到数据但无元数据: {}", format_ext);
                }
                (None, Some(_)) => {
                    info!("[友链头像] 找到元数据但无数据: {}", format_ext);
                }
                (None, None) => {
                    info!("[友链头像] 无缓存: {}", format_ext);
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

                info!("[友链头像] 缓存状态 [{}]: fresh={}, expired={}, legacy={}", 
                    format_ext, is_fresh, is_expired, meta.legacy_mode);

                // 任何非新鲜的缓存都触发后台更新（包括过期的）
                if !is_fresh {
                    info!("[友链头像] 缓存不新鲜，触发后台更新: {}", url);
                    let service = self.clone_for_background();
                    let url_clone = url.to_string();
                    let cache_key_clone = cache_key.clone();
                    let target_format_clone = target_format;
                    tokio::spawn(async move {
                        info!("[友链头像] 后台任务已启动: {}", url_clone);
                        let _ = service.background_update(&url_clone, target_format_clone, &cache_key_clone).await;
                    });
                }

                // 立即返回缓存数据
                info!("[友链头像] 返回缓存 [{}]: {}", status, url);
                return Ok((data, format_ext.to_string(), status.to_string()));
            }
        }

        // 无缓存：同步下载
        info!("[友链头像] 无缓存，开始下载: {}", url);
        let cache_key = self.get_cache_key(url, target_format_ext);
        self.download_and_cache(url, target_format, &cache_key).await
    }

    /// 同步下载并缓存
    async fn download_and_cache(
        &self,
        url: &str,
        format: ImageFormat,
        cache_key: &str,
    ) -> Result<(Vec<u8>, String, String)> {
        // 下载原图
        let raw_bytes = self.download_image(url).await?;
        info!("[友链头像] 下载完成: {} ({} 字节)", url, raw_bytes.len());

        // 智能转码（AVIF 等无法解码的格式会透传）
        let (final_bytes, final_format) = tokio::task::spawn_blocking(move || {
            ImageService::smart_transcode(raw_bytes, format)
        })
        .await
        .map_err(|e| Error::Internal(format!("Task join error: {}", e)))??;

        let format_ext = ImageService::format_extension(final_format);
        
        // 如果格式变了（如 AVIF 透传），需要用新的 cache_key
        let actual_cache_key = if final_format != format {
            info!("[友链头像] 格式变更: {} -> {}", ImageService::format_extension(format), format_ext);
            self.get_cache_key(url, format_ext)
        } else {
            cache_key.to_string()
        };
        
        // 保存缓存
        self.save_cache(&actual_cache_key, &final_bytes, url, format_ext).await?;

        info!("[友链头像] 缓存已保存: {} ({} 字节, {})", url, final_bytes.len(), format_ext);
        Ok((final_bytes, format_ext.to_string(), "hit".to_string()))
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
                debug!("[友链头像] 已在更新中，跳过: {}", url);
                return Ok(());
            }
            updating.insert(url.to_string());
        }

        info!("[友链头像] 后台更新开始: {}", url);

        // 执行更新
        let result = async {
            let raw_bytes = self.download_image(url).await?;
            info!("[友链头像] 后台下载完成: {} ({} 字节)", url, raw_bytes.len());
            
            // 智能转码
            let (final_bytes, final_format) = tokio::task::spawn_blocking(move || {
                ImageService::smart_transcode(raw_bytes, format)
            })
            .await
            .map_err(|e| Error::Internal(format!("Task join error: {}", e)))??;

            let final_format_ext = ImageService::format_extension(final_format);
            
            // 如果格式变了（如 AVIF 透传），需要用新的 cache_key
            let actual_cache_key = if final_format != format {
                info!("[友链头像] 后台更新格式变更: {} -> {}", 
                    ImageService::format_extension(format), final_format_ext);
                self.get_cache_key(url, final_format_ext)
            } else {
                cache_key.to_string()
            };

            self.save_cache(&actual_cache_key, &final_bytes, url, final_format_ext).await?;
            info!("[友链头像] 后台更新成功: {} ({} 字节, {})", url, final_bytes.len(), final_format_ext);
            Ok::<(), Error>(())
        }
        .await;

        // 处理失败情况
        if let Err(e) = result {
            error!("[友链头像] 后台更新失败: {} - {}", url, e);
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
        debug!("[友链头像] 正在请求: {}", url);
        
        let response = self
            .client
            .get(url)
            .header("User-Agent", "Mozilla/5.0 (compatible; MaigoStarlightChecker/1.0; +mailto:tnxg@outlook.jp; ) AppleWebKit/99 (KHTML, like Gecko) Chrome/99 MyGO/5 (KiraKira/DokiDoki; Bananice/Protected) Giraffe/4.11 (Wakarimasu/; Haruhikage/Stop)")
            .send()
            .await
            .map_err(|e| Error::Internal(format!("请求失败: {}", e)))?;

        let status = response.status();
        debug!("[友链头像] 响应状态: {}", status);
        
        if !status.is_success() {
            return Err(Error::NotFound(format!(
                "图片未找到: HTTP {}",
                status
            )));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| Error::Internal(format!("读取响应失败: {}", e)))?;

        Ok(bytes.to_vec())
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

    /// 克隆用于后台任务（避免生命周期问题）
    fn clone_for_background(&self) -> Self {
        Self {
            client: self.client.clone(),
            cache_dir: self.cache_dir.clone(),
            updating: RwLock::new(std::collections::HashSet::new()),
        }
    }
}
