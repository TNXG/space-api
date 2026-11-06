use super::{MemoryError, MemoryManager, ReleaseResult};
use crate::utils::jemalloc_interface::JemallocInterface;
use chrono::Utc;
use std::time::Instant;

impl MemoryManager {
    /// 清理缓存条目
    async fn cleanup_cache(&self) -> Result<usize, MemoryError> {
        use crate::utils::cache::{cleanup_expired_cache, CACHE_BUCKET};

        log::debug!("Starting cache cleanup operation");

        // 获取清理前的缓存条目数量
        let before_count = CACHE_BUCKET.entry_count();
        log::debug!("Cache entries before cleanup: {}", before_count);

        // 清理内存缓存中的过期条目
        match tokio::time::timeout(
            tokio::time::Duration::from_secs(30), // 30秒超时
            CACHE_BUCKET.run_pending_tasks(),
        )
        .await
        {
            Ok(_) => {
                log::debug!("Memory cache cleanup completed successfully");
            }
            Err(_) => {
                log::warn!("Memory cache cleanup timed out after 30 seconds");
                return Err(MemoryError::CacheCleanupFailed(
                    "Memory cache cleanup timeout".to_string(),
                ));
            }
        }

        // 清理磁盘缓存（在阻塞线程中执行，避免阻塞 async runtime）
        match tokio::task::spawn_blocking(|| {
            std::panic::catch_unwind(|| {
                cleanup_expired_cache();
            })
        })
        .await
        {
            Ok(Ok(_)) => {
                log::debug!("Disk cache cleanup completed successfully");
            }
            Ok(Err(_)) => {
                log::warn!(
                    "Disk cache cleanup panicked, continuing with memory cache cleanup only"
                );
            }
            Err(e) => {
                log::warn!(
                    "Disk cache cleanup task failed: {}, continuing with memory cache cleanup only",
                    e
                );
            }
        }

        // 获取清理后的缓存条目数量
        let after_count = CACHE_BUCKET.entry_count();
        let cleaned_count = before_count.saturating_sub(after_count);

        log::info!(
            "Cache cleanup completed: removed {} memory cache entries (before: {}, after: {})",
            cleaned_count,
            before_count,
            after_count
        );

        Ok(cleaned_count as usize)
    }

