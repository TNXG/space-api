use crate::utils::cache;
use crate::{Error, Result};
use bytes::Bytes;
use image::{DynamicImage, ImageFormat};
use reqwest::Client;
use std::io::Cursor;
use std::sync::Arc;

pub struct ImageService {
    client: Client,
}

impl ImageService {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    /// 壁纸服务专用：硬盘缓存优先的内存优化图片获取
    /// 
    /// 核心逻辑：
    /// - 有缓存：直接读取硬盘返回，全程不占用额外内存
    /// - 无缓存：下载至内存作为中转，并行执行"返回数据"和"写入硬盘"，完成后立即释放
    /// 
    /// 返回 (bytes, cache_hit)
    pub async fn fetch_image(&self, url: &str) -> Result<(Vec<u8>, bool)> {
        // 1. 硬盘缓存优先：有缓存直接读取返回
        if let Some(cached_image) = cache::get_disk(url) {
            println!("[ImageService] Disk cache hit: {} bytes", cached_image.len());
            return Ok((cached_image, true));
        }

        // 2. 无缓存：网络请求下载图片至内存（作为中转）
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

        // 使用 Bytes 零拷贝获取数据，减少内存分配
        let image_bytes: Bytes = response
            .bytes()
            .await
            .map_err(|e| Error::Internal(format!("Failed to read image bytes: {}", e)))?;

        let bytes_len = image_bytes.len();
        println!("[ImageService] Downloaded: {} bytes from {}", bytes_len, url);

        // 3. 并行处理：使用 Arc 共享数据，避免克隆
        let shared_bytes = Arc::new(image_bytes);
        let url_for_cache = url.to_string();
        let bytes_for_cache = Arc::clone(&shared_bytes);

        // 异步写入硬盘缓存（不阻塞返回）
        tokio::task::spawn_blocking(move || {
            cache::put_disk(&url_for_cache, &bytes_for_cache);
            // bytes_for_cache 的 Arc 引用在此释放
            println!("[ImageService] Disk cache write completed: {}", url_for_cache);
        });

        // 4. 返回数据给调用方
        // Arc::try_unwrap 尝试获取所有权，如果还有其他引用则克隆
        let result_bytes = match Arc::try_unwrap(shared_bytes) {
            Ok(bytes) => bytes.to_vec(),
            Err(arc) => arc.to_vec(),
        };

        // 此时 shared_bytes 已被消费或释放，内存得到及时回收
        Ok((result_bytes, false))
    }

    /// 头像获取：内存缓存优先（头像通常较小，适合内存缓存）
    /// 
    /// 缓存策略：内存 -> 硬盘 -> 网络
    /// 头像较小，允许内存缓存以提升响应速度
    pub async fn fetch_avatar(&self, url: &str) -> Result<(Vec<u8>, bool)> {
        let memory_cache_key = format!("avatar_raw:{}", url);

        // 1. 内存缓存优先（头像小，适合内存）
        if let Some(cached_avatar) = cache::get(&cache::CACHE_BUCKET, &memory_cache_key).await {
            println!("[ImageService] Avatar memory cache hit: {} bytes", cached_avatar.len());
            return Ok((cached_avatar, true));
        }

        // 2. 硬盘缓存次之
        if let Some(cached_image) = cache::get_disk(url) {
            let bytes_len = cached_image.len();
            
            // 小于 512KB 的头像提升到内存缓存
            if bytes_len < 512 * 1024 {
                let memory_key = memory_cache_key.clone();
                let bytes_for_memory = cached_image.clone();
                tokio::spawn(async move {
                    cache::put(&cache::CACHE_BUCKET, memory_key, bytes_for_memory).await;
                });
            }
            
            println!("[ImageService] Avatar disk cache hit: {} bytes", bytes_len);
            return Ok((cached_image, true));
        }

        // 3. 网络下载
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| Error::Internal(format!("Failed to fetch avatar: {}", e)))?;

        if !response.status().is_success() {
            return Err(Error::NotFound(format!(
                "Avatar not found: HTTP {}",
                response.status()
            )));
        }

        let image_bytes: Bytes = response
            .bytes()
            .await
            .map_err(|e| Error::Internal(format!("Failed to read avatar bytes: {}", e)))?;

        let bytes_len = image_bytes.len();
        let result_bytes = image_bytes.to_vec();

        // 4. 并行写入缓存
        let url_for_disk = url.to_string();
        let bytes_for_disk = result_bytes.clone();
        
        // 异步写入硬盘
        tokio::task::spawn_blocking(move || {
            cache::put_disk(&url_for_disk, &bytes_for_disk);
        });

        // 小头像放入内存缓存
        if bytes_len < 512 * 1024 {
            cache::put(&cache::CACHE_BUCKET, memory_cache_key, result_bytes.clone()).await;
        }

        println!("[ImageService] Avatar downloaded: {} bytes", bytes_len);
        Ok((result_bytes, false))
    }

    // 处理图像（调整大小等）
    pub async fn process_image(
        &self,
        image_data: Vec<u8>,
        width: Option<u32>,
        height: Option<u32>,
        format: ImageFormat,
    ) -> Result<Vec<u8>> {
        // 解析图像
        let img = image::load_from_memory(&image_data)
            .map_err(|e| Error::Internal(format!("Failed to parse image: {}", e)))?;

        // 如果需要，调整图像大小
        let processed_img = if let (Some(w), Some(h)) = (width, height) {
            DynamicImage::ImageRgba8(
                img.resize(w, h, image::imageops::FilterType::Lanczos3)
                    .to_rgba8(),
            )
        } else if let Some(w) = width {
            let ratio = w as f32 / img.width() as f32;
            let new_height = (img.height() as f32 * ratio) as u32;
            DynamicImage::ImageRgba8(
                img.resize(w, new_height, image::imageops::FilterType::Lanczos3)
                    .to_rgba8(),
            )
        } else if let Some(h) = height {
            let ratio = h as f32 / img.height() as f32;
            let new_width = (img.width() as f32 * ratio) as u32;
            DynamicImage::ImageRgba8(
                img.resize(new_width, h, image::imageops::FilterType::Lanczos3)
                    .to_rgba8(),
            )
        } else {
            img
        };

        // 编码图像为指定格式
        let mut result_bytes: Vec<u8> = Vec::new();
        processed_img
            .write_to(&mut Cursor::new(&mut result_bytes), format)
            .map_err(|e| Error::Internal(format!("Failed to encode image: {}", e)))?;

        Ok(result_bytes)
    }

    // 根据Content-Type获取图像格式
    pub fn get_image_format_from_content_type(&self, content_type: &str) -> ImageFormat {
        match content_type {
            "image/jpeg" | "image/jpg" => ImageFormat::Jpeg,
            "image/png" => ImageFormat::Png,
            "image/gif" => ImageFormat::Gif,
            "image/webp" => ImageFormat::WebP,
            "image/avif" => ImageFormat::Avif,
            _ => ImageFormat::Jpeg, // 默认为JPEG
        }
    }

    // 根据Accept头获取最合适的图像格式
    pub fn get_preferred_image_format(&self, accept_header: &str) -> ImageFormat {
        if accept_header.contains("image/avif") {
            ImageFormat::Avif
        } else if accept_header.contains("image/webp") {
            ImageFormat::WebP
        } else if accept_header.contains("image/png") {
            ImageFormat::Png
        } else {
            ImageFormat::Jpeg
        }
    }
}
