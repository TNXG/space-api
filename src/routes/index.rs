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
use crate::services::memory_service::MemoryManager;

mod memory;
mod metrics;

pub use memory::{get_memory_report, get_memory_trend};
pub use metrics::{get_metrics, metrics_stream};


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
    pub fn new() -> Result<Self, sysinfo::Error> {
        Ok(Self {
            system: Arc::new(Mutex::new(System::new_all()?)),
        })
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
            .headers()
            .get_one("CF-Connecting-IP")
            .or_else(|| req.headers().get_one("X-Forwarded-For").and_then(|s| s.split(',').next()))
            .or_else(|| req.headers().get_one("X-Real-IP"))
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                req.client_ip()
                    .map(|ip| ip.to_string())
                    .unwrap_or_else(|| "Unknown".to_string())
            });

        let location = req
            .headers()
            .get_one("cf-ipcountry")
            .or_else(|| req.headers().get_one("eo-connecting-region"))
            .unwrap_or("Unknown Region")
            .to_string();

        let protocol = {
            // 检查是否是HTTPS
            let is_https = req.headers().get_one("x-forwarded-proto").map_or(false, |p| p == "https")
                || req.headers().get_one("cf-visitor").map_or(false, |v| v.contains("\"https\""))
                || req.headers().get_one("eo-connecting-protocol").map_or(false, |p| p.to_lowercase().contains("https"));
            
            // 尝试从headers中获取HTTP版本信息
            let version = req.headers().get_one(":version")
                .or_else(|| req.headers().get_one("http-version"))
                .unwrap_or("1.1"); // 默认HTTP/1.1
            
            if is_https {
                format!("HTTPS/{}", version)
            } else {
                format!("HTTP/{}", version)
            }
        };

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
        let mut sys = sys_state.system.lock().unwrap_or_else(|e| e.into_inner());
        
        // Refresh only what we need
        sys.refresh_memory();
        // 不需要refresh_cpu_all，因为我们只关心当前进程的CPU使用率
        
        let os_name = System::name().unwrap_or("Unknown".to_string());
        let sys_os_version = System::os_version().unwrap_or_default();
        let sys_kernel = System::kernel_version().unwrap_or("Unknown".to_string());
        let sys_hostname = System::host_name().unwrap_or("Unknown".to_string());
        
        let avg_load = System::load_average().unwrap_or_default();
        let uptime_sec = System::uptime().unwrap_or_default();
        let boot_time_sec = System::boot_time().unwrap_or_default();
        
        let total_system_mem = sys.total_memory();
        
        let (rss, virt, cpu) = get_process_stats(&mut sys);
        (total_system_mem, rss, virt, cpu,
         os_name, sys_os_version, sys_kernel, sys_hostname,
         avg_load, uptime_sec, boot_time_sec)
    };
    
    let boot_time = Local.timestamp_opt(boot_time_sec as i64, 0)
        .single()
        .unwrap_or_else(|| Local::now());

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
        ts_hist.push_back(timestamp);
    }

    // 获取历史数据用于图表
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



pub fn routes() -> Vec<rocket::Route> {
    rocket::routes![
        index,
        metrics::get_metrics,
        metrics::metrics_stream,
        memory::get_memory_report,
        memory::get_memory_trend
    ]
}

#[cfg(test)]
#[path = "../../tests/unit/index.rs"]
mod tests;