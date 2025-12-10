use crate::services::{db_service, verify_service::VerificationService};
use crate::utils::response::ApiResponse;
use crate::{Error, Result};
use mongodb::bson::doc;
use rocket::serde::{json::Json, Deserialize, Serialize};
use rocket::{get, post, routes, Route};
use serde_json::json;

#[derive(Debug, Serialize)]
pub struct Link {
    id: Option<String>,
    name: String,
    url: String,
    avatar: String,
    description: Option<String>,
    state: i32,
    created: String,
    rssurl: String,
    techstack: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct SubmitLinkRequest {
    name: String,
    url: String,
    avatar: String,
    description: String,
    created: Option<String>,
    rssurl: Option<String>,
    techstack: Option<Vec<String>>,
    email: String,
    code: String,
}

// 获取链接列表（对齐 TS：不按 verified 过滤）
#[get("/")]
async fn get_links() -> Result<Json<ApiResponse<Vec<Link>>>> {
    // 查询数据库，获取全部链接（如需分页可继续扩展）
    let links_docs = db_service::find_many("links", doc! {}).await?;

    // 将文档转换为Link结构体
    let mut links = Vec::new();
    for doc in links_docs {
        let link = Link {
            id: doc.get_object_id("_id").ok().map(|id| id.to_string()),
            name: doc.get_str("name").unwrap_or("").to_string(),
            url: doc.get_str("url").unwrap_or("").to_string(),
            avatar: doc.get_str("avatar").unwrap_or("").to_string(),
            description: doc.get_str("description").ok().map(|s| s.to_string()),
            state: doc.get_i32("state").unwrap_or(1),
            created: doc.get_str("created").unwrap_or("").to_string(),
            rssurl: doc.get_str("rssurl").unwrap_or("").to_string(),
            techstack: doc
                .get_array("techstack")
                .ok()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|b| b.as_str().map(|s| s.to_string()))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
        };
        links.push(link);
    }

    Ok(ApiResponse::success(links, "Links retrieved successfully"))
}

// 提交新链接
#[post("/submit", data = "<data>")]
async fn submit_link(
    data: Json<SubmitLinkRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>> {
    // 验证必填字段
    if data.name.trim().is_empty()
        || data.url.trim().is_empty()
        || data.avatar.trim().is_empty()
        || data.description.trim().is_empty()
        || data.email.trim().is_empty()
        || data.code.trim().is_empty()
    {
        return Err(Error::BadRequest("Missing required fields".to_string()));
    }

    // 验证邮箱验证码
    let code_ok = VerificationService::verify_code(&data.email, &data.code).await?;
    if !code_ok {
        return Err(Error::Unauthorized("Invalid verification code".to_string()));
    }

    // 规范化URL：去掉末尾斜杠
    let mut normalized_url = data.url.trim().to_string();
    if normalized_url.ends_with('/') {
        normalized_url.pop();
        while normalized_url.ends_with('/') {
            normalized_url.pop();
        }
    }

    // 不允许包含子目录
    if let Ok(url) = url::Url::parse(&normalized_url) {
        let path = url.path();
        if path.split('/').filter(|s| !s.is_empty()).count() > 0 {
            return Err(Error::BadRequest("URL不能包含子目录".to_string()));
        }
    }

    // 重复检查
    if let Some(_) = db_service::find_one("links", doc! { "url": &normalized_url }).await? {
        return Err(Error::Conflict("URL already exists".to_string()));
    }

    // 组装要插入的数据（对齐 TS 的 space-api 库结构）
    let created_iso = data
        .created
        .clone()
        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
    let rssurl = data.rssurl.clone().unwrap_or_default();
    let techstack = data.techstack.clone().unwrap_or_default();

    let link_doc = doc! {
        "name": &data.name,
        "url": &normalized_url,
        "avatar": &data.avatar,
        "description": &data.description,
        "state": 1i32,
        "created": &created_iso,
        "rssurl": &rssurl,
        "techstack": mongodb::bson::to_bson(&techstack).unwrap_or(mongodb::bson::Bson::Array(vec![])),
        "email": &data.email,
    };

    // 保存到数据库（当前仅写入一个数据库）
    let _id = db_service::insert_one("links", link_doc).await?;

    // 构造返回：移除 email
    let resp = json!({
        "name": data.name,
        "url": normalized_url,
        "avatar": data.avatar,
        "description": data.description,
        "state": 1,
        "created": created_iso,
        "rssurl": rssurl,
        "techstack": techstack,
    });

    Ok(ApiResponse::success(resp, "Link submitted successfully"))
}

pub fn routes() -> Vec<Route> {
    routes![get_links, submit_link]
}
