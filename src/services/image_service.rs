use crate::utils::cache;
use crate::{Error, Result};
use image::ImageFormat;
use reqwest::Client;
use std::io::Cursor;

pub struct ImageService {
    client: Client,
}

impl ImageService {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    /// 壁纸服务：按格式缓存编码后的图片
    /// 
    /// 缓存策略：
    /// - 缓存 key = url + format (如 avif/webp/jpeg)
    /// - 有缓存：直接返回编码后的数据，无需任何处理
    /// - 无缓存：下载原图 -> 编码为目标格式 -> 缓存编码结果 -> 返回
    /// 
    /// 这样避免了重复的图片解码/编码操作，大幅降低内存占用
    pub async fn fetch_wallpaper(&self, url: &str, accept_header: &str) -> Result<(Vec<u8>, ImageFormat)> {
        // 1. 确定目标格式：avif > webp > jpeg
        let format = self.get_preferred_format(accept_header);
        let format_ext = Self::format_extension(format);
        
        // 2. 缓存 key = url + format
        let cache_key = format!("{}:{}", url, format_ext);
        
        // 3. 检查硬盘缓存（编码后的数据）
        if let Some(cached_data) = cache::get_disk(&cache_key) {
            println!("[Wallpaper] Cache hit: {} ({} bytes)", format_ext, cached_data.len());
            return Ok((cached_data, format));
        }
        
        // 4. 无缓存：下载原图
        println!("[Wallpaper] Cache miss, downloading: {}", url);
        let raw_bytes = self.download_image(url).await?;
        let raw_len = raw_bytes.len();
        
        // 5. 在阻塞线程中处理图片（解码+编码），避免阻塞 async runtime
        let encoded_bytes = tokio::task::spawn_blocking(move || {
            Self::encode_image_blocking(&raw_bytes, format)
            // raw_bytes 在这里被消费并释放
        })
        .await
        .map_err(|e| Error::Internal(format!("Task join error: {}", e)))??;
        
        let encoded_len = encoded_bytes.len();
        println!("[Wallpaper] Encoded: {} -> {} bytes ({})", raw_len, encoded_len, format_ext);
        
        // 6. 异步写入硬盘缓存（编码后的数据）
        let cache_key_clone = cache_key.clone();
        let bytes_for_cache = encoded_bytes.clone();
        tokio::task::spawn_blocking(move || {
            cache::put_disk(&cache_key_clone, &bytes_for_cache);
            // bytes_for_cache 在这里释放
        });
        
        // 7. 返回编码后的数据
        Ok((encoded_bytes, format))
    }

    /// 下载原始图片
    async fn download_image(&self, url: &str) -> Result<Vec<u8>> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| Error::Internal(format!("Failed to fetch image: {}", e)))?;

        if !response.status().is_success() {
            return Err(Error::NotFound(format!(
                "Image not found: HTTP {}",
                response.status()
            )));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| Error::Internal(format!("Failed to read image bytes: {}", e)))?;

        Ok(bytes.to_vec())
    }

    /// 阻塞式图片编码（在 spawn_blocking 中调用）
    fn encode_image_blocking(raw_bytes: &[u8], format: ImageFormat) -> Result<Vec<u8>> {
        // 解码原图
        let img = image::load_from_memory(raw_bytes)
            .map_err(|e| Error::Internal(format!("Failed to decode image: {}", e)))?;

        // 编码为目标格式
        let mut output = Vec::new();
        img.write_to(&mut Cursor::new(&mut output), format)
            .map_err(|e| Error::Internal(format!("Failed to encode image: {}", e)))?;

        // img 在这里被 drop，释放解码后的内存
        Ok(output)
    }

    /// 根据 Accept 头确定最佳格式：avif > webp > jpeg
    pub fn get_preferred_format(&self, accept_header: &str) -> ImageFormat {
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

    /// 头像获取：内存缓存优先（头像通常较小）
    pub async fn fetch_avatar(&self, url: &str) -> Result<(Vec<u8>, bool)> {
        let memory_cache_key = format!("avatar:{}", url);

        // 1. 内存缓存优先
        if let Some(cached) = cache::get(&cache::CACHE_BUCKET, &memory_cache_key).await {
            println!("[Avatar] Memory cache hit: {} bytes", cached.len());
            return Ok((cached, true));
        }

        // 2. 硬盘缓存
        if let Some(cached) = cache::get_disk(url) {
            let len = cached.len();
            // 小于 512KB 提升到内存
            if len < 512 * 1024 {
                let key = memory_cache_key.clone();
                let data = cached.clone();
                tokio::spawn(async move {
                    cache::put(&cache::CACHE_BUCKET, key, data).await;
                });
            }
            println!("[Avatar] Disk cache hit: {} bytes", len);
            return Ok((cached, true));
        }

        // 3. 下载
        let bytes = self.download_image(url).await?;
        let len = bytes.len();

        // 4. 写入缓存
        let url_clone = url.to_string();
        let bytes_for_disk = bytes.clone();
        tokio::task::spawn_blocking(move || {
            cache::put_disk(&url_clone, &bytes_for_disk);
        });

        if len < 512 * 1024 {
            cache::put(&cache::CACHE_BUCKET, memory_cache_key, bytes.clone()).await;
        }

        println!("[Avatar] Downloaded: {} bytes", len);
        Ok((bytes, false))
    }
}
