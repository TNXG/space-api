use crate::{Error, Result};
use moka::future::Cache;
use once_cell::sync::Lazy;
use rand::Rng;
// 暂时移除，我们使用其他方式生成验证码
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// 验证码缓存（邮箱 -> (验证码，过期时间戳)）
pub static VERIFICATION_CACHE: Lazy<Cache<String, (String, u64)>> = Lazy::new(|| {
    Cache::builder()
        .time_to_live(Duration::from_secs(600)) // 10分钟
        .build()
});

pub struct VerificationService;

impl VerificationService {
    // 生成验证码
    pub fn generate_verification_code() -> String {
        let mut rng = rand::rng();
        let code: String = (0..6)
            .map(|_| rng.random_range(0..10).to_string())
            .collect();
        code
    }

    // 存储验证码
    pub async fn store_verification_code(email: &str, code: &str) -> Result<()> {
        let expiry = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0))
            .as_secs()
            + 600; // 10分钟后过期

        VERIFICATION_CACHE
            .insert(email.to_string(), (code.to_string(), expiry))
            .await;
        Ok(())
    }

    // 验证验证码
    pub async fn verify_code(email: &str, code: &str) -> Result<bool> {
        if let Some((stored_code, expiry)) = VERIFICATION_CACHE.get(email).await {
            let current_time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_else(|_| Duration::from_secs(0))
                .as_secs();

            // 如果验证码已过期
            if current_time > expiry {
                VERIFICATION_CACHE.remove(email).await;
                return Ok(false);
            }

            // 验证码匹配
            if stored_code == code {
                VERIFICATION_CACHE.remove(email).await;
                return Ok(true);
            }

            // 验证码不匹配
            Ok(false)
        } else {
            // 未找到验证码
            Err(Error::NotFound(
                "Verification code not found or expired".to_string(),
            ))
        }
    }
}
