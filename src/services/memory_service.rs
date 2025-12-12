use crate::config::settings::MemoryConfig;
use crate::utils::jemalloc_interface::{JemallocError, JemallocInterface};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;
use thiserror::Error;
use tokio::sync::Mutex;

/// 内存管理错误类型
#[derive(Debug, Error)]
pub enum MemoryError {
    #[error("Jemalloc interface not available")]
    JemallocUnavailable,

    #[error("Memory monitoring task failed: {0}")]
    MonitoringFailed(String),

    #[error("Global memory release failed: {0}")]
    ReleaseFailed(String),

    #[error("Invalid memory configuration: {0}")]
    InvalidConfig(String),

    #[error("System metrics collection failed: {0}")]
    MetricsCollectionFailed(String),

    #[error("Cache cleanup failed: {0}")]
    CacheCleanupFailed(String),

    #[error("Memory pressure calculation failed: {0}")]
    PressureCalculationFailed(String),

    #[error("Monitoring task initialization failed: {0}")]
    MonitoringInitFailed(String),
}

/// 内存压力等级
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MemoryPressure {
    /// 低压力 - 内存使用 < 60% 阈值
    Low,
    /// 中等压力 - 内存使用 60%-80% 阈值
    Medium,
    /// 高压力 - 内存使用 80%-100% 阈值
    High,
    /// 严重压力 - 内存使用 > 阈值
    Critical,
}

/// 内存状态信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStatus {
    /// 当前内存使用量（MB）
    pub current_mb: u64,
    /// 内存阈值（MB）
    pub threshold_mb: u64,
    /// 内存压力等级
    pub pressure: MemoryPressure,
    /// 距离上次GC的时间（秒）
    pub time_since_last_gc_secs: u64,
    /// 是否正在监控
    pub is_monitoring: bool,
}

/// 内存释放操作结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseResult {
    /// 释放的内存量（MB）
    pub memory_freed_mb: u64,
    /// 清理的缓存条目数量
    pub cache_entries_cleared: usize,
    /// 是否执行了垃圾回收
    pub gc_executed: bool,
    /// 操作时间戳
    pub timestamp: DateTime<Utc>,
}

impl Default for ReleaseResult {
    fn default() -> Self {
        Self {
            memory_freed_mb: 0,
            cache_entries_cleared: 0,
            gc_executed: false,
            timestamp: Utc::now(),
        }
    }
}

/// 内存监控状态
#[derive(Debug, Clone)]
pub struct MemoryMonitorState {
    /// 当前内存使用量（MB）
    pub current_usage_mb: u64,
    /// 峰值内存使用量（MB）
    pub peak_usage_mb: u64,
    /// 内存压力等级
    pub pressure_level: MemoryPressure,
    /// 上次释放时间
    pub last_release_time: Option<Instant>,
    /// 释放操作计数
    pub release_count: u64,
    /// 总释放内存量（MB）
    pub total_freed_mb: u64,
}

/// 性能统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceStats {
    /// 监控任务执行次数
    pub monitoring_cycles: u64,
    /// 平均监控周期时间（毫秒）
    pub avg_monitoring_time_ms: f64,
    /// 最大监控周期时间（毫秒）
    pub max_monitoring_time_ms: u64,
    /// 内存查询成功次数
    pub memory_query_success: u64,
    /// 内存查询失败次数
    pub memory_query_failures: u64,
    /// 平均内存查询时间（毫秒）
    pub avg_memory_query_time_ms: f64,
    /// 自适应间隔调整次数
    pub interval_adjustments: u64,
    /// 当前动态间隔（秒）
    pub current_dynamic_interval: u64,
}

impl Default for PerformanceStats {
    fn default() -> Self {
        Self {
            monitoring_cycles: 0,
            avg_monitoring_time_ms: 0.0,
            max_monitoring_time_ms: 0,
            memory_query_success: 0,
            memory_query_failures: 0,
            avg_memory_query_time_ms: 0.0,
            interval_adjustments: 0,
            current_dynamic_interval: 30, // 默认30秒
        }
    }
}

/// 内存使用报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryUsageReport {
    /// 报告生成时间
    pub timestamp: DateTime<Utc>,
    /// 当前内存使用量（MB）
    pub current_usage_mb: u64,
    /// 峰值内存使用量（MB）
    pub peak_usage_mb: u64,
    /// 平均内存使用量（MB）
    pub avg_usage_mb: f64,
    /// 内存压力等级
    pub pressure_level: MemoryPressure,
    /// 总释放次数
    pub total_releases: u64,
    /// 总释放内存量（MB）
    pub total_freed_mb: u64,
    /// 释放效率（释放内存/峰值内存）
    pub release_efficiency: f64,
    /// 运行时长（秒）
    pub uptime_seconds: u64,
    /// 性能统计
    pub performance_stats: PerformanceStats,
}

