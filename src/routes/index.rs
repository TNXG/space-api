use rocket::get;
use rocket_dyn_templates::{context, Template};
use rocket::request::{FromRequest, Outcome, Request};
use chrono::{Local, TimeZone};
use sysinfo::System;
use rocket::State;
use mongodb::Client;

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
        let user_agent = req.headers().get_one("User-Agent").unwrap_or("Unknown").to_string();
        
        let ip = req.client_ip()
            .map(|ip| ip.to_string())
            .unwrap_or_else(|| "Unknown".to_string());

        let location = req.headers().get_one("cf-ipcountry")
            .or_else(|| req.headers().get_one("eo-connecting-region"))
            .unwrap_or("Unknown Region")
            .to_string();

        let protocol = req.headers().get_one("eo-connecting-protocol")
            .map(|p| p.to_uppercase())
            .unwrap_or_else(|| "Unknown".to_string());

        Outcome::Success(ClientInfo { ip, location, user_agent, protocol })
    }
}

// 格式化时长
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

// 格式化字节
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

#[get("/")]
pub async fn index(client: ClientInfo, mongo_client: &State<Client>) -> Template {
    let now = Local::now();
    
    let os_name = System::name().unwrap_or("Unknown".to_string());
    let avg_load = System::load_average();
    let uptime_sec = System::uptime();
    let boot_time_sec = System::boot_time();
    let boot_time = Local.timestamp_opt(boot_time_sec as i64, 0).unwrap();

    let sys_os_version = System::os_version().unwrap_or_default();
    let sys_kernel = System::kernel_version().unwrap_or("Unknown".to_string());
    let sys_hostname = System::host_name().unwrap_or("Unknown".to_string());

    // 内存信息依然需要实例并刷新
    let mut sys = System::new_all();
    sys.refresh_all();
    
    let total_mem = sys.total_memory();
    let used_mem = sys.used_memory();

    let mem_percent = (used_mem as f64 / total_mem as f64) * 100.0;

    let mongo_status = match mongo_client.list_database_names().await {
        Ok(_) => "Connected",
        Err(_) => "Disconnected"
    };

    Template::render("index", context! {
        version: "v3.1.0",
        server_time: now.format("%Y-%m-%d %H:%M:%S %Z").to_string(),
        client_ip: client.ip,
        client_location: client.location,
        client_protocol: client.protocol, 
        raw_ua: client.user_agent,
        
        // 系统信息
        sys_os: format!("{} {}", os_name, sys_os_version),
        sys_arch: std::env::consts::ARCH,
        sys_kernel: sys_kernel,
        sys_hostname: sys_hostname,
        
        // 状态信息
        sys_uptime: format_duration(uptime_sec),
        sys_boot_time: boot_time.format("%Y-%m-%d %H:%M").to_string(),
        sys_load_avg: format!("{:.2} / {:.2} / {:.2}", avg_load.one, avg_load.five, avg_load.fifteen),
        
        // 资源信息
        mem_total: format_bytes(total_mem),
        mem_used: format_bytes(used_mem),
        mem_percent: format!("{:.1}", mem_percent),
        
        // 服务状态
        mongo_status: mongo_status,
    })
}

pub fn routes() -> Vec<rocket::Route> {
    rocket::routes![index]
}