use super::{MemoryError, MemoryManager};
use crate::utils::jemalloc_interface::{JemallocError, JemallocInterface};
use std::time::Instant;

impl MemoryManager {
    /// 获取当前内存使用量（MB）- 性能优化版本
    pub async fn get_current_memory_usage(&self) -> Result<u64, MemoryError> {
        let query_start = Instant::now();

        if JemallocInterface::is_available() {
            match tokio::time::timeout(
                tokio::time::Duration::from_secs(5),
                tokio::task::spawn_blocking(|| JemallocInterface::get_allocated_bytes()),
            )
            .await
            {
                Ok(Ok(Ok(bytes))) if bytes > 0 => {
                    let mb = bytes / 1024 / 1024;
                    if mb > 0 {
                        return Ok(mb);
                    }
                }
                _ => {}
            }
        }

        // 回退到系统内存使用量
        match tokio::task::spawn_blocking(move || {
            use sysinfo::{Pid, ProcessesToUpdate, System};

            let mut sys = System::new().map_err(|e| {
                MemoryError::MetricsCollectionFailed(format!("系统信息初始化失败: {e}"))
            })?;
            let current_pid = Pid::from(std::process::id() as usize);

            // 使用正确的API刷新进程信息
            sys.refresh_processes(ProcessesToUpdate::Some(&[current_pid]), true);

            if let Some(process) = sys.process(current_pid) {
                let memory_bytes = process.memory();
                let memory_mb = memory_bytes / 1024 / 1024;

                Ok((memory_bytes, memory_mb))
            } else {
                Err(MemoryError::MetricsCollectionFailed(format!(
                    "Unable to find process with PID {}",
                    current_pid
                )))
            }
        })
        .await
        {
            Ok(Ok((memory_bytes, memory_mb))) => {
                let query_duration = query_start.elapsed();
                log::debug!(
                    "System memory usage: {} MB ({} bytes) (retrieved in {:?})",
                    memory_mb,
                    memory_bytes,
                    query_duration
                );

                // 验证内存使用量的合理性
                if memory_mb > 50000 {
                    // 超过50GB可能有问题
                    log::warn!("Unusually high memory usage detected: {} MB - this may indicate a memory leak", memory_mb);
                } else if memory_mb == 0 {
                    log::warn!(
                        "Zero memory usage reported - this may indicate a measurement issue"
                    );
                }

                // 更新性能统计
                self.update_memory_query_stats(query_duration, true).await;

                // 更新内存历史记录
                self.update_memory_history(memory_mb).await;

                Ok(memory_mb)
            }
            Ok(Err(e)) => {
                let query_duration = query_start.elapsed();
                log::error!("Failed to get system memory usage: {}", e);
                self.update_memory_query_stats(query_duration, false).await;
                Err(e)
            }
            Err(e) => {
                let query_duration = query_start.elapsed();
                log::error!("System memory query task failed: {}", e);
                self.update_memory_query_stats(query_duration, false).await;
                Err(MemoryError::MetricsCollectionFailed(format!(
                    "System memory query task panicked: {}",
                    e
                )))
            }
        }
    }

    /// 验证jemalloc配置
    pub fn validate_jemalloc_config(&self) -> Result<(), MemoryError> {
        match JemallocInterface::validate_config() {
            Ok(_) => {
                log::info!("Jemalloc configuration validated successfully");
                Ok(())
            }
            Err(JemallocError::NotAvailable) => {
                log::warn!("Jemalloc not available, will use fallback memory management");
                Ok(()) // 不可用不算错误，只是会使用回退机制
            }
            Err(e) => {
                log::error!("Jemalloc configuration validation failed: {}", e);
                Err(MemoryError::InvalidConfig(e.to_string()))
            }
        }
    }
}
