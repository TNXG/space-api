use dotenv::dotenv;
use rocket_dyn_templates::Template;
use space_api_rs::config;
use space_api_rs::routes;
use space_api_rs::routes::index::MetricsHistory;
use space_api_rs::services::db_service;
use space_api_rs::services::image_service::ImageService;
use space_api_rs::utils::charset::Utf8CharsetFairing;
use space_api_rs::utils::cache;
use std::time::Duration;

// Configure jemallocator
#[cfg(not(target_os = "windows"))]
#[global_allocator]
static ALLOC: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

#[rocket::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    let config = config::settings::load_config();
    let mongo_client = match db_service::initialize_db(&config.mongo).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("⚠️  数据库初始化失败: {}", e);
            return Err(e.into());
        }
    };

    // 启动缓存清理后台任务
    tokio::spawn(async {
        let mut interval = tokio::time::interval(Duration::from_secs(60 * 30)); // 每30分钟清理一次
        loop {
            interval.tick().await;
            cache::cleanup_expired_cache();
        }
    });

    let figment = rocket::Config::figment().merge(("template_dir", "src/templates"));

    // 使用 custom(figment) 替代 build()
    let rocket = rocket::custom(figment)
        .attach(Utf8CharsetFairing)
        .attach(Template::fairing())
        .mount("/", routes::index::routes())
        .mount("/avatar", routes::avatar::routes())
        .mount("/email", routes::email::routes())
        .mount("/images", routes::images::routes())
        .mount("/links", routes::links::routes())
        .mount("/oauth", routes::oauth::routes())
        .mount("/status", routes::status::routes())
        .mount("/", routes::sw::routes())
        .mount("/user", routes::user::routes())
        .manage(config)
        .manage(mongo_client)
        .manage(MetricsHistory::new())
        .manage(routes::index::SystemState::new())
        .manage(ImageService::new());

    // 从Cargo.toml获取版本号
    let version = concat!("v", env!("CARGO_PKG_VERSION"));
    println!(
        r#"
  ____                                         _ 
 / ___| _ __   __ _  ___ ___        __ _ _ __ (_)
 \___ \| '_ \ / _` |/ __/ _ \_____ / _` | '_ \| |
  ___) | |_) | (_| | (_|  __/_____| (_| | |_) | |
 |____/| .__/ \__,_|\___\___|      \__,_| .__/|_|
       |_|                              |_|      

 ✿ 🅢 🅟 🅐 🅒 🅔 - 🅐 🅟 🅘 ✿ ({version} BUILD WITH 🚀 Rust · Rocket.rs Framework)
    "#
    );
    rocket.launch().await?;

    Ok(())
}