    /// 触发全局内存释放操作
    pub async fn trigger_global_release(&self) -> Result<ReleaseResult, MemoryError> {
        let start_time = Instant::now();
        let mut result = ReleaseResult {
            memory_freed_mb: 0,
            cache_entries_cleared: 0,
            gc_executed: false,
            timestamp: Utc::now(),
        };

        log::info!(
            "Starting global memory release operation (threshold: {} MB)",
            self.config.threshold_mb
        );

        // 获取释放前的内存使用量
        let memory_before = match self.get_current_memory_usage().await {
            Ok(usage) => {
                log::debug!("Memory usage before release: {} MB", usage);
                usage
            }
            Err(e) => {
                log::warn!(
                    "Failed to get memory usage before release: {}, using 0 as fallback",
                    e
                );
                0
            }
        };

        // 1. 清理缓存（总是执行，即使失败也继续）
        match self.cleanup_cache().await {
            Ok(cleaned_count) => {
                result.cache_entries_cleared = cleaned_count;
                log::info!(
                    "Cache cleanup successful: {} entries removed",
                    cleaned_count
                );
            }
            Err(e) => {
                log::error!("Cache cleanup failed: {}, continuing with GC", e);
                result.cache_entries_cleared = 0;
                // 不返回错误，继续执行GC
            }
        }

        // 2. 执行jemalloc垃圾回收（如果可用）
        if JemallocInterface::is_available() {
            log::debug!("Attempting jemalloc garbage collection");

            match tokio::time::timeout(
                tokio::time::Duration::from_secs(10), // 10秒超时
                tokio::task::spawn_blocking(|| JemallocInterface::purge_dirty_pages()),
            )
            .await
            {
                Ok(Ok(_)) => {
                    result.gc_executed = true;
                    log::info!("Jemalloc garbage collection executed successfully");

                    // 重置失败计数
                    {
                        let mut failure_count = self.gc_failure_count.lock().await;
                        if *failure_count > 0 {
                            log::info!("Resetting GC failure count from {} to 0", *failure_count);
                            *failure_count = 0;
                        }
                    }
                }
                Ok(Err(e)) => {
                    log::warn!("Jemalloc garbage collection failed: {}", e);
                    self.handle_gc_failure().await;
                }
                Err(_) => {
                    log::warn!("Jemalloc garbage collection timed out after 10 seconds");
                    self.handle_gc_failure().await;
                }
            }
        } else {
            log::debug!("Jemalloc not available, skipping garbage collection");
        }

        // 等待一小段时间让内存释放生效
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        // 3. 计算释放的内存量
        let memory_after = match self.get_current_memory_usage().await {
            Ok(usage) => {
                log::debug!("Memory usage after release: {} MB", usage);
                usage
            }
            Err(e) => {
                log::warn!(
                    "Failed to get memory usage after release: {}, assuming no change",
                    e
                );
                memory_before // 如果获取失败，假设没有变化
            }
        };

        result.memory_freed_mb = memory_before.saturating_sub(memory_after);

        // 4. 更新内存压力等级
        if let Err(e) = self.safe_update_memory_pressure(memory_after).await {
            log::warn!("Failed to update memory pressure: {}", e);
        }

        // 5. 更新最后GC时间和监控状态
        let now = Instant::now();
        if let Err(e) = self.update_gc_timestamp_and_stats(now, &result).await {
            log::warn!("Failed to update GC statistics: {}", e);
        }

        let duration = start_time.elapsed();
        let effectiveness = if memory_before > 0 {
            (result.memory_freed_mb as f64 / memory_before as f64) * 100.0
        } else {
            0.0
        };

        log::info!(
            "Global memory release completed in {:?}: freed {} MB ({:.1}% of {} MB), cleared {} cache entries, GC executed: {}",
            duration,
            result.memory_freed_mb,
            effectiveness,
            memory_before,
            result.cache_entries_cleared,
            result.gc_executed
        );

        // 检查释放效果
        if result.memory_freed_mb == 0 && result.cache_entries_cleared == 0 && !result.gc_executed {
            log::warn!("Memory release operation had no effect - no memory freed, no cache cleared, no GC executed");
        } else if result.memory_freed_mb < memory_before / 10 && memory_before > 100 {
            log::warn!("Memory release was less effective than expected: only freed {} MB out of {} MB ({:.1}%)", 
                result.memory_freed_mb, memory_before, effectiveness);
        }

        Ok(result)
    }

    /// 处理GC失败的情况
    async fn handle_gc_failure(&self) {
        let mut failure_count = self.gc_failure_count.lock().await;
        *failure_count += 1;

        match *failure_count {
            1..=2 => {
                log::warn!(
                    "Jemalloc garbage collection failed {} time(s)",
                    *failure_count
                );
            }
            3..=5 => {
                log::error!("Jemalloc garbage collection has failed {} times consecutively - this may indicate a serious issue", *failure_count);
            }
            count if count > 5 => {
                log::error!("Jemalloc garbage collection has failed {} times consecutively - consider restarting the application", count);
            }
            _ => {}
        }
    }


    /// 更新GC时间戳和统计信息
    async fn update_gc_timestamp_and_stats(
        &self,
        timestamp: Instant,
        result: &ReleaseResult,
    ) -> Result<(), MemoryError> {
        // 更新最后GC时间
        {
            let mut last_gc = self.last_gc_time.lock().await;
            *last_gc = timestamp;
        }

        // 更新监控状态
        {
            let mut state = self.monitor_state.lock().await;
            state.last_release_time = Some(timestamp);
            state.release_count += 1;
            state.total_freed_mb += result.memory_freed_mb;

            log::debug!(
                "Updated GC statistics: total releases: {}, total freed: {} MB",
                state.release_count,
                state.total_freed_mb
            );
        }

        Ok(())
    }

    /// 检查内存使用并在必要时触发释放
    pub async fn check_and_release_if_needed(&self) -> Result<Option<ReleaseResult>, MemoryError> {
        let current_memory = self.get_current_memory_usage().await?;

        // 更新内存压力等级
        self.update_memory_pressure(current_memory).await;

        // 检查是否需要触发释放
        if self.should_trigger_release(current_memory).await {
            log::info!(
                "Memory usage ({} MB) exceeds threshold ({} MB), triggering release",
                current_memory,
                self.config.threshold_mb
            );

            let result = self.trigger_global_release().await?;
            Ok(Some(result))
        } else {
            Ok(None)
        }
    }
}
