use dotenv::dotenv;
use log::{error, info, warn};
use rocket_dyn_templates::Template;
use space_api_rs::config;
use space_api_rs::routes;
use space_api_rs::routes::index::MetricsHistory;
use space_api_rs::services::db_service;
use space_api_rs::services::friend_avatar_service::FriendAvatarService;
use space_api_rs::services::image_service::ImageService;
use space_api_rs::services::memory_service::MemoryManager;
use space_api_rs::utils::cache;
use space_api_rs::utils::charset::Utf8CharsetFairing;
use std::sync::Arc;
use std::time::Duration;

#[cfg(not(target_os = "windows"))]
#[global_allocator]
static ALLOC: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

#[cfg(not(target_os = "windows"))]
#[allow(non_upper_case_globals)]
#[export_name = "malloc_conf"]
pub static malloc_conf: &[u8] = b"\
background_thread:true,\
dirty_decay_ms:5000,\
muzzy_decay_ms:5000,\
abort_conf:false,\
metadata_thp:auto,\
narenas:4\
\0";

#[rocket::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    // 初始化日志系统
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    let config = config::settings::load_config();
    let mongo_client = match db_service::initialize_db(&config.mongo).await {
        Ok(c) => c,
        Err(e) => {
            error!("数据库初始化失败: {}", e);
            return Err(e.into());
        }
    };

    // 初始化内存管理器
    let memory_manager = Arc::new(MemoryManager::new(config.memory.clone()));

    // 验证jemalloc配置
    if let Err(e) = memory_manager.validate_jemalloc_config() {
        warn!("内存管理配置验证失败: {}", e);
    }

    // 启动内存监控后台任务
    let _monitoring_handle = memory_manager.start_monitoring();
    info!(
        "内存监控系统已启动 (阈值: {} MB, 检查间隔: {} 秒)",
        config.memory.threshold_mb, config.memory.check_interval_secs
    );

    // 启动缓存清理后台任务（在阻塞线程中执行，避免阻塞 async runtime）
    tokio::spawn(async {
        let mut interval = tokio::time::interval(Duration::from_secs(60 * 30)); // 每30分钟清理一次
        loop {
            interval.tick().await;
            let _ = tokio::task::spawn_blocking(|| cache::cleanup_expired_cache()).await;
        }
    });

    // 输出初始内存状态
    if let Ok(status) = memory_manager.get_memory_status().await {
        info!(
            "初始内存状态: {} MB (阈值: {} MB, 压力等级: {:?})",
            status.current_mb, status.threshold_mb, status.pressure
        );
    }

    let figment = rocket::Config::figment().merge(("template_dir", "src/templates"));

    // 使用 custom(figment) 替代 build()
    let rocket = rocket::custom(figment)
        .attach(Utf8CharsetFairing)
        .attach(Template::fairing())
        .mount("/", routes::index::routes())
        .mount("/avatar", routes::avatar::routes())
        .mount("/email", routes::email::routes())
        .mount("/friend-avatar", routes::friend_avatar::routes())
        .mount("/images", routes::images::routes())
        .mount("/oauth", routes::oauth::routes())
        .mount("/status", routes::status::routes())
        .mount("/", routes::sw::routes())
        .mount("/user", routes::user::routes())
        .manage(config)
        .manage(mongo_client)
        .manage(MetricsHistory::new())
        .manage(routes::index::SystemState::new())
        .manage(ImageService::new())
        .manage(FriendAvatarService::new())
        .manage(memory_manager);

    // 从Cargo.toml获取版本号
    let version = concat!("v", env!("CARGO_PKG_VERSION"));
    info!(
        "\n\
    ╔═══════════════════════════════════════════════════════════════╗\n\
    ║                                                               ║\n\
    ║   ███████╗██████╗  █████╗  ██████╗███████╗                    ║\n\
    ║   ██╔════╝██╔══██╗██╔══██╗██╔════╝██╔════╝                    ║\n\
    ║   ███████╗██████╔╝███████║██║     █████╗                      ║\n\
    ║   ╚════██║██╔═══╝ ██╔══██║██║     ██╔══╝                      ║\n\
    ║   ███████║██║     ██║  ██║╚██████╗███████╗                    ║\n\
    ║   ╚══════╝╚═╝     ╚═╝  ╚═╝ ╚═════╝╚══════╝                    ║\n\
    ║                                                               ║\n\
    ║    █████╗ ██████╗ ██╗    ██████╗ ███████╗                     ║\n\
    ║   ██╔══██╗██╔══██╗██║    ██╔══██╗██╔════╝                     ║\n\
    ║   ███████║██████╔╝██║    ██████╔╝███████╗                     ║\n\
    ║   ██╔══██║██╔═══╝ ██║    ██╔══██╗╚════██║                     ║\n\
    ║   ██║  ██║██║     ██║    ██║  ██║███████║                     ║\n\
    ║   ╚═╝  ╚═╝╚═╝     ╚═╝    ╚═╝  ╚═╝╚══════╝                     ║\n\
    ║                                                               ║\n\
    ║   ✨ Version: {:<15} 🚀 Powered by Rust & Rocket     ║\n\
    ║                                                               ║\n\
    ╚═══════════════════════════════════════════════════════════════╝\n",
        version
    );
    rocket.launch().await?;

    Ok(())
}
