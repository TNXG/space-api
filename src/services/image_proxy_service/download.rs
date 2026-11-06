use super::{ImageProxyFetch, ImageProxyService};
use crate::services::image_service::ImageService;
use crate::{Error, Result};
use image::ImageFormat;
use log::{debug, error, info};
use tokio::io::AsyncWriteExt;

impl ImageProxyService {
    /// 首次拉取时，客户端支持源格式则流式透传，否则完成转码后返回。
    pub(super) async fn download_or_stream(
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
    pub(super) async fn download_image(&self, url: &str) -> Result<Vec<u8>> {
        let (response, initial_bytes, _) = self.begin_download(url).await?;
        self.finish_download(response, initial_bytes).await
    }
}
