use super::{ImageProxyMetadata, ImageProxyService};
use crate::services::image_service::ImageService;
use crate::{Error, Result};
use image::ImageFormat;
use log::{debug, error, info};
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::fs;

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
    pub(super) fn is_fresh(&self) -> bool {
        let now = now_secs();
        now.saturating_sub(self.last_check_time) < 2 * 60 * 60 // 2小时
    }

    /// 检查缓存是否过期（30天）
    pub(super) fn is_expired(&self) -> bool {
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

impl ImageProxyService {
    /// 后台更新（SWR）
    pub(super) async fn background_update(
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

    /// 保存缓存（数据 + 元数据）
    pub(super) async fn save_cache(
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
    pub(super) async fn load_cache_data(&self, cache_key: &str) -> Option<Vec<u8>> {
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
    pub(super) async fn load_metadata(&self, cache_key: &str) -> Option<ImageProxyMetadata> {
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
    pub(super) fn get_cache_key(&self, url: &str, format: &str) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(url.as_bytes());
        let hash = hex::encode(hasher.finalize());
        format!("{}_{}", &hash[..16], format)
    }
}
