use super::{get_process_stats, MetricsHistory, SystemState};
use crate::services::memory_service::MemoryManager;
use chrono::Local;
use rocket::get;
use rocket::response::stream::{Event, EventStream};
use rocket::tokio::time::{interval, Duration};
use rocket::State;
use std::process;
use std::sync::Arc;
use sysinfo::{Pid, ProcessesToUpdate};

// API 端点用于实时更新数据
#[get("/api/metrics")]
pub async fn get_metrics(
    metrics: &State<MetricsHistory>,
    sys_state: &State<SystemState>,
    memory_manager: &State<Arc<MemoryManager>>,
) -> rocket::serde::json::Json<serde_json::Value> {
    let (proc_rss, proc_cpu_raw) = {
        let mut sys = sys_state.system.lock().unwrap_or_else(|e| e.into_inner());
        sys.refresh_memory();
        // 不需要refresh_cpu_all，因为我们只关心当前进程的CPU使用率
        
        let (proc_rss, _, proc_cpu_raw) = get_process_stats(&mut sys);
        (proc_rss, proc_cpu_raw)
    };
    // 进程CPU使用率已经是正确的百分比值
    let proc_cpu = proc_cpu_raw;

    let now = Local::now();
    let timestamp = now.format("%H:%M:%S").to_string();

    // 获取系统内存监控状态
    let system_memory_mb = match memory_manager.get_memory_status().await {
        Ok(status) => status.current_mb,
        Err(_) => 0,
    };

    // 更新历史
    {
        let mut cpu_hist = metrics.cpu_history.lock().unwrap_or_else(|e| e.into_inner());
        let mut mem_hist = metrics.mem_history.lock().unwrap_or_else(|e| e.into_inner());
        let mut sys_mem_hist = metrics.system_memory_history.lock().unwrap_or_else(|e| e.into_inner());
        let mut ts_hist = metrics.timestamps.lock().unwrap_or_else(|e| e.into_inner());

        if cpu_hist.len() >= 60 {
            cpu_hist.pop_front();
            mem_hist.pop_front();
            sys_mem_hist.pop_front();
            ts_hist.pop_front();
        }

        cpu_hist.push_back(proc_cpu);
        mem_hist.push_back(proc_rss);
        sys_mem_hist.push_back(system_memory_mb);
        ts_hist.push_back(timestamp.clone());
    }

    let (cpu_history, mem_history, system_memory_history, timestamps) = {
        let cpu_hist = metrics.cpu_history.lock().unwrap_or_else(|e| e.into_inner());
        let mem_hist = metrics.mem_history.lock().unwrap_or_else(|e| e.into_inner());
        let sys_mem_hist = metrics.system_memory_history.lock().unwrap_or_else(|e| e.into_inner());
        let ts_hist = metrics.timestamps.lock().unwrap_or_else(|e| e.into_inner());

        (
            cpu_hist.iter().cloned().collect::<Vec<_>>(),
            mem_hist
                .iter()
                .map(|&m| m as f64 / (1024.0 * 1024.0))
                .collect::<Vec<_>>(),
            sys_mem_hist.iter().cloned().collect::<Vec<_>>(),
            ts_hist.iter().cloned().collect::<Vec<_>>(),
        )
    };

    // 获取内存监控状态
    let memory_monitor_status = match memory_manager.get_memory_status().await {
        Ok(status) => Some(serde_json::json!({
            "current_memory_mb": status.current_mb,
            "threshold_mb": status.threshold_mb,
            "memory_pressure": match status.pressure {
                crate::services::memory_service::MemoryPressure::Low => "low",
                crate::services::memory_service::MemoryPressure::Medium => "medium",
                crate::services::memory_service::MemoryPressure::High => "high",
                crate::services::memory_service::MemoryPressure::Critical => "critical",
            },
            "memory_usage_percentage": (status.current_mb as f64 / status.threshold_mb as f64 * 100.0).round(),
            "time_since_last_gc_secs": status.time_since_last_gc_secs,
            "is_monitoring": status.is_monitoring,
        })),
        Err(e) => {
            log::warn!("Failed to get memory status for API: {}", e);
            None
        }
    };

    rocket::serde::json::Json(serde_json::json!({
        "cpu": proc_cpu,
        "mem_rss": proc_rss,
        "mem_rss_mb": proc_rss as f64 / (1024.0 * 1024.0),
        "timestamp": timestamp,
        "cpu_history": cpu_history,
        "mem_history": mem_history,
        "system_memory_history": system_memory_history,
        "timestamps": timestamps,
        "memory_monitor": memory_monitor_status,
    }))
}

