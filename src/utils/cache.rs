use moka::future::Cache;
use once_cell::sync::Lazy;
use std::time::Duration;

// 创建一个全局的缓存实例
pub static CACHE_BUCKET: Lazy<Cache<String, Vec<u8>>> = Lazy::new(|| {
    Cache::builder()
        .time_to_live(Duration::from_secs(12 * 60 * 60)) // 12小时刷新全部缓存
        .time_to_idle(Duration::from_secs(2 * 60 * 60)) // 2小时不访问则失效
        .max_capacity(1000) // 最多缓存1000个项目
        .build()
});

// 缓存项目，返回是否是新插入的项目
pub async fn put<K, V>(cache: &Cache<K, V>, key: K, value: V) -> bool
where
    K: Clone + std::hash::Hash + Eq + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    let exists = cache.get(&key).await.is_some();
    cache.insert(key, value).await;
    !exists
}

// 从缓存获取项目
pub async fn get<K, V>(cache: &Cache<K, V>, key: &K) -> Option<V>
where
    K: Clone + std::hash::Hash + Eq + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    cache.get(key).await
}

// 检查缓存中是否存在指定的键
pub async fn exists<K, V>(cache: &Cache<K, V>, key: &K) -> bool
where
    K: Clone + std::hash::Hash + Eq + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    cache.get(key).await.is_some()
}

// 从缓存中删除项目
pub async fn remove<K, V>(cache: &Cache<K, V>, key: &K)
where
    K: Clone + std::hash::Hash + Eq + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    cache.remove(key).await;
}

// ==========================================
// Disk Cache Implementation
// ==========================================

use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

const CACHE_DIR: &str = "cache";
const IMAGE_CACHE_TTL: u64 = 30; // 30 seconds

fn get_cache_path(key: &str) -> PathBuf {
    let mut path = PathBuf::from(CACHE_DIR);
    // Use md5 hash for filename to avoid special characters
    let hash = format!("{:x}", md5::compute(key));
    path.push(hash);
    path
}

pub fn put_disk(key: &str, value: &[u8]) {
    if let Err(e) = fs::create_dir_all(CACHE_DIR) {
        eprintln!("[Cache] Failed to create cache dir: {}", e);
        return;
    }

    let path = get_cache_path(key);
    if let Err(e) = fs::write(&path, value) {
        eprintln!("[Cache] Failed to write cache file {:?}: {}", path, e);
    }
}

pub fn get_disk(key: &str) -> Option<Vec<u8>> {
    let path = get_cache_path(key);
    
    if !path.exists() {
        return None;
    }

    // Check expiration
    if let Ok(metadata) = fs::metadata(&path) {
        if let Ok(modified) = metadata.modified() {
            if let Ok(elapsed) = SystemTime::now().duration_since(modified) {
                if elapsed.as_secs() > IMAGE_CACHE_TTL {
                    // Expired
                    let _ = fs::remove_file(&path);
                    return None;
                }
            }
        }
    }

    match fs::read(&path) {
        Ok(data) => Some(data),
        Err(e) => {
            eprintln!("[Cache] Failed to read cache file {:?}: {}", path, e);
            None
        }
    }
}
