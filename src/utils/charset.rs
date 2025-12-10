use rocket::fairing::{Fairing, Info, Kind};
use rocket::http::Header;
use rocket::{Request, Response};

// 在所有响应中为文本类内容类型追加 charset=utf-8（若未显式指定）
pub struct Utf8CharsetFairing;

#[rocket::async_trait]
impl Fairing for Utf8CharsetFairing {
    fn info(&self) -> Info {
        Info {
            name: "Default UTF-8 charset for textual responses",
            kind: Kind::Response,
        }
    }

    async fn on_response<'r>(&self, _req: &'r Request<'_>, res: &mut Response<'r>) {
        // 检查 Content-Type 头部
        if let Some(ct_val) = res.headers().get_one("Content-Type") {
            let lower = ct_val.to_ascii_lowercase();
            // 只对常见文本类类型追加 charset：text/*、application/json、application/javascript、application/xml、application/xhtml+xml
            let is_textual = lower.starts_with("text/")
                || lower.starts_with("application/json")
                || lower.starts_with("application/javascript")
                || lower.starts_with("application/xml")
                || lower.starts_with("application/xhtml+xml");
            let has_charset = lower.contains("charset=");

            if is_textual && !has_charset {
                let new_val = format!("{}; charset=utf-8", ct_val);
                res.set_header(Header::new("Content-Type", new_val));
            }
        }
        // 添加 CORS 头
        res.set_header(Header::new("Access-Control-Allow-Origin", "*"));
    }
}