#[get("/api/metrics/stream")]
pub fn metrics_stream(
    metrics: &State<MetricsHistory>,
    sys_state: &State<SystemState>,
    memory_manager: &State<Arc<MemoryManager>>,
) -> EventStream![] {
    let metrics = metrics.inner().clone();
    let sys_state = sys_state.inner().clone();
    let memory_manager = memory_manager.inner().clone();

    EventStream! {
        let mut timer = interval(Duration::from_secs(5)); // Push every 5 seconds (reduced frequency)

        loop {
            let _ = timer.tick().await;

            let (proc_rss, proc_virtual, proc_cpu_raw) = {
                // 将阻塞的 sysinfo 操作移到阻塞线程执行
                let sys_clone = sys_state.system.clone();
                tokio::task::spawn_blocking(move || {
                    let mut sys = sys_clone.lock().unwrap_or_else(|e| e.into_inner());
                    sys.refresh_memory();
                    let pid = Pid::from(process::id() as usize);
                    sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);

                    if let Some(proc) = sys.process(pid) {
                        (proc.memory(), proc.virtual_memory(), proc.cpu_usage())
                    } else {
                        (0, 0, 0.0)
                    }
                })
                .await
                .unwrap_or((0, 0, 0.0))
            };
            
            // 进程CPU使用率已经是正确的百分比值
            let proc_cpu = proc_cpu_raw;
            let now = Local::now();
            let timestamp = now.format("%H:%M:%S").to_string();

            // 获取系统内存监控状态
            let system_memory_mb = match memory_manager.get_memory_status().await {
                Ok(status) => status.current_mb,
                Err(_) => 0,
            };
            
            // Update History
            // To avoid double counting with basic API if both are used,
            // we might want to ONLY read here if get_metrics is deprecated.
            // But we will UPDATE here too to ensure history is live even if no one polls.
            // But wait, if 10 users stream, 10x updates.
            // For now, let's READ history and Current stats.
            // We'll update history ONLY if needed? 
            // Let's stick to updating history here too for now.
            // Actually, if we want to replace polling, this stream IS the updater.
            
            {
                let mut cpu_hist = metrics.cpu_history.lock().unwrap_or_else(|e| e.into_inner());
                let mut mem_hist = metrics.mem_history.lock().unwrap_or_else(|e| e.into_inner());
                let mut sys_mem_hist = metrics.system_memory_history.lock().unwrap_or_else(|e| e.into_inner());
                let mut ts_hist = metrics.timestamps.lock().unwrap_or_else(|e| e.into_inner());

                if cpu_hist.len() >= 60 {
                    cpu_hist.pop_front();
                    mem_hist.pop_front();
                    sys_mem_hist.pop_front();
                    ts_hist.pop_front();
                }

                cpu_hist.push_back(proc_cpu);
                mem_hist.push_back(proc_rss);
                sys_mem_hist.push_back(system_memory_mb);
                ts_hist.push_back(timestamp.clone());
            }

            let (cpu_history, mem_history, system_memory_history, timestamps) = {
                let cpu_hist = metrics.cpu_history.lock().unwrap_or_else(|e| e.into_inner());
                let mem_hist = metrics.mem_history.lock().unwrap_or_else(|e| e.into_inner());
                let sys_mem_hist = metrics.system_memory_history.lock().unwrap_or_else(|e| e.into_inner());
                let ts_hist = metrics.timestamps.lock().unwrap_or_else(|e| e.into_inner());

                (
                    cpu_hist.iter().cloned().collect::<Vec<_>>(),
                    mem_hist
                        .iter()
                        .map(|&m| m as f64 / (1024.0 * 1024.0))
                        .collect::<Vec<_>>(),
                    sys_mem_hist.iter().cloned().collect::<Vec<_>>(),
                    ts_hist.iter().cloned().collect::<Vec<_>>(),
                )
            };
            
            // 获取内存监控状态和性能统计
            let memory_monitor_status = match memory_manager.get_memory_status().await {
                Ok(status) => {
                    // 获取性能统计
                    let perf_stats = memory_manager.get_performance_stats().await;
                    let avg_memory = memory_manager.calculate_average_memory_usage().await;
                    let memory_trend = memory_manager.get_memory_trend().await;
                    
                    Some(serde_json::json!({
                        "current_memory_mb": status.current_mb,
                        "threshold_mb": status.threshold_mb,
                        "memory_pressure": match status.pressure {
                            crate::services::memory_service::MemoryPressure::Low => "low",
                            crate::services::memory_service::MemoryPressure::Medium => "medium",
                            crate::services::memory_service::MemoryPressure::High => "high",
                            crate::services::memory_service::MemoryPressure::Critical => "critical",
                        },
                        "memory_usage_percentage": (status.current_mb as f64 / status.threshold_mb as f64 * 100.0).round(),
                        "time_since_last_gc_secs": status.time_since_last_gc_secs,
                        "is_monitoring": status.is_monitoring,
                        "performance": {
                            "monitoring_cycles": perf_stats.monitoring_cycles,
                            "avg_monitoring_time_ms": perf_stats.avg_monitoring_time_ms,
                            "memory_query_success_rate": if perf_stats.memory_query_success + perf_stats.memory_query_failures > 0 {
                                (perf_stats.memory_query_success as f64 / (perf_stats.memory_query_success + perf_stats.memory_query_failures) as f64 * 100.0).round()
                            } else { 100.0 },
                            "avg_memory_query_time_ms": perf_stats.avg_memory_query_time_ms,
                            "current_dynamic_interval": perf_stats.current_dynamic_interval,
                            "interval_adjustments": perf_stats.interval_adjustments,
                        },
                        "statistics": {
                            "average_memory_mb": avg_memory.round(),
                            "memory_trend_mb_per_hour": memory_trend.map(|t| t.round()),
                        }
                    }))
                }
                Err(e) => {
                    log::warn!("Failed to get memory status for SSE: {}", e);
                    None
                }
            };

            let payload = serde_json::json!({
                "cpu": proc_cpu,
                "mem_rss": proc_rss,
                "mem_virtual": proc_virtual,
                "mem_rss_mb": proc_rss as f64 / (1024.0 * 1024.0),
                "mem_virtual_mb": proc_virtual as f64 / (1024.0 * 1024.0),
                "timestamp": timestamp,
                "cpu_history": cpu_history,
                "mem_history": mem_history,
                "system_memory_history": system_memory_history,
                "timestamps": timestamps,
                "memory_monitor": memory_monitor_status,
            });

            yield Event::json(&payload);
        }
    }
}
