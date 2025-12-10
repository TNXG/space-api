use rocket::get;
use rocket_dyn_templates::{context, Template};
use rocket::request::{FromRequest, Outcome, Request};
use chrono::Local;

// 1. 更新结构体，增加 protocol 字段
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

        // 2. 获取协议版本
        // 优先使用 eo-connecting-protocol，如果没有（比如本地开发），则回退到 Rocket 检测到的版本
        let protocol = req.headers().get_one("eo-connecting-protocol")
            .map(|p| p.to_uppercase()) // 统一转大写，例如 "http/2.0" -> "HTTP/2.0"
            .unwrap_or_else(|| "Unknown".to_string());

        Outcome::Success(ClientInfo { ip, location, user_agent, protocol })
    }
}

#[get("/")]
pub fn index(client: ClientInfo) -> Template {
    let now = Local::now();
    
    Template::render("index", context! {
        version: "v3.0.0",
        server_time: now.format("%Y-%m-%d %H:%M:%S %Z").to_string(),
        client_ip: client.ip,
        client_location: client.location,
        // 3. 将协议传递给模板
        client_protocol: client.protocol, 
        raw_ua: client.user_agent 
    })
}

pub fn routes() -> Vec<rocket::Route> {
    rocket::routes![index]
}