use rocket::{Route, get, State, routes, Either};
use rocket::serde::json::Json;
use crate::config::settings::Config;
use crate::services::oauth_service::OAuthService;
use crate::utils::response::ApiResponse;
use crate::Result;
use mongodb::bson::doc;
use crate::services::db_service;
use rocket::response::Redirect;
use rocket::serde::json::serde_json;
use rand::RngCore;
use hex::ToHex;
use chrono::{Utc, Duration};
use url::Url;

// 兼容 Nitro: GET /oauth/qq/authorize?state=&return_url=&redirect=true|false
#[get("/qq/authorize?<state>&<return_url>&<redirect>")]
fn qq_authorize(
    state: Option<&str>,
    return_url: Option<&str>,
    redirect: Option<&str>,
    config: &State<Config>,
) -> Result<Either<Redirect, Json<ApiResponse<serde_json::Value>>>> {
    let oauth_service = OAuthService::new(config.oauth.clone());
    // 将 return_url 放入 state JSON
    let state_json = serde_json::json!({
        "original_state": state.unwrap_or(""),
        "return_url": return_url.unwrap_or("")
    })
    .to_string();

    let auth_url = oauth_service.get_qq_login_url(Some(&state_json));

    if redirect.unwrap_or("") == "true" {
        return Ok(Either::Left(Redirect::to(auth_url)));
    }

    // 返回与 Nitro 一致的 ApiResponse<{ authUrl }>
    let data = serde_json::json!({ "authUrl": auth_url });
    let resp = ApiResponse::success(data, "QQ OAuth authorization URL generated successfully");
    Ok(Either::Right(resp))
}

#[get("/qq/callback?<code>&<state>")]
async fn qq_callback(
    code: &str,
    state: Option<&str>,
    config: &State<Config>,
) -> Result<Redirect> {
    let oauth_service = OAuthService::new(config.oauth.clone());

    // 解析 state，提取 return_url 与 original_state
    let mut return_url = std::env::var("DEFAULT_RETURN_URL")
        .unwrap_or_else(|_| "http://localhost:3000".to_string());
    let mut original_state: Option<String> = None;
    if let Some(s) = state {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(s) {
            if let Some(r) = v.get("return_url").and_then(|x| x.as_str()) {
                if !r.is_empty() {
                    return_url = r.to_string();
                }
            }
            if let Some(os) = v.get("original_state").and_then(|x| x.as_str()) {
                if !os.is_empty() {
                    original_state = Some(os.to_string());
                }
            }
        } else {
            original_state = Some(s.to_string());
        }
    }

    // 完成 QQ OAuth 流程并处理错误：始终重定向
    let redirect = (|| async {
        let access_token = oauth_service.get_qq_access_token(code).await?;
        let openid = oauth_service.get_qq_openid(&access_token).await?;
        let user_info = oauth_service.get_qq_user_info(&access_token, &openid).await?;

        // upsert 用户
        let now = Utc::now();
        let existing_user = db_service::find_one("users", doc! { "qq_openid": &openid }).await?;

        let avatar = user_info
            .figureurl_qq_2
            .clone()
            .or(user_info.figureurl_2.clone())
            .unwrap_or_default();
        let nickname = user_info
            .nickname
            .clone()
            .unwrap_or_else(|| "QQ User".to_string());

        if existing_user.is_some() {
            let filter = doc! { "qq_openid": &openid };
            let update = doc! {
                "$set": {
                    "nickname": &nickname,
                    "avatar": &avatar,
                    "gender": user_info.gender.clone().unwrap_or_default(),
                    "updated_at": now.to_rfc3339(),
                    "last_login": now.to_rfc3339(),
                }
            };
            db_service::update_one("users", filter, update).await?;
        } else {
            let user_doc = doc! {
                "qq_openid": &openid,
                "nickname": &nickname,
                "avatar": &avatar,
                "gender": user_info.gender.clone().unwrap_or_default(),
                "created_at": now.to_rfc3339(),
                "updated_at": now.to_rfc3339(),
            };
            let _ = db_service::insert_one("users", user_doc).await?;
        }

        // 生成一次性临时代码，保存 temp_codes
        let mut buf = [0u8; 32];
        rand::rng().fill_bytes(&mut buf);
        let temp_code = buf.encode_hex::<String>();
        let expires_at = (now + Duration::minutes(10)).to_rfc3339();

        let temp_doc = doc! {
            "code": &temp_code,
            "qq_openid": &openid,
            "created_at": now.to_rfc3339(),
            "expires_at": &expires_at,
            "used": false,
        };
        let _ = db_service::insert_one("temp_codes", temp_doc).await?;

        // 构建成功重定向
        let mut url = Url::parse(&return_url).unwrap_or_else(|_| Url::parse("http://localhost:3000").unwrap());
        {
            let mut qp = url.query_pairs_mut();
            qp.append_pair("code", &temp_code);
            if let Some(os) = &original_state {
                qp.append_pair("state", os);
            }
        }
        Ok::<Url, crate::Error>(url)
    })().await;

    match redirect {
        Ok(url) => Ok(Redirect::to(url.to_string())),
        Err(e) => {
            // 构建错误重定向
            let mut url = Url::parse(&return_url).unwrap_or_else(|_| Url::parse("http://localhost:3000").unwrap());
            {
                let mut qp = url.query_pairs_mut();
                qp.append_pair("error", "oauth_failed");
                qp.append_pair("error_description", &e.to_string());
                if let Some(os) = original_state {
                    qp.append_pair("state", &os);
                }
            }
            Ok(Redirect::to(url.to_string()))
        }
    }
}

pub fn routes() -> Vec<Route> {
    routes![qq_authorize, qq_callback]
}