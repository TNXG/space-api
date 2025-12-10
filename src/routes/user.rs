use rocket::{Route, get, routes};
use rocket::serde::json::Json;
use mongodb::bson::{doc, Bson};
use crate::services::db_service;
use crate::utils::response::ApiResponse;
use crate::{Result, Error};

// 获取用户信息
#[get("/info?<qq_openid>&<openid>&<id>")]
async fn user_info(
    qq_openid: Option<&str>, 
    openid: Option<&str>, 
    id: Option<&str>,
) -> Result<Json<ApiResponse<serde_json::Value>>> {
    // 获取QQ OpenID
    let qqopenid = qq_openid.or(openid).or(id).ok_or_else(|| {
        Error::BadRequest("id is required".to_string())
    })?;
    
    // 查询数据库
    let user = db_service::find_one(
        "users", 
        doc! { "qq_openid": qqopenid }
    ).await?;
    
    // 检查用户是否存在
    match user {
        Some(user_doc) => {
            Ok(ApiResponse::success(
                serde_json::to_value(user_doc).map_err(|e| {
                    Error::Internal(format!("Failed to serialize user: {}", e))
                })?,
                "User found"
            ))
        }
        None => Err(Error::NotFound("User not found".to_string())),
    }
}

// 兼容 Nitro: GET /user/get?code= 临时代码换取用户信息
#[get("/get?<code>")]
async fn user_get(code: Option<&str>) -> Result<Json<ApiResponse<serde_json::Value>>> {
    let code = code.ok_or_else(|| Error::BadRequest("Temporary code is required".into()))?;

    // 查找未使用的临时代码
    let temp_opt = db_service::find_one("temp_codes", doc! { "code": code, "used": false }).await?;
    let temp = temp_opt.ok_or_else(|| Error::NotFound("Invalid or expired temporary code".into()))?;

    // 过期校验
    if let Some(Bson::String(expires_at)) = temp.get("expires_at") {
        if let Ok(exp) = chrono::DateTime::parse_from_rfc3339(expires_at) {
            if chrono::Utc::now() > exp.with_timezone(&chrono::Utc) {
                return Err(Error::Gone("Temporary code has expired".into()));
            }
        }
    }

    // 获取 openid
    let openid = match temp.get("qq_openid") {
        Some(Bson::String(s)) => s.clone(),
        _ => return Err(Error::Internal("Malformed temp code record".into())),
    };

    // 获取用户
    let user_doc_opt = db_service::find_one("users", doc! { "qq_openid": &openid }).await?;
    let user_doc = user_doc_opt.ok_or_else(|| Error::NotFound("User not found".into()))?;

    // 删除临时代码（一次性）
    let _ = db_service::delete_one("temp_codes", doc! { "code": code }).await?;

    // 构造返回
    let user_id = match user_doc.get("_id") {
        Some(Bson::ObjectId(oid)) => oid.to_hex(),
        _ => "".to_string(),
    };
    let nickname = user_doc.get_str("nickname").unwrap_or("").to_string();
    let avatar = user_doc.get_str("avatar").ok().map(|s| s.to_string());
    let gender = user_doc.get_str("gender").ok().map(|s| s.to_string());
    let created_at = user_doc.get_str("created_at").unwrap_or("").to_string();
    let updated_at = user_doc.get_str("updated_at").unwrap_or("").to_string();

    let data = serde_json::json!({
        "user_id": user_id,
        "qq_openid": openid,
        "nickname": nickname,
        "avatar": avatar,
        "gender": gender,
        "created_at": created_at,
        "updated_at": updated_at,
    });

    Ok(ApiResponse::success(data, "User information retrieved successfully"))
}

pub fn routes() -> Vec<Route> {
    routes![user_info, user_get]
}