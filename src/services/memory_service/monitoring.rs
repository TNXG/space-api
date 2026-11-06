use super::{MemoryManager, MemoryPressure};
use std::sync::Arc;
use std::time::Instant;

impl MemoryManager {
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
    pub(super) async fn calculate_adaptive_interval(
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