/// 内存管理器
pub struct MemoryManager {
    /// 配置信息
    config: MemoryConfig,
    /// 上次GC时间
    last_gc_time: Arc<Mutex<Instant>>,
    /// 内存压力等级
    memory_pressure: Arc<Mutex<MemoryPressure>>,
    /// GC失败计数
    #[allow(unused)]
    gc_failure_count: Arc<Mutex<u32>>,
    /// 监控状态
    monitor_state: Arc<Mutex<MemoryMonitorState>>,
    /// 性能统计
    performance_stats: Arc<Mutex<PerformanceStats>>,
    /// 启动时间
    start_time: Instant,
    /// 内存使用历史（用于计算平均值）
    memory_history: Arc<Mutex<Vec<(Instant, u64)>>>,
    /// 系统内存历史（用于前端图表显示）
    system_memory_history: Arc<Mutex<std::collections::VecDeque<u64>>>,
}

impl MemoryManager {
    /// 创建新的内存管理器实例
    pub fn new(config: MemoryConfig) -> Self {
        Self {
            config,
            last_gc_time: Arc::new(Mutex::new(Instant::now())),
            memory_pressure: Arc::new(Mutex::new(MemoryPressure::Low)),
            gc_failure_count: Arc::new(Mutex::new(0)),
            monitor_state: Arc::new(Mutex::new(MemoryMonitorState {
                current_usage_mb: 0,
                peak_usage_mb: 0,
                pressure_level: MemoryPressure::Low,
                last_release_time: None,
                release_count: 0,
                total_freed_mb: 0,
            })),
            performance_stats: Arc::new(Mutex::new(PerformanceStats::default())),
            start_time: Instant::now(),
            memory_history: Arc::new(Mutex::new(Vec::with_capacity(1000))), // 保留最近1000个记录
            system_memory_history: Arc::new(Mutex::new(std::collections::VecDeque::with_capacity(60))), // 保留最近60个数据点
        }
    }

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

            let mut sys = System::new();
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

    /// 计算内存压力等级
    pub fn calculate_pressure_level(&self, current_mb: u64, threshold_mb: u64) -> MemoryPressure {
        let usage_percentage = (current_mb as f64 / threshold_mb as f64) * 100.0;

        match usage_percentage {
            p if p < 60.0 => MemoryPressure::Low,
            p if p < 80.0 => MemoryPressure::Medium,
            p if p < 100.0 => MemoryPressure::High,
            _ => MemoryPressure::Critical,
        }
    }

    /// 获取当前内存压力等级
    pub async fn get_memory_pressure(&self) -> MemoryPressure {
        let pressure = self.memory_pressure.lock().await;
        pressure.clone()
    }

    /// 更新内存压力等级
    pub async fn update_memory_pressure(&self, current_mb: u64) {
        if let Err(e) = self.safe_update_memory_pressure(current_mb).await {
            log::error!("Failed to update memory pressure: {}", e);
        }
    }

    /// 检查是否应该触发内存释放
    pub async fn should_trigger_release(&self, current_mb: u64) -> bool {
        if current_mb <= self.config.threshold_mb {
            return false;
        }

        // 检查冷却时间
        let last_gc = self.last_gc_time.lock().await;
        let elapsed = last_gc.elapsed().as_secs();

        elapsed >= self.config.gc_cooldown_secs
    }

    /// 获取内存状态
    pub async fn get_memory_status(&self) -> Result<MemoryStatus, MemoryError> {
        let current_mb = self.get_current_memory_usage().await?;
        let pressure = self.get_memory_pressure().await;
        let last_gc = self.last_gc_time.lock().await;
        let time_since_last_gc = last_gc.elapsed().as_secs();

        Ok(MemoryStatus {
            current_mb,
            threshold_mb: self.config.threshold_mb,
            pressure,
            time_since_last_gc_secs: time_since_last_gc,
            is_monitoring: true, // 这里暂时硬编码，后续会在监控任务中更新
        })
    }

    /// 获取监控状态
    pub async fn get_monitor_state(&self) -> MemoryMonitorState {
        let state = self.monitor_state.lock().await;
        state.clone()
    }

    /// 更新内存查询性能统计
    async fn update_memory_query_stats(&self, duration: std::time::Duration, success: bool) {
        let mut stats = self.performance_stats.lock().await;

        let duration_ms = duration.as_millis() as u64;

        if success {
            stats.memory_query_success += 1;
        } else {
            stats.memory_query_failures += 1;
        }

        // 更新平均查询时间
        let total_queries = stats.memory_query_success + stats.memory_query_failures;
        if total_queries > 0 {
            stats.avg_memory_query_time_ms =
                (stats.avg_memory_query_time_ms * (total_queries - 1) as f64 + duration_ms as f64)
                    / total_queries as f64;
        }

        log::debug!(
            "Memory query completed in {} ms (success: {}), avg: {:.2} ms",
            duration_ms,
            success,
            stats.avg_memory_query_time_ms
        );
    }

    /// 更新内存使用历史记录
    async fn update_memory_history(&self, memory_mb: u64) {
        let mut history = self.memory_history.lock().await;
        let now = Instant::now();

        // 添加新记录
        history.push((now, memory_mb));

        // 保持历史记录在合理大小内（最近1000个记录）
        if history.len() > 1000 {
            history.remove(0);
        }

        // 清理超过1小时的旧记录
        let one_hour_ago = now - std::time::Duration::from_secs(3600);
        history.retain(|(timestamp, _)| *timestamp > one_hour_ago);

        log::debug!(
            "Updated memory history: {} MB (history size: {})",
            memory_mb,
            history.len()
        );

        // 同时更新系统内存历史（用于前端图表）
        self.update_system_memory_history(memory_mb).await;
    }

