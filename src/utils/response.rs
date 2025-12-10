use rocket::serde::{Serialize, json::Json};

#[derive(Debug, Serialize)]
pub struct ApiResponse<T>
where
    T: Serialize,
{
    pub code: String,
    pub message: String,
    pub status: String,
    pub data: Option<T>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn success(data: T, message: &str) -> Json<Self> {
        Json(Self {
            code: "200".to_string(),
            message: message.to_string(),
            status: "success".to_string(),
            data: Some(data),
        })
    }
    
    pub fn error(code: &str, message: &str) -> Json<Self> {
        Json(Self {
            code: code.to_string(),
            message: message.to_string(),
            status: "error".to_string(),
            data: None,
        })
    }
}

// 为没有数据的响应提供便利方法
impl ApiResponse<()> {
    pub fn failed(code: &str, message: &str) -> Json<Self> {
        Json(Self {
            code: code.to_string(),
            message: message.to_string(),
            status: "failed".to_string(),
            data: None,
        })
    }
}