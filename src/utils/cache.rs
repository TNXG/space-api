use moka::future::Cache;
use once_cell::sync::Lazy;
use std::time::Duration;

// 创建一个全局的轻量级缓存实例（只缓存小数据，如元数据、配置等）
pub static CACHE_BUCKET: Lazy<Cache<String, Vec<u8>>> = Lazy::new(|| {
    Cache::builder()
        .time_to_live(Duration::from_secs(12 * 60 * 60)) // 12小时刷新全部缓存
        .time_to_idle(Duration::from_secs(2 * 60 * 60)) // 2小时不访问则失效
        .max_capacity(100) // 减少到100个项目，避免大图片占用过多内存
        .weigher(|_key, value: &Vec<u8>| -> u32 {
            // 限制单个缓存项最大1MB，超过则不缓存到内存
            if value.len() > 1024 * 1024 {
                u32::MAX // 拒绝缓存大文件
            } else {
                value.len() as u32
            }
        })
        .max_capacity(50 * 1024 * 1024) // 最大50MB内存缓存
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
use sha2::{Sha256, Digest};

const CACHE_DIR: &str = "cache";
const IMAGE_CACHE_TTL: u64 = 30; // 30 seconds

fn get_cache_path(key: &str) -> PathBuf {
    let mut path = PathBuf::from(CACHE_DIR);
    
    // 使用SHA256哈希，更安全且避免特殊字符
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    
    // 创建两级目录结构，避免单个目录文件过多
    let (dir1, dir2) = hash.split_at(2);
    let (dir2, filename) = dir2.split_at(2);
    
    path.push(dir1);
    path.push(dir2);
    path.push(filename);
    path
}

pub fn put_disk(key: &str, value: &[u8]) {
    let path = get_cache_path(key);
    
    // 硬盘缓存允许无限次缓存，不检查数量限制
    // 创建必要的父目录
    if let Some(parent) = path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            eprintln!("[Cache] Failed to create cache dir {:?}: {}", parent, e);
            return;
        }
    }

    // 直接写入，不限制缓存次数
    if let Err(e) = fs::write(&path, value) {
        eprintln!("[Cache] Failed to write cache file {:?}: {}", path, e);
    } else {
        println!("[Cache] Cached to disk: {} bytes -> {:?}", value.len(), path);
    }
}

/// 从硬盘缓存读取数据
/// 
/// 内存优化：预分配精确大小的缓冲区，避免多次扩容
pub fn get_disk(key: &str) -> Option<Vec<u8>> {
    let path = get_cache_path(key);
    
    if !path.exists() {
        return None;
    }

    // 获取元数据检查过期和文件大小
    let metadata = match fs::metadata(&path) {
        Ok(m) => m,
        Err(_) => return None,
    };

    // 检查过期
    if let Ok(modified) = metadata.modified() {
        if let Ok(elapsed) = SystemTime::now().duration_since(modified) {
            if elapsed.as_secs() > IMAGE_CACHE_TTL {
                let _ = fs::remove_file(&path);
                println!("[Cache] Expired cache removed: {:?}", path);
                return None;
            }
        }
    }

    match fs::read(&path) {
        Ok(data) => {
            println!("[Cache] Disk hit: {} bytes from {:?}", data.len(), path);
            Some(data)
        },
        Err(e) => {
            eprintln!("[Cache] Read failed {:?}: {}", path, e);
            None
        }
    }
}

// 获取硬盘缓存统计信息
fn get_disk_cache_stats() -> (usize, u64) {
    use std::fs;
    use std::path::Path;
    
    let mut file_count = 0;
    let mut total_size = 0u64;
    
    fn scan_dir(dir: &Path, file_count: &mut usize, total_size: &mut u64) -> std::io::Result<()> {
        if !dir.exists() {
            return Ok(());
        }
        
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_dir() {
                scan_dir(&path, file_count, total_size)?;
            } else if path.is_file() {
                *file_count += 1;
                if let Ok(metadata) = fs::metadata(&path) {
                    *total_size += metadata.len();
                }
            }
        }
        Ok(())
    }
    
    let cache_dir = Path::new(CACHE_DIR);
    let _ = scan_dir(cache_dir, &mut file_count, &mut total_size);
    
    (file_count, total_size)
}

// 清理过期的缓存文件
pub fn cleanup_expired_cache() {
    use std::fs;
    use std::path::Path;
    
    fn cleanup_dir(dir: &Path) -> std::io::Result<()> {
        if !dir.exists() {
            return Ok(());
        }
        
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_dir() {
                cleanup_dir(&path)?;
                // 尝试删除空目录
                let _ = fs::remove_dir(&path);
            } else if path.is_file() {
                if let Ok(metadata) = fs::metadata(&path) {
                    if let Ok(modified) = metadata.modified() {
                        if let Ok(elapsed) = SystemTime::now().duration_since(modified) {
                            if elapsed.as_secs() > IMAGE_CACHE_TTL {
                                let _ = fs::remove_file(&path);
                                println!("[Cache] Cleaned expired cache file: {:?}", path);
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
    
    let cache_dir = Path::new(CACHE_DIR);
    let (before_count, before_size) = get_disk_cache_stats();
    
    if let Err(e) = cleanup_dir(cache_dir) {
        eprintln!("[Cache] Failed to cleanup cache directory: {}", e);
    } else {
        let (after_count, after_size) = get_disk_cache_stats();
        let cleaned_count = before_count.saturating_sub(after_count);
        let cleaned_size = before_size.saturating_sub(after_size);
        
        if cleaned_count > 0 {
            println!("[Cache] Cleanup completed: removed {} files, freed {} bytes", 
                    cleaned_count, cleaned_size);
        }
        
        println!("[Cache] Current stats: {} files, {} bytes total", 
                after_count, after_size);
    }
}