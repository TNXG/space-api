use chrono::{Local, TimeZone};
use mongodb::Client;
use rocket::get;
use rocket::request::{FromRequest, Outcome, Request};
use rocket::State;
use rocket_dyn_templates::{context, Template};
use std::collections::VecDeque;
use std::process;
use std::sync::{Arc, Mutex};
use sysinfo::{Pid, ProcessesToUpdate, System};
use rocket::response::stream::{Event, EventStream};
use rocket::tokio::time::{interval, Duration};
use crate::services::memory_service::MemoryManager;


// 存储历史数据的结构
#[derive(Clone)]
pub struct MetricsHistory {
    pub cpu_history: Arc<Mutex<VecDeque<f32>>>,
    pub mem_history: Arc<Mutex<VecDeque<u64>>>,
    pub system_memory_history: Arc<Mutex<VecDeque<u64>>>,
    pub timestamps: Arc<Mutex<VecDeque<String>>>,
}

impl MetricsHistory {
    pub fn new() -> Self {
        Self {
            cpu_history: Arc::new(Mutex::new(VecDeque::with_capacity(60))),
            mem_history: Arc::new(Mutex::new(VecDeque::with_capacity(60))),
            system_memory_history: Arc::new(Mutex::new(VecDeque::with_capacity(60))),
            timestamps: Arc::new(Mutex::new(VecDeque::with_capacity(60))),
        }
    }
}

#[derive(Clone)]
pub struct SystemState {
    pub system: Arc<Mutex<System>>,
}

impl SystemState {
    pub fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();
        Self {
            system: Arc::new(Mutex::new(sys)),
        }
    }
}

pub struct ClientInfo {
    pub ip: String,
    pub location: String,
    pub user_agent: String,
    pub protocol: String,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for ClientInfo {
    type Error = ();
    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let user_agent = req
            .headers()
            .get_one("User-Agent")
            .unwrap_or("Unknown")
            .to_string();

        let ip = req
            .client_ip()
            .map(|ip| ip.to_string())
            .unwrap_or_else(|| "Unknown".to_string());

        let location = req
            .headers()
            .get_one("cf-ipcountry")
            .or_else(|| req.headers().get_one("eo-connecting-region"))
            .unwrap_or("Unknown Region")
            .to_string();

        let protocol = req
            .headers()
            .get_one("eo-connecting-protocol")
            .or_else(|| req.headers().get_one("x-forwarded-proto"))
            .or_else(|| req.headers().get_one("cf-visitor"))
            .map(|p| {
                // 解码HTML实体
                let decoded = p
                    .replace("&#x2F;", "/")
                    .replace("&#x3A;", ":")
                    .replace("&amp;", "&")
                    .replace("&lt;", "<")
                    .replace("&gt;", ">")
                    .replace("&quot;", "\"");
                
                // 处理 CloudFlare 的 cf-visitor JSON 格式
                if decoded.starts_with("{") && decoded.contains("scheme") {
                    if decoded.contains("\"https\"") {
                        "HTTPS".to_string()
                    } else if decoded.contains("\"http\"") {
                        "HTTP".to_string()
                    } else {
                        decoded.to_uppercase()
                    }
                } else {
                    decoded.to_uppercase()
                }
            })
            .unwrap_or_else(|| {
                // 本地环境或无CDN头时，根据TLS推断协议
                if req.headers().get_one("x-forwarded-proto").map_or(false, |p| p == "https") {
                    "HTTPS".to_string()
                } else {
                    // 本地开发环境默认HTTP
                    "HTTP".to_string()
                }
            });

        Outcome::Success(ClientInfo {
            ip,
            location,
            user_agent,
            protocol,
        })
    }
}

fn format_duration(seconds: u64) -> String {
    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;
    let minutes = (seconds % 3600) / 60;

    if days > 0 {
        format!("{}d {}h {}m", days, hours, minutes)
    } else if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else {
        format!("{}m", minutes)
    }
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    if bytes >= GB {
        format!("{:.2} GiB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MiB", bytes as f64 / MB as f64)
    } else {
        format!("{:.2} KiB", bytes as f64 / KB as f64)
    }
}

fn get_process_stats(sys: &mut System) -> (u64, u64, f32) {
    let pid = Pid::from(process::id() as usize);

    // Refresh process info
    sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
    
    if let Some(proc) = sys.process(pid) {
        // proc.cpu_usage() 返回的是当前进程的CPU使用率百分比
        // 这个值已经是百分比形式，不需要除以核心数
        (proc.memory(), proc.virtual_memory(), proc.cpu_usage())
    } else {
        (0, 0, 0.0)
    }
}

#[get("/")]
pub async fn index(
    client: ClientInfo,
    mongo_client: &State<Client>,
    metrics: &State<MetricsHistory>,
    sys_state: &State<SystemState>,
    memory_manager: &State<Arc<MemoryManager>>,
) -> Template {
    let now = Local::now();

    // Scope the lock so it drops before async calls
    let (total_system_mem, proc_rss, proc_virtual, proc_cpu_raw, 
         os_name, sys_os_version, sys_kernel, sys_hostname, 
         avg_load, uptime_sec, boot_time_sec) = {
        let mut sys = sys_state.system.lock().unwrap();
        
        // Refresh only what we need
        sys.refresh_memory();
        // 不需要refresh_cpu_all，因为我们只关心当前进程的CPU使用率
        
        let os_name = System::name().unwrap_or("Unknown".to_string());
        let sys_os_version = System::os_version().unwrap_or_default();
        let sys_kernel = System::kernel_version().unwrap_or("Unknown".to_string());
        let sys_hostname = System::host_name().unwrap_or("Unknown".to_string());
        
        let avg_load = System::load_average();
        let uptime_sec = System::uptime();
        let boot_time_sec = System::boot_time();
        
        let total_system_mem = sys.total_memory();
        
        let (rss, virt, cpu) = get_process_stats(&mut sys);
        (total_system_mem, rss, virt, cpu,
         os_name, sys_os_version, sys_kernel, sys_hostname,
         avg_load, uptime_sec, boot_time_sec)
    };
    
    let boot_time = Local.timestamp_opt(boot_time_sec as i64, 0).unwrap();

    // 进程CPU使用率已经是正确的百分比值，不需要除以核心数
    // sysinfo的process.cpu_usage()返回的是该进程占用的CPU百分比（0-100%）
    let proc_cpu = proc_cpu_raw;

    let mem_percent = if total_system_mem > 0 {
        (proc_rss as f64 / total_system_mem as f64) * 100.0
    } else {
        0.0
    };

    // 获取系统内存监控状态
    let system_memory_mb = match memory_manager.get_memory_status().await {
        Ok(status) => status.current_mb,
        Err(_) => 0,
    };

    // 更新历史数据
    let timestamp = now.format("%H:%M:%S").to_string();
    {
        let mut cpu_hist = metrics.cpu_history.lock().unwrap();
        let mut mem_hist = metrics.mem_history.lock().unwrap();
        let mut sys_mem_hist = metrics.system_memory_history.lock().unwrap();
        let mut ts_hist = metrics.timestamps.lock().unwrap();

        if cpu_hist.len() >= 60 {
            cpu_hist.pop_front();
            mem_hist.pop_front();
            sys_mem_hist.pop_front();
            ts_hist.pop_front();
        }

        cpu_hist.push_back(proc_cpu);
        mem_hist.push_back(proc_rss);
        sys_mem_hist.push_back(system_memory_mb);
        ts_hist.push_back(timestamp);
    }

    // 获取历史数据用于图表
    let (cpu_history, mem_history, system_memory_history, timestamps) = {
        let cpu_hist = metrics.cpu_history.lock().unwrap();
        let mem_hist = metrics.mem_history.lock().unwrap();
        let sys_mem_hist = metrics.system_memory_history.lock().unwrap();
        let ts_hist = metrics.timestamps.lock().unwrap();

        (
            cpu_hist.iter().cloned().collect::<Vec<_>>(),
            mem_hist
                .iter()
                .map(|&m| m as f64 / (1024.0 * 1024.0))
                .collect::<Vec<_>>(), // 转换为 MiB
            sys_mem_hist.iter().cloned().collect::<Vec<_>>(),
            ts_hist.iter().cloned().collect::<Vec<_>>(),
        )
    };

    let mongo_status = match mongo_client.list_database_names().await {
        Ok(_) => "Connected",
        Err(_) => "Disconnected",
    };

    Template::render(
        "index",
        context! {
            version: concat!("v", env!("CARGO_PKG_VERSION")),
            server_time: now.format("%Y-%m-%d %H:%M:%S %Z").to_string(),
            client_ip: client.ip,
            client_location: client.location,
            client_protocol: client.protocol,
            raw_ua: client.user_agent,

            sys_os: format!("{} {}", os_name, sys_os_version),
            sys_arch: std::env::consts::ARCH,
            sys_kernel: sys_kernel,
            sys_hostname: sys_hostname,

            sys_uptime: format_duration(uptime_sec),
            sys_boot_time: boot_time.format("%Y-%m-%d %H:%M").to_string(),
            sys_load_avg: format!("{:.2} / {:.2} / {:.2}", avg_load.one, avg_load.five, avg_load.fifteen),

            // 进程资源使用
            proc_cpu: format!("{:.1}", proc_cpu),
            proc_mem_rss: format_bytes(proc_rss),
            proc_mem_virtual: format_bytes(proc_virtual),
            proc_mem_percent: format!("{:.2}", mem_percent),

            // 系统总内存
            sys_mem_total: format_bytes(total_system_mem),

            // 历史数据（JSON 格式）
            cpu_history_json: serde_json::to_string(&cpu_history).unwrap_or_default(),
            mem_history_json: serde_json::to_string(&mem_history).unwrap_or_default(),
            system_memory_history_json: serde_json::to_string(&system_memory_history).unwrap_or_default(),
            timestamps_json: serde_json::to_string(&timestamps).unwrap_or_default(),

            mongo_status: mongo_status,
        },
    )
}