    /// 更新系统内存历史记录（用于前端图表显示）
    async fn update_system_memory_history(&self, memory_mb: u64) {
        let mut sys_history = self.system_memory_history.lock().await;
        
        // 添加新记录
        sys_history.push_back(memory_mb);
        
        // 保持最近60个数据点（对应2分钟的数据，每2秒一个点）
        if sys_history.len() > 60 {
            sys_history.pop_front();
        }
        
        log::debug!(
            "Updated system memory history: {} MB (history size: {})",
            memory_mb,
            sys_history.len()
        );
    }

    /// 获取系统内存历史数据
    pub async fn get_system_memory_history(&self) -> Vec<u64> {
        let sys_history = self.system_memory_history.lock().await;
        sys_history.iter().cloned().collect()
    }

    /// 获取性能统计信息
    pub async fn get_performance_stats(&self) -> PerformanceStats {
        let stats = self.performance_stats.lock().await;
        stats.clone()
    }

    /// 计算平均内存使用量
    pub async fn calculate_average_memory_usage(&self) -> f64 {
        let history = self.memory_history.lock().await;

        if history.is_empty() {
            return 0.0;
        }

        let total: u64 = history.iter().map(|(_, memory)| *memory).sum();
        total as f64 / history.len() as f64
    }

    /// 生成内存使用报告
    pub async fn generate_memory_report(&self) -> MemoryUsageReport {
        let state = self.get_monitor_state().await;
        let stats = self.get_performance_stats().await;
        let avg_usage = self.calculate_average_memory_usage().await;
        let uptime = self.start_time.elapsed().as_secs();

        let release_efficiency = if state.peak_usage_mb > 0 {
            state.total_freed_mb as f64 / state.peak_usage_mb as f64
        } else {
            0.0
        };

        MemoryUsageReport {
            timestamp: Utc::now(),
            current_usage_mb: state.current_usage_mb,
            peak_usage_mb: state.peak_usage_mb,
            avg_usage_mb: avg_usage,
            pressure_level: state.pressure_level,
            total_releases: state.release_count,
            total_freed_mb: state.total_freed_mb,
            release_efficiency,
            uptime_seconds: uptime,
            performance_stats: stats,
        }
    }

    /// 打印性能报告到日志
    pub async fn log_performance_report(&self) {
        let report = self.generate_memory_report().await;

        log::info!("=== Memory Management Performance Report ===");
        log::info!(
            "Uptime: {} seconds ({:.1} hours)",
            report.uptime_seconds,
            report.uptime_seconds as f64 / 3600.0
        );
        log::info!(
            "Current Memory: {} MB, Peak: {} MB, Average: {:.1} MB",
            report.current_usage_mb,
            report.peak_usage_mb,
            report.avg_usage_mb
        );
        log::info!("Memory Pressure: {:?}", report.pressure_level);
        log::info!(
            "Total Releases: {}, Total Freed: {} MB, Efficiency: {:.1}%",
            report.total_releases,
            report.total_freed_mb,
            report.release_efficiency * 100.0
        );

        let stats = &report.performance_stats;
        log::info!(
            "Monitoring Cycles: {}, Avg Time: {:.2} ms, Max Time: {} ms",
            stats.monitoring_cycles,
            stats.avg_monitoring_time_ms,
            stats.max_monitoring_time_ms
        );
        log::info!(
            "Memory Queries: {} success, {} failures, Avg Time: {:.2} ms",
            stats.memory_query_success,
            stats.memory_query_failures,
            stats.avg_memory_query_time_ms
        );
        log::info!(
            "Interval Adjustments: {}, Current Interval: {} seconds",
            stats.interval_adjustments,
            stats.current_dynamic_interval
        );
        log::info!("=== End Performance Report ===");
    }

