use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Instant;
use thiserror::Error;

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
