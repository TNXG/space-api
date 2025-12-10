use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub username: String,
    pub email: Option<String>,
    pub avatar: Option<String>,
    pub qq_openid: Option<String>,
    pub is_verified: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl User {
    pub fn new(username: String) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        
        Self {
            id: None,
            username,
            email: None,
            avatar: None,
            qq_openid: None,
            is_verified: false,
            created_at: now.clone(),
            updated_at: now,
        }
    }
}