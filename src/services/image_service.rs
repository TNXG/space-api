use crate::utils::cache;
use crate::{Error, Result};
use image::{DynamicImage, ImageFormat};
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

    // 从URL获取图像数据，返回 (bytes, cache_hit)
    pub async fn fetch_image(&self, url: &str) -> Result<(Vec<u8>, bool)> {
        // 检查缓存
        if let Some(cached_image) = cache::get(&cache::CACHE_BUCKET, &url.to_string()).await {
            return Ok((cached_image, true));
        }

        // 发起HTTP请求获取图像
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

        // 获取图像数据
        let image_bytes = response
            .bytes()
            .await
            .map_err(|e| Error::Internal(format!("Failed to read image bytes: {}", e)))?
            .to_vec();

        // 缓存图像
        cache::put(&cache::CACHE_BUCKET, url.to_string(), image_bytes.clone()).await;

        Ok((image_bytes, false))
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
            _ => ImageFormat::Jpeg, // 默认为JPEG
        }
    }

    // 根据Accept头获取最合适的图像格式
    pub fn get_preferred_image_format(&self, accept_header: &str) -> ImageFormat {
        if accept_header.contains("image/webp") {
            ImageFormat::WebP
        } else if accept_header.contains("image/png") {
            ImageFormat::Png
        } else {
            ImageFormat::Jpeg
        }
    }
}
