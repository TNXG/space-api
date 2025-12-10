use rocket::{Route, post, routes, State};
use rocket::serde::{json::Json, Deserialize};
use crate::config::settings::Config;
use crate::services::email_service::EmailService;
use crate::services::verify_service::VerificationService;
use crate::utils::response::ApiResponse;
use crate::{Result, Error};

#[derive(Debug, Deserialize)]
pub struct SendEmailRequest {
    email: String,
}

#[derive(Debug, Deserialize)]
pub struct VerifyEmailRequest {
    email: String,
    code: String,
}

// 发送邮件路由
#[post("/send", data = "<data>")]
async fn send_email(data: Json<SendEmailRequest>, config: &State<Config>) -> Result<Json<ApiResponse<String>>> {
    // 验证邮箱格式
    if !data.email.contains('@') || !data.email.contains('.') {
        return Err(Error::BadRequest("Invalid email format".to_string()));
    }
    
    // 生成验证码
    let verification_code = VerificationService::generate_verification_code();
    
    // 存储验证码
    VerificationService::store_verification_code(&data.email, &verification_code).await?;
    
    // 创建邮件服务
    let email_service = EmailService::new(config.email.clone())?;
    
    // 发送验证邮件
    email_service.send_verification_email(&data.email, &verification_code).await?;
    
    Ok(ApiResponse::success("Verification email sent successfully".to_string(), "验证邮件已发送"))
}

// 验证邮箱路由
#[post("/verify", data = "<data>")]
async fn verify_email(data: Json<VerifyEmailRequest>) -> Result<Json<ApiResponse<bool>>> {
    // 验证验证码
    let verified = VerificationService::verify_code(&data.email, &data.code).await?;
    
    if verified {
        Ok(ApiResponse::success(true, "Email verified successfully"))
    } else {
        Ok(ApiResponse::success(false, "Verification code is invalid or expired"))
    }
}

pub fn routes() -> Vec<Route> {
    routes![send_email, verify_email]
}