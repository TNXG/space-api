use crate::utils::cache;
use crate::{Error, Result};
use image::ImageFormat;
use log::{debug, info};
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

    /// 获取上游响应头后立即返回，调用方流式转发响应体，不解码或重编码。
    pub async fn open_wallpaper(&self, url: &str) -> Result<reqwest::Response> {
        let response = self.client.get(url)
            .header(reqwest::header::ACCEPT_ENCODING, "identity")
            .send().await
            .map_err(|e| Error::Internal(format!("Failed to fetch wallpaper: {e}")))?;
        if !response.status().is_success() {
            return Err(Error::Internal(format!(
                "Wallpaper upstream returned HTTP {}", response.status()
            )));
        }
        Ok(response)
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


    /// 格式扩展名
    pub fn format_extension(format: ImageFormat) -> &'static str {
        match format {
            ImageFormat::Avif => "avif",
            ImageFormat::WebP => "webp",
            ImageFormat::Png => "png",
            ImageFormat::Gif => "gif",
            _ => "jpeg",
        }
    }

    /// 检测图片格式（通过魔数）
    pub fn detect_format(bytes: &[u8]) -> Option<ImageFormat> {
        if bytes.len() < 12 {
            return None;
        }
        
        // AVIF: 检查 ftyp box
        if &bytes[4..8] == b"ftyp" {
            let brand = &bytes[8..12];
            if brand == b"avif" || brand == b"avis" || brand == b"mif1" {
                return Some(ImageFormat::Avif);
            }
        }
        
        // PNG: 89 50 4E 47
        if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
            return Some(ImageFormat::Png);
        }
        
        // JPEG: FF D8 FF
        if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
            return Some(ImageFormat::Jpeg);
        }
        
        // WebP: RIFF....WEBP
        if &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
            return Some(ImageFormat::WebP);
        }
        
        // GIF: GIF87a or GIF89a
        if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
            return Some(ImageFormat::Gif);
        }
        
        None
    }

    /// 智能转码：如果源格式无法解码或已是目标格式则透传
    /// 
    /// 返回 (图片数据, 实际格式)
    pub fn smart_transcode(raw_bytes: Vec<u8>, target_format: ImageFormat) -> Result<(Vec<u8>, ImageFormat)> {
        // 检测源格式
        if let Some(source_format) = Self::detect_format(&raw_bytes) {
            // 已经是目标格式，直接返回
            if source_format == target_format {
                debug!("Image already in target format ({}), passing through", Self::format_extension(target_format));
                return Ok((raw_bytes, target_format));
            }
            
            // AVIF 无法解码，直接透传
            if source_format == ImageFormat::Avif {
                debug!("Source is AVIF (cannot decode), passing through");
                return Ok((raw_bytes, ImageFormat::Avif));
            }
        }
        
        // 尝试转码
        let encoded = Self::encode_image_blocking(&raw_bytes, target_format)?;
        Ok((encoded, target_format))
    }

    /// 头像获取：内存缓存优先（头像通常较小）
    pub async fn fetch_avatar(&self, url: &str) -> Result<(Vec<u8>, bool)> {
        let memory_cache_key = format!("avatar:{}", url);

        // 1. 内存缓存优先
        if let Some(cached) = cache::get(&cache::CACHE_BUCKET, &memory_cache_key).await {
            debug!("Avatar memory cache hit: {} bytes", cached.len());
            return Ok((cached, true));
        }

        // 2. 硬盘缓存
        if let Some(cached) = cache::get_disk(url) {
            let len = cached.len();
            // 小于 512KB 提升到内存（直接 move 进 spawn，避免 clone）
            if len < 512 * 1024 {
                let key = memory_cache_key.clone();
                let data = cached.clone(); // 需要 clone 一份给内存缓存
                tokio::spawn(async move {
                    cache::put(&cache::CACHE_BUCKET, key, data).await;
                });
            }
            debug!("Avatar disk cache hit: {} bytes", len);
            return Ok((cached, true));
        }

        // 3. 下载
        let bytes = self.download_image(url).await?;
        let len = bytes.len();

        // 4. 写入缓存（使用 Arc 共享数据避免多次深拷贝）
        let bytes_arc = std::sync::Arc::new(bytes);
        {
            let url_clone = url.to_string();
            let bytes_for_disk = std::sync::Arc::clone(&bytes_arc);
            tokio::task::spawn_blocking(move || {
                cache::put_disk(&url_clone, &bytes_for_disk);
            });
        }

        if len < 512 * 1024 {
            cache::put(&cache::CACHE_BUCKET, memory_cache_key, (*bytes_arc).clone()).await;
        }

        let bytes = std::sync::Arc::try_unwrap(bytes_arc)
            .unwrap_or_else(|arc| (*arc).clone());

        info!("Avatar downloaded: {} bytes", len);
        Ok((bytes, false))
    }
}

#[cfg(test)]
include!("../../tests/unit/image_service.rs");