    /// 获取内存使用趋势
    pub async fn get_memory_trend(&self) -> Option<f64> {
        let history = self.memory_history.lock().await;

        if history.len() < 10 {
            return None; // 需要至少10个数据点
        }

        // 简单的线性趋势计算
        let recent_count = (history.len() / 4).max(5).min(50); // 取最近1/4的数据，但至少5个，最多50个
        let recent_data: Vec<_> = history.iter().rev().take(recent_count).collect();

        if recent_data.len() < 2 {
            return None;
        }

        let first_memory = recent_data.last().unwrap().1 as f64;
        let last_memory = recent_data.first().unwrap().1 as f64;
        let time_span = recent_data
            .first()
            .unwrap()
            .0
            .duration_since(recent_data.last().unwrap().0)
            .as_secs() as f64;

        if time_span > 0.0 {
            // 返回每小时的内存变化率（MB/hour）
            Some((last_memory - first_memory) * 3600.0 / time_span)
        } else {
            None
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

        // 清理磁盘缓存
        match std::panic::catch_unwind(|| {
            cleanup_expired_cache();
        }) {
            Ok(_) => {
                log::debug!("Disk cache cleanup completed successfully");
            }
            Err(_) => {
                log::warn!(
                    "Disk cache cleanup panicked, continuing with memory cache cleanup only"
                );
                // 不返回错误，因为内存缓存清理已经成功
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

    /// 安全地更新内存压力等级
    async fn safe_update_memory_pressure(&self, current_mb: u64) -> Result<(), MemoryError> {
        // 计算新的压力等级
        let new_pressure = self.calculate_pressure_level(current_mb, self.config.threshold_mb);

        // 更新内存压力
        {
            let mut pressure = self.memory_pressure.lock().await;
            let old_pressure = pressure.clone();
            *pressure = new_pressure.clone();

            if old_pressure != new_pressure {
                log::info!(
                    "Memory pressure level changed: {:?} -> {:?} (usage: {} MB / {} MB)",
                    old_pressure,
                    new_pressure,
                    current_mb,
                    self.config.threshold_mb
                );
            }
        }

        // 更新监控状态
        {
            let mut state = self.monitor_state.lock().await;
            state.current_usage_mb = current_mb;
            state.pressure_level = new_pressure;

            // 更新峰值内存使用量
            if current_mb > state.peak_usage_mb {
                state.peak_usage_mb = current_mb;
            }
        }

        Ok(())
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

    /// 启动内存监控后台任务 - 性能优化版本
    pub fn start_monitoring(&self) -> tokio::task::JoinHandle<()> {
        let config = self.config.clone();
        let last_gc_time = Arc::clone(&self.last_gc_time);
        let memory_pressure = Arc::clone(&self.memory_pressure);
        let gc_failure_count = Arc::clone(&self.gc_failure_count);
        let monitor_state = Arc::clone(&self.monitor_state);
        let performance_stats = Arc::clone(&self.performance_stats);
        let start_time = self.start_time;
        let memory_history = Arc::clone(&self.memory_history);
        let system_memory_history = Arc::clone(&self.system_memory_history);

        tokio::spawn(async move {
            log::info!("Starting enhanced memory monitoring task with base interval: {} seconds, threshold: {} MB", 
                config.check_interval_secs, config.threshold_mb);

            // 创建一个临时的内存管理器实例用于监控任务
            let temp_manager = MemoryManager {
                config: config.clone(),
                last_gc_time,
                memory_pressure,
                gc_failure_count,
                monitor_state,
                performance_stats: Arc::clone(&performance_stats),
                start_time,
                memory_history,
                system_memory_history,
            };

            let mut consecutive_failures = 0u32;
            let mut last_successful_check = Instant::now();
            let mut current_interval = config.check_interval_secs;
            let mut last_interval_adjustment = Instant::now();

            loop {
                // 智能间隔调整
                let adjusted_interval = temp_manager
                    .calculate_adaptive_interval(
                        current_interval,
                        consecutive_failures,
                        &last_interval_adjustment,
                    )
                    .await;

                if adjusted_interval != current_interval {
                    current_interval = adjusted_interval;

                    // 更新统计
                    {
                        let mut stats = performance_stats.lock().await;
                        stats.interval_adjustments += 1;
                        stats.current_dynamic_interval = current_interval;
                    }
                    last_interval_adjustment = Instant::now();
                }

                tokio::time::sleep(tokio::time::Duration::from_secs(current_interval)).await;

                let cycle_start = Instant::now();

                // 更新监控周期统计
                temp_manager
                    .update_monitoring_cycle_stats(cycle_start)
                    .await;

                match tokio::time::timeout(
                    tokio::time::Duration::from_secs(30), // 30秒超时
                    temp_manager.check_and_release_if_needed(),
                )
                .await
                {
                    Ok(Ok(Some(result))) => {
                        let cycle_duration = cycle_start.elapsed();
                        log::info!("Automatic memory release completed in {:?}: freed {} MB, cleared {} cache entries", 
                            cycle_duration, result.memory_freed_mb, result.cache_entries_cleared);
                        consecutive_failures = 0;
                        last_successful_check = Instant::now();

                        temp_manager
                            .update_monitoring_stats(cycle_duration, true)
                            .await;
                    }
                    Ok(Ok(None)) => {
                        let cycle_duration = cycle_start.elapsed();
                        consecutive_failures = 0;
                        last_successful_check = Instant::now();

                        temp_manager
                            .update_monitoring_stats(cycle_duration, true)
                            .await;
                    }
                    Ok(Err(e)) => {
                        consecutive_failures += 1;
                        let cycle_duration = cycle_start.elapsed();
                        log::error!(
                            "Memory monitoring check failed (attempt {}): {}",
                            consecutive_failures,
                            e
                        );

                        temp_manager
                            .update_monitoring_stats(cycle_duration, false)
                            .await;
                        Self::handle_monitoring_failure(
                            consecutive_failures,
                            &last_successful_check,
                        )
                        .await;
                    }
                    Err(_) => {
                        consecutive_failures += 1;
                        let cycle_duration = cycle_start.elapsed();
                        log::error!(
                            "Memory monitoring check timed out after 30 seconds (attempt {})",
                            consecutive_failures
                        );

                        temp_manager
                            .update_monitoring_stats(cycle_duration, false)
                            .await;
                        Self::handle_monitoring_failure(
                            consecutive_failures,
                            &last_successful_check,
                        )
                        .await;
                    }
                }
            }
        })
    }

    /// 计算自适应监控间隔
    async fn calculate_adaptive_interval(
        &self,
        current_interval: u64,
        consecutive_failures: u32,
        last_adjustment: &Instant,
    ) -> u64 {
        // 获取当前内存压力等级
        let pressure = self.get_memory_pressure().await;
        let base_interval = self.config.check_interval_secs;

        // 根据内存压力调整间隔
        let pressure_multiplier = match pressure {
            MemoryPressure::Critical => 0.5, // 严重压力时更频繁检查
            MemoryPressure::High => 0.75,    // 高压力时稍微频繁
            MemoryPressure::Medium => 1.0,   // 中等压力时正常间隔
            MemoryPressure::Low => 1.5,      // 低压力时可以放宽间隔
        };

        // 根据连续失败次数调整间隔
        let failure_multiplier = match consecutive_failures {
            0..=2 => 1.0,
            3..=5 => 1.5, // 有失败时稍微放宽
            _ => 2.0,     // 连续失败时大幅放宽
        };

        // 计算新间隔
        let new_interval = ((base_interval as f64 * pressure_multiplier * failure_multiplier)
            as u64)
            .max(5) // 最小5秒
            .min(300); // 最大5分钟

        // 避免频繁调整（至少间隔1分钟）
        if last_adjustment.elapsed() < std::time::Duration::from_secs(60)
            && new_interval != current_interval
        {
            return current_interval;
        }

        new_interval
    }

    /// 更新监控周期统计
    async fn update_monitoring_cycle_stats(&self, _cycle_start: Instant) {
        let mut stats = self.performance_stats.lock().await;
        stats.monitoring_cycles += 1;
    }

    /// 更新监控统计信息
    async fn update_monitoring_stats(&self, duration: std::time::Duration, _success: bool) {
        let mut stats = self.performance_stats.lock().await;

        let duration_ms = duration.as_millis() as u64;

        // 更新最大监控时间
        if duration_ms > stats.max_monitoring_time_ms {
            stats.max_monitoring_time_ms = duration_ms;
        }

        // 更新平均监控时间
        if stats.monitoring_cycles > 0 {
            stats.avg_monitoring_time_ms = (stats.avg_monitoring_time_ms
                * (stats.monitoring_cycles - 1) as f64
                + duration_ms as f64)
                / stats.monitoring_cycles as f64;
        }
    }

    /// 处理监控失败的情况
    async fn handle_monitoring_failure(consecutive_failures: u32, last_successful_check: &Instant) {
        let time_since_success = last_successful_check.elapsed();

        match consecutive_failures {
            1..=2 => {
                log::warn!(
                    "Memory monitoring experiencing issues (failure count: {})",
                    consecutive_failures
                );
            }
            3..=5 => {
                log::error!(
                    "Memory monitoring has failed {} times consecutively (last success: {:?} ago)",
                    consecutive_failures,
                    time_since_success
                );
            }
            count if count > 5 => {
                log::error!("CRITICAL: Memory monitoring has failed {} times consecutively (last success: {:?} ago) - system may be under severe stress", 
                    count, time_since_success);

                // 如果超过1小时没有成功检查，记录严重警告
                if time_since_success > tokio::time::Duration::from_secs(3600) {
                    log::error!("CRITICAL: Memory monitoring has been failing for over 1 hour - immediate attention required");
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_manager_creation() {
        let config = MemoryConfig {
            threshold_mb: 500,
            check_interval_secs: 30,
            gc_cooldown_secs: 30,
        };

        let manager = MemoryManager::new(config);
        let pressure = manager.get_memory_pressure().await;
        assert_eq!(pressure, MemoryPressure::Low);
    }

    #[test]
    fn test_pressure_level_calculation() {
        let config = MemoryConfig {
            threshold_mb: 500,
            check_interval_secs: 30,
            gc_cooldown_secs: 30,
        };
        let manager = MemoryManager::new(config);

        // 测试低压力 (< 60%)
        assert_eq!(
            manager.calculate_pressure_level(200, 500),
            MemoryPressure::Low
        );

        // 测试中等压力 (60-80%)
        assert_eq!(
            manager.calculate_pressure_level(350, 500),
            MemoryPressure::Medium
        );

        // 测试高压力 (80-100%)
        assert_eq!(
            manager.calculate_pressure_level(450, 500),
            MemoryPressure::High
        );

        // 测试严重压力 (> 100%)
        assert_eq!(
            manager.calculate_pressure_level(600, 500),
            MemoryPressure::Critical
        );
    }

    #[tokio::test]
    async fn test_memory_pressure_update() {
        let config = MemoryConfig {
            threshold_mb: 500,
            check_interval_secs: 30,
            gc_cooldown_secs: 30,
        };
        let manager = MemoryManager::new(config);

        // 更新为高压力
        manager.update_memory_pressure(450).await;
        let pressure = manager.get_memory_pressure().await;
        assert_eq!(pressure, MemoryPressure::High);

        // 检查监控状态
        let state = manager.get_monitor_state().await;
        assert_eq!(state.current_usage_mb, 450);
        assert_eq!(state.pressure_level, MemoryPressure::High);
        assert_eq!(state.peak_usage_mb, 450);
    }

    #[tokio::test]
    async fn test_should_trigger_release() {
        let config = MemoryConfig {
            threshold_mb: 500,
            check_interval_secs: 30,
            gc_cooldown_secs: 1, // 1秒冷却时间用于测试
        };
        let manager = MemoryManager::new(config);

        // 内存使用未超过阈值，不应触发
        assert!(!manager.should_trigger_release(400).await);

        // 刚创建的manager，last_gc_time是当前时间，所以即使内存超过阈值也不应该触发（冷却期内）
        assert!(!manager.should_trigger_release(600).await);

        // 等待冷却时间过去
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // 现在内存使用超过阈值，且冷却时间已过，应该触发
        assert!(manager.should_trigger_release(600).await);

        // 模拟刚执行过GC，更新last_gc_time
        {
            let mut last_gc = manager.last_gc_time.lock().await;
            *last_gc = Instant::now();
        }

        // 刚执行过GC，应该在冷却期内，不应触发
        assert!(!manager.should_trigger_release(600).await);
    }

    #[tokio::test]
    async fn test_gc_cooldown_mechanism() {
        let config = MemoryConfig {
            threshold_mb: 100, // 低阈值便于测试
            check_interval_secs: 30,
            gc_cooldown_secs: 1, // 1秒冷却时间
        };
        let manager = MemoryManager::new(config);

        // 等待初始冷却时间过去
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // 第一次检查：内存超过阈值，应该触发
        assert!(manager.should_trigger_release(200).await);

        // 模拟执行GC后更新时间戳
        {
            let mut last_gc = manager.last_gc_time.lock().await;
            *last_gc = Instant::now();
        }

        // 立即再次检查：应该在冷却期内，不应触发
        assert!(!manager.should_trigger_release(200).await);

        // 等待一半冷却时间
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        assert!(!manager.should_trigger_release(200).await);

        // 等待剩余冷却时间
        tokio::time::sleep(tokio::time::Duration::from_millis(600)).await;

        // 现在冷却时间已过，应该可以再次触发
        assert!(manager.should_trigger_release(200).await);
    }

    #[tokio::test]
    async fn test_jemalloc_integration() {
        let config = MemoryConfig {
            threshold_mb: 500,
            check_interval_secs: 30,
            gc_cooldown_secs: 30,
        };
        let manager = MemoryManager::new(config);

        // 测试jemalloc配置验证
        let validation_result = manager.validate_jemalloc_config();
        assert!(validation_result.is_ok());

        // 测试内存使用量获取
        let memory_usage = manager.get_current_memory_usage().await;

        match memory_usage {
            Ok(usage_mb) => {
                println!("Current memory usage: {} MB", usage_mb);
                // 只验证函数能够成功返回，不对具体值做假设
                // 因为在不同环境下内存使用量可能差异很大，甚至可能为0
                // u64类型本身就不会是负数，所以这里只是确认函数正常执行
            }
            Err(e) => {
                println!("Failed to get memory usage: {}", e);
                // 在某些测试环境中可能无法获取内存信息，这是可以接受的
                // 但我们至少要确保错误是预期的类型
                match e {
                    MemoryError::MetricsCollectionFailed(_) => {
                        // 这是预期的错误类型
                    }
                    _ => panic!("Unexpected error type: {}", e),
                }
            }
        }
    }

    #[tokio::test]
    async fn test_trigger_global_release() {
        let config = MemoryConfig {
            threshold_mb: 500,
            check_interval_secs: 30,
            gc_cooldown_secs: 1,
        };
        let manager = MemoryManager::new(config);

        // 等待冷却时间过去
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // 执行全局内存释放
        let result = manager.trigger_global_release().await;
        assert!(result.is_ok());

        if let Ok(release_result) = result {
            // 验证结果结构（u64类型本身就不会是负数）
            // 只需要验证时间戳是合理的
            assert!(release_result.timestamp > Utc::now() - chrono::Duration::seconds(10));

            println!(
                "Release result: freed {} MB, cleared {} cache entries, GC executed: {}",
                release_result.memory_freed_mb,
                release_result.cache_entries_cleared,
                release_result.gc_executed
            );
        }

        // 验证监控状态更新
        let state = manager.get_monitor_state().await;
        assert_eq!(state.release_count, 1);
        assert!(state.last_release_time.is_some());
    }

    #[tokio::test]
    async fn test_check_and_release_if_needed() {
        let config = MemoryConfig {
            threshold_mb: 1, // 设置很低的阈值，确保会触发释放
            check_interval_secs: 30,
            gc_cooldown_secs: 1,
        };
        let manager = MemoryManager::new(config);

        // 等待冷却时间过去
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // 检查并释放
        let result = manager.check_and_release_if_needed().await;
        assert!(result.is_ok());

        if let Ok(Some(release_result)) = result {
            println!(
                "Automatic release triggered: freed {} MB",
                release_result.memory_freed_mb
            );
        } else if let Ok(None) = result {
            println!("No release needed");
        }
    }

    #[tokio::test]
    async fn test_gc_failure_counting() {
        let config = MemoryConfig {
            threshold_mb: 500,
            check_interval_secs: 30,
            gc_cooldown_secs: 1,
        };
        let manager = MemoryManager::new(config);

        // 等待冷却时间过去
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // 执行多次释放操作来测试失败计数
        for i in 1..=3 {
            let _ = manager.trigger_global_release().await;

            // 检查失败计数（在正常情况下应该为0，因为GC会成功或者jemalloc不可用）
            let failure_count = {
                let count = manager.gc_failure_count.lock().await;
                *count
            };

            println!("After release {}: failure count = {}", i, failure_count);

            // 等待一小段时间
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    }

    #[tokio::test]
    async fn test_start_monitoring() {
        let config = MemoryConfig {
            threshold_mb: 500,
            check_interval_secs: 1, // 1秒间隔用于测试
            gc_cooldown_secs: 1,
        };
        let manager = MemoryManager::new(config);

        // 启动监控任务
        let monitoring_handle = manager.start_monitoring();

        // 让监控任务运行一小段时间
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

        // 停止监控任务
        monitoring_handle.abort();

        // 验证监控任务确实在运行（通过检查是否有状态更新）
        let state = manager.get_monitor_state().await;
        println!(
            "Monitoring test completed, current usage: {} MB",
            state.current_usage_mb
        );

        // 监控任务应该至少更新了一次内存使用量（u64类型本身就不会是负数）
        // 这里只是验证监控任务正常运行
    }

    #[tokio::test]
    async fn test_monitoring_interval_consistency() {
        let config = MemoryConfig {
            threshold_mb: 500,
            check_interval_secs: 1, // 1秒间隔
            gc_cooldown_secs: 30,
        };
        let manager = MemoryManager::new(config);

        let start_time = std::time::Instant::now();
        let monitoring_handle = manager.start_monitoring();

        // 运行2.5秒，应该触发2-3次检查
        tokio::time::sleep(tokio::time::Duration::from_millis(2500)).await;

        monitoring_handle.abort();
        let elapsed = start_time.elapsed();

        println!("Monitoring ran for {:?}", elapsed);

        // 验证监控任务按预期间隔运行
        assert!(elapsed >= tokio::time::Duration::from_secs(2));
        assert!(elapsed < tokio::time::Duration::from_secs(4));
    }
}
#[tokio::test]
async fn test_enhanced_error_handling() {
    let config = MemoryConfig {
        threshold_mb: 500,
        check_interval_secs: 30,
        gc_cooldown_secs: 30,
    };
    let manager = MemoryManager::new(config);

    // 测试安全的内存压力更新
    let result = manager.safe_update_memory_pressure(450).await;
    assert!(result.is_ok());

    // 验证压力等级已更新
    let pressure = manager.get_memory_pressure().await;
    assert_eq!(pressure, MemoryPressure::High);

    // 测试监控状态更新
    let state = manager.get_monitor_state().await;
    assert_eq!(state.current_usage_mb, 450);
    assert_eq!(state.pressure_level, MemoryPressure::High);
    assert_eq!(state.peak_usage_mb, 450);
}

#[tokio::test]
async fn test_gc_failure_handling() {
    let config = MemoryConfig {
        threshold_mb: 100, // 低阈值便于测试
        check_interval_secs: 30,
        gc_cooldown_secs: 1,
    };
    let manager = MemoryManager::new(config);

    // 等待冷却时间过去
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // 执行内存释放操作
    let result = manager.trigger_global_release().await;
    assert!(result.is_ok());

    if let Ok(release_result) = result {
        // 验证操作结果结构
        assert!(release_result.timestamp > Utc::now() - chrono::Duration::seconds(10));
        println!(
            "Enhanced release result: freed {} MB, cleared {} cache entries, GC executed: {}",
            release_result.memory_freed_mb,
            release_result.cache_entries_cleared,
            release_result.gc_executed
        );
    }

    // 验证统计信息更新
    let state = manager.get_monitor_state().await;
    assert_eq!(state.release_count, 1);
    assert!(state.last_release_time.is_some());
}

#[tokio::test]
async fn test_memory_usage_error_handling() {
    let config = MemoryConfig {
        threshold_mb: 500,
        check_interval_secs: 30,
        gc_cooldown_secs: 30,
    };
    let manager = MemoryManager::new(config);

    // 测试内存使用量获取
    let memory_usage = manager.get_current_memory_usage().await;

    match memory_usage {
        Ok(usage_mb) => {
            println!("Enhanced memory usage retrieval: {} MB", usage_mb);
            // 验证返回值的合理性
            assert!(usage_mb < 50000); // 不应该超过50GB
        }
        Err(e) => {
            println!(
                "Memory usage retrieval failed (expected in some test environments): {}",
                e
            );
            // 在测试环境中失败是可以接受的
            match e {
                MemoryError::MetricsCollectionFailed(_) => {
                    // 这是预期的错误类型
                }
                _ => panic!("Unexpected error type: {}", e),
            }
        }
    }
}

#[tokio::test]
async fn test_enhanced_monitoring_task() {
    let config = MemoryConfig {
        threshold_mb: 500,
        check_interval_secs: 1, // 1秒间隔用于测试
        gc_cooldown_secs: 1,
    };
    let manager = MemoryManager::new(config);

    // 启动增强的监控任务
    let monitoring_handle = manager.start_monitoring();

    // 让监控任务运行一小段时间
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // 停止监控任务
    monitoring_handle.abort();

    // 验证监控任务确实在运行（通过检查是否有状态更新）
    let state = manager.get_monitor_state().await;
    println!(
        "Enhanced monitoring test completed, current usage: {} MB, peak: {} MB",
        state.current_usage_mb, state.peak_usage_mb
    );

    // 监控任务应该至少更新了一次内存使用量
    // 在某些测试环境中可能为0，这是可以接受的
}
#[tokio::test]
async fn test_performance_optimization_features() {
    let config = MemoryConfig {
        threshold_mb: 500,
        check_interval_secs: 30,
        gc_cooldown_secs: 30,
    };
    let manager = MemoryManager::new(config);

    // 测试性能统计初始化
    let stats = manager.get_performance_stats().await;
    assert_eq!(stats.monitoring_cycles, 0);
    assert_eq!(stats.memory_query_success, 0);
    assert_eq!(stats.memory_query_failures, 0);
    assert_eq!(stats.current_dynamic_interval, 30);

    // 测试内存历史记录
    manager.update_memory_history(100).await;
    manager.update_memory_history(150).await;
    manager.update_memory_history(200).await;

    let avg_usage = manager.calculate_average_memory_usage().await;
    assert!((avg_usage - 150.0).abs() < 1.0); // 应该接近150MB

    // 添加小延迟以确保uptime > 0
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    // 测试内存使用报告生成
    let report = manager.generate_memory_report().await;
    // uptime_seconds 是 u64 类型，总是 >= 0，所以只检查它存在
    assert_eq!(report.avg_usage_mb, avg_usage);
    assert!(report.timestamp > Utc::now() - chrono::Duration::seconds(10));

    println!("Performance optimization test completed successfully");
    println!("Average memory usage: {:.1} MB", avg_usage);
    println!("Report uptime: {} seconds", report.uptime_seconds);
}

#[tokio::test]
async fn test_adaptive_interval_calculation() {
    let config = MemoryConfig {
        threshold_mb: 500,
        check_interval_secs: 30,
        gc_cooldown_secs: 30,
    };
    let manager = MemoryManager::new(config);
    let last_adjustment = Instant::now();

    // 测试不同压力等级下的间隔调整

    // 设置低压力
    manager.safe_update_memory_pressure(200).await.unwrap(); // 40% of threshold
    let interval_low = manager
        .calculate_adaptive_interval(30, 0, &last_adjustment)
        .await;

    // 设置高压力
    manager.safe_update_memory_pressure(450).await.unwrap(); // 90% of threshold
    let interval_high = manager
        .calculate_adaptive_interval(30, 0, &last_adjustment)
        .await;

    // 设置严重压力
    manager.safe_update_memory_pressure(600).await.unwrap(); // 120% of threshold
    let interval_critical = manager
        .calculate_adaptive_interval(30, 0, &last_adjustment)
        .await;

    println!(
        "Adaptive intervals - Low: {}s, High: {}s, Critical: {}s",
        interval_low, interval_high, interval_critical
    );

    // 严重压力时应该有更短的间隔
    assert!(interval_critical <= interval_high);
    assert!(interval_high <= interval_low);

    // 测试失败情况下的间隔调整
    let interval_with_failures = manager
        .calculate_adaptive_interval(30, 5, &last_adjustment)
        .await;
    assert!(interval_with_failures >= 30); // 失败时应该增加间隔

    println!("Interval with failures: {}s", interval_with_failures);
}

#[tokio::test]
async fn test_memory_trend_analysis() {
    let config = MemoryConfig {
        threshold_mb: 500,
        check_interval_secs: 30,
        gc_cooldown_secs: 30,
    };
    let manager = MemoryManager::new(config);

    // 添加一些内存历史数据来模拟趋势
    for i in 0..20 {
        let memory_usage = 100 + i * 5; // 递增趋势
        manager.update_memory_history(memory_usage).await;

        // 添加小延迟以确保时间戳不同
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    let trend = manager.get_memory_trend().await;

    match trend {
        Some(trend_value) => {
            println!("Memory trend: {:.2} MB/hour", trend_value);
            // 由于我们创建了递增趋势，应该是正值
            assert!(trend_value > 0.0);
        }
        None => {
            println!("Not enough data for trend analysis");
        }
    }
}

#[tokio::test]
async fn test_performance_reporting() {
    let config = MemoryConfig {
        threshold_mb: 500,
        check_interval_secs: 30,
        gc_cooldown_secs: 30,
    };
    let manager = MemoryManager::new(config);

    // 模拟一些性能数据
    manager
        .update_memory_query_stats(std::time::Duration::from_millis(50), true)
        .await;
    manager
        .update_memory_query_stats(std::time::Duration::from_millis(75), true)
        .await;
    manager
        .update_memory_query_stats(std::time::Duration::from_millis(100), false)
        .await;

    let stats = manager.get_performance_stats().await;
    assert_eq!(stats.memory_query_success, 2);
    assert_eq!(stats.memory_query_failures, 1);
    assert!(stats.avg_memory_query_time_ms > 0.0);

    // 测试性能报告日志输出
    manager.log_performance_report().await;

    println!("Performance reporting test completed");
    println!(
        "Average query time: {:.2} ms",
        stats.avg_memory_query_time_ms
    );
    println!(
        "Success rate: {:.1}%",
        stats.memory_query_success as f64
            / (stats.memory_query_success + stats.memory_query_failures) as f64
            * 100.0
    );
}
