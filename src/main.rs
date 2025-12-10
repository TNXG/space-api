use dotenv::dotenv;
use space_api_rs::config;
use space_api_rs::routes;
use space_api_rs::utils::charset::Utf8CharsetFairing;
use rocket_dyn_templates::Template;
use space_api_rs::services::db_service;

#[rocket::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    
    let config = config::settings::load_config();
    if let Err(e) = db_service::initialize_db(&config.mongo).await {
        eprintln!("⚠️  数据库初始化失败: {}", e);
    }

    let figment = rocket::Config::figment()
        .merge(("template_dir", "src/templates")); 

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
        .manage(config);
        
    println!(r#"
  ____                                         _ 
 / ___| _ __   __ _  ___ ___        __ _ _ __ (_)
 \___ \| '_ \ / _` |/ __/ _ \_____ / _` | '_ \| |
  ___) | |_) | (_| | (_|  __/_____| (_| | |_) | |
 |____/| .__/ \__,_|\___\___|      \__,_| .__/|_|
       |_|                              |_|      

 ✿ 🅢 🅟 🅐 🅒 🅔 - 🅐 🅟 🅘 ✿ (v3.0.0 BUILD WITH 🚀 Rust · Rocket.rs Framework)
    "#);
    rocket.launch().await?;
    
    Ok(())
}