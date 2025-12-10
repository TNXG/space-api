use crate::{Result, Error};
use crate::config::settings::OAuthConfig;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize)]
pub struct QQUserInfo {
    pub openid: String,
    pub nickname: Option<String>,
    pub figureurl: Option<String>,
    pub figureurl_1: Option<String>,
    pub figureurl_2: Option<String>,
    pub figureurl_qq_1: Option<String>,
    pub figureurl_qq_2: Option<String>,
    pub gender: Option<String>,
}

pub struct OAuthService {
    config: OAuthConfig,
    client: Client,
}

impl OAuthService {
    pub fn new(config: OAuthConfig) -> Self {
        Self {
            config,
            client: Client::new(),
        }
    }
    
    // 获取QQ登录URL（可带自定义 state）
    pub fn get_qq_login_url(&self, state: Option<&str>) -> String {
        let state_param = state.unwrap_or("state");
        format!(
            "https://graph.qq.com/oauth2.0/authorize?response_type=code&client_id={}&redirect_uri={}&scope={}&state={}",
            self.config.qq_app_id,
            urlencoding::encode(&self.config.redirect_uri),
            // 与 Nitro 版本保持一致，请求 get_user_info 权限
            urlencoding::encode("get_user_info"),
            urlencoding::encode(state_param)
        )
    }
    
    // 使用授权码获取QQ访问令牌
    pub async fn get_qq_access_token(&self, code: &str) -> Result<String> {
        let url = format!(
            "https://graph.qq.com/oauth2.0/token?grant_type=authorization_code&client_id={}&client_secret={}&code={}&redirect_uri={}",
            self.config.qq_app_id,
            self.config.qq_app_key,
            code,
            urlencoding::encode(&self.config.redirect_uri)
        );
        
        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| Error::Internal(format!("Failed to get access token: {}", e)))?;
            
        let text = response
            .text()
            .await
            .map_err(|e| Error::Internal(format!("Failed to read response: {}", e)))?;
            
        // 解析响应（格式为：access_token=xxx&expires_in=xxx&refresh_token=xxx）
        let params: Vec<&str> = text.split('&').collect();
        for param in params {
            let kv: Vec<&str> = param.split('=').collect();
            if kv.len() == 2 && kv[0] == "access_token" {
                return Ok(kv[1].to_string());
            }
        }
        
        Err(Error::Internal("Failed to parse access token".to_string()))
    }
    
    // 使用访问令牌获取OpenID
    pub async fn get_qq_openid(&self, access_token: &str) -> Result<String> {
        let url = format!(
            "https://graph.qq.com/oauth2.0/me?access_token={}&fmt=json",
            access_token
        );
        
        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| Error::Internal(format!("Failed to get OpenID: {}", e)))?;
            
        let data: Value = response
            .json()
            .await
            .map_err(|e| Error::Internal(format!("Failed to parse response: {}", e)))?;
            
        if let Some(openid) = data["openid"].as_str() {
            Ok(openid.to_string())
        } else {
            Err(Error::Internal("OpenID not found in response".to_string()))
        }
    }
    
    // 获取QQ用户信息
    pub async fn get_qq_user_info(&self, access_token: &str, openid: &str) -> Result<QQUserInfo> {
        let url = format!(
            "https://graph.qq.com/user/get_user_info?access_token={}&oauth_consumer_key={}&openid={}&fmt=json",
            access_token,
            self.config.qq_app_id,
            openid
        );
        
        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| Error::Internal(format!("Failed to get user info: {}", e)))?;
            
        let data: Value = response
            .json()
            .await
            .map_err(|e| Error::Internal(format!("Failed to parse response: {}", e)))?;
            
        if data["ret"].as_i64().unwrap_or(-1) != 0 {
            return Err(Error::Internal(format!("QQ API error: {}", data["msg"].as_str().unwrap_or("Unknown error"))));
        }
        
        Ok(QQUserInfo {
            openid: openid.to_string(),
            nickname: data["nickname"].as_str().map(|s| s.to_string()),
            figureurl: data["figureurl"].as_str().map(|s| s.to_string()),
            figureurl_1: data["figureurl_1"].as_str().map(|s| s.to_string()),
            figureurl_2: data["figureurl_2"].as_str().map(|s| s.to_string()),
            figureurl_qq_1: data["figureurl_qq_1"].as_str().map(|s| s.to_string()),
            figureurl_qq_2: data["figureurl_qq_2"].as_str().map(|s| s.to_string()),
            gender: data["gender"].as_str().map(|s| s.to_string()),
        })
    }
}