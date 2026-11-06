use super::{MemoryManager, MemoryUsageReport, PerformanceStats};
use chrono::Utc;
use std::time::Instant;

impl MemoryManager {
    /// 更新内存查询性能统计
    pub(super) async fn update_memory_query_stats(&self, duration: std::time::Duration, success: bool) {
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
    pub(super) async fn update_memory_history(&self, memory_mb: u64) {
        let mut history = self.memory_history.lock().await;
        let now = Instant::now();

        // 添加新记录
        history.push_back((now, memory_mb));

        // 保持历史记录在合理大小内（最近1000个记录，pop_front 是 O(1)）
        while history.len() > 1000 {
            history.pop_front();
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
}