// API 端点用于实时更新数据
#[get("/api/metrics")]
pub async fn get_metrics(
    metrics: &State<MetricsHistory>,
    sys_state: &State<SystemState>,
    memory_manager: &State<Arc<MemoryManager>>,
) -> rocket::serde::json::Json<serde_json::Value> {
    let (proc_rss, proc_cpu_raw) = {
        let mut sys = sys_state.system.lock().unwrap();
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
        let mut cpu_hist = metrics.cpu_history.lock().unwrap();
        let mut mem_hist = metrics.mem_history.lock().unwrap();
        let mut sys_mem_hist = metrics.system_memory_history.lock().unwrap();
        let mut ts_hist = metrics.timestamps.lock().unwrap();

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
        let cpu_hist = metrics.cpu_history.lock().unwrap();
        let mem_hist = metrics.mem_history.lock().unwrap();
        let sys_mem_hist = metrics.system_memory_history.lock().unwrap();
        let ts_hist = metrics.timestamps.lock().unwrap();

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
                // Warning: Blocking operation in async loop. 
                // sysinfo refresh is usually fast but strictly should be spawn_blocking.
                // For simplicity we keep it inline as requested "simple implementation".
                // If needed we can wrap in task::spawn_blocking.
                let mut sys = sys_state.system.lock().unwrap();
                sys.refresh_memory();
                // 不需要refresh_cpu_all，因为我们只关心当前进程的CPU使用率
                let pid = Pid::from(process::id() as usize);
                sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
                
                let (rss, virt, cpu) = if let Some(proc) = sys.process(pid) {
                    (proc.memory(), proc.virtual_memory(), proc.cpu_usage())
                } else {
                    (0, 0, 0.0)
                };
                (rss, virt, cpu)
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
                let mut cpu_hist = metrics.cpu_history.lock().unwrap();
                let mut mem_hist = metrics.mem_history.lock().unwrap();
                let mut sys_mem_hist = metrics.system_memory_history.lock().unwrap();
                let mut ts_hist = metrics.timestamps.lock().unwrap();

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
                let cpu_hist = metrics.cpu_history.lock().unwrap();
                let mem_hist = metrics.mem_history.lock().unwrap();
                let sys_mem_hist = metrics.system_memory_history.lock().unwrap();
                let ts_hist = metrics.timestamps.lock().unwrap();

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

// API 端点用于获取详细的内存性能报告
#[get("/api/memory/report")]
pub async fn get_memory_report(
    memory_manager: &State<Arc<MemoryManager>>,
) -> rocket::serde::json::Json<serde_json::Value> {
    match memory_manager.generate_memory_report().await {
        report => {
            rocket::serde::json::Json(serde_json::json!({
                "status": "success",
                "report": report
            }))
        }
    }
}

// API 端点用于获取内存使用趋势
#[get("/api/memory/trend")]
pub async fn get_memory_trend(
    memory_manager: &State<Arc<MemoryManager>>,
) -> rocket::serde::json::Json<serde_json::Value> {
    let trend = memory_manager.get_memory_trend().await;
    let avg_usage = memory_manager.calculate_average_memory_usage().await;
    let perf_stats = memory_manager.get_performance_stats().await;
    
    rocket::serde::json::Json(serde_json::json!({
        "status": "success",
        "data": {
            "trend_mb_per_hour": trend,
            "average_usage_mb": avg_usage,
            "monitoring_cycles": perf_stats.monitoring_cycles,
            "current_interval_seconds": perf_stats.current_dynamic_interval,
            "query_success_rate": if perf_stats.memory_query_success + perf_stats.memory_query_failures > 0 {
                (perf_stats.memory_query_success as f64 / (perf_stats.memory_query_success + perf_stats.memory_query_failures) as f64 * 100.0).round()
            } else { 100.0 }
        }
    }))
}

pub fn routes() -> Vec<rocket::Route> {
    rocket::routes![index, get_metrics, metrics_stream, get_memory_report, get_memory_trend]
}

#[cfg(test)]
mod tests {
    use crate::services::memory_service::MemoryManager;
    use crate::config::settings::MemoryConfig;

    #[tokio::test]
    async fn test_memory_status_serialization() {
        let config = MemoryConfig {
            threshold_mb: 500,
            check_interval_secs: 30,
            gc_cooldown_secs: 30,
        };
        let manager = MemoryManager::new(config);

        // 获取内存状态
        let status = manager.get_memory_status().await;
        assert!(status.is_ok());

        if let Ok(status) = status {
            // 测试序列化为JSON
            let json_status = serde_json::json!({
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
            });

            println!("Memory status JSON: {}", serde_json::to_string_pretty(&json_status).unwrap());
            
            // 验证JSON结构
            assert!(json_status["current_memory_mb"].is_number());
            assert!(json_status["threshold_mb"].is_number());
            assert!(json_status["memory_pressure"].is_string());
            assert!(json_status["memory_usage_percentage"].is_number());
            assert!(json_status["time_since_last_gc_secs"].is_number());
            assert!(json_status["is_monitoring"].is_boolean());
        }
    }

    #[tokio::test]
    async fn test_memory_pressure_levels() {
        let config = MemoryConfig {
            threshold_mb: 100, // 低阈值便于测试
            check_interval_secs: 30,
            gc_cooldown_secs: 30,
        };
        let manager = MemoryManager::new(config);

        // 测试不同内存使用量对应的压力等级
        let test_cases = vec![
            (30, "low"),      // 30% < 60%
            (70, "medium"),   // 70% 在 60%-80%
            (90, "high"),     // 90% 在 80%-100%
            (120, "critical"), // 120% > 100%
        ];

        for (usage_mb, expected_pressure) in test_cases {
            manager.update_memory_pressure(usage_mb).await;
            let status = manager.get_memory_status().await.unwrap();
            
            let pressure_str = match status.pressure {
                crate::services::memory_service::MemoryPressure::Low => "low",
                crate::services::memory_service::MemoryPressure::Medium => "medium",
                crate::services::memory_service::MemoryPressure::High => "high",
                crate::services::memory_service::MemoryPressure::Critical => "critical",
            };
            
            assert_eq!(pressure_str, expected_pressure, 
                "Memory usage {}MB should result in {} pressure", usage_mb, expected_pressure);
            
            println!("✓ {}MB usage -> {} pressure", usage_mb, pressure_str);
        }
    }
}