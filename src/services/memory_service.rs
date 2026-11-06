use crate::config::settings::MemoryConfig;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

mod measurement;
mod models;
mod monitoring;
mod release;
mod reporting;

pub use models::{
    MemoryError, MemoryMonitorState, MemoryPressure, MemoryStatus, MemoryUsageReport, PerformanceStats,
    ReleaseResult,
};

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
    /// 内存使用历史（用于计算平均值，使用 VecDeque 以便高效移除旧数据）
    memory_history: Arc<Mutex<std::collections::VecDeque<(Instant, u64)>>>,
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
            memory_history: Arc::new(Mutex::new(std::collections::VecDeque::with_capacity(1000))), // 保留最近1000个记录
            system_memory_history: Arc::new(Mutex::new(std::collections::VecDeque::with_capacity(60))), // 保留最近60个数据点
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

}

#[cfg(test)]
#[path = "../../tests/unit/memory_service.rs"]
mod tests;
