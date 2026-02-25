use log::{debug, error, info};
use moka::future::Cache;
use once_cell::sync::Lazy;
use std::time::Duration;

// 创建一个全局的轻量级缓存实例（只缓存小数据，如元数据、配置等）
pub static CACHE_BUCKET: Lazy<Cache<String, Vec<u8>>> = Lazy::new(|| {
    Cache::builder()
        .time_to_live(Duration::from_secs(12 * 60 * 60)) // 12小时刷新全部缓存
        .time_to_idle(Duration::from_secs(2 * 60 * 60)) // 2小时不访问则失效
        .weigher(|_key, value: &Vec<u8>| -> u32 {
            // 限制单个缓存项最大1MB，超过则不缓存到内存
            if value.len() > 1024 * 1024 {
                u32::MAX // 拒绝缓存大文件
            } else {
                value.len() as u32
            }
        })
        .max_capacity(50 * 1024 * 1024) // 最大50MB内存缓存（按 weigher 权重计算）
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
            error!("Failed to create cache dir {:?}: {}", parent, e);
            return;
        }
    }

    // 直接写入，不限制缓存次数
    if let Err(e) = fs::write(&path, value) {
        error!("Failed to write cache file {:?}: {}", path, e);
    } else {
        debug!("Cached to disk: {} bytes -> {:?}", value.len(), path);
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
                debug!("Expired cache removed: {:?}", path);
                return None;
            }
        }
    }

    match fs::read(&path) {
        Ok(data) => {
            debug!("Disk cache hit: {} bytes from {:?}", data.len(), path);
            Some(data)
        },
        Err(e) => {
            error!("Cache read failed {:?}: {}", path, e);
            None
        }
    }
}

/// 不由通用清理任务管理的目录（有独立缓存策略）
const CACHE_EXCLUDED_DIRS: &[&str] = &["friend_avatars"];

// 清理过期的缓存文件（统计在清理过程中直接收集，避免额外的目录扫描）
pub fn cleanup_expired_cache() {
    use std::fs;
    use std::path::Path;

    struct CleanupStats {
        removed_count: usize,
        removed_size: u64,
        remaining_count: usize,
        remaining_size: u64,
    }

    fn cleanup_dir(dir: &Path, stats: &mut CleanupStats) -> std::io::Result<()> {
        if !dir.exists() {
            return Ok(());
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                // 跳过有独立缓存策略的目录
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if CACHE_EXCLUDED_DIRS.contains(&name) {
                        debug!("Skipping excluded cache dir: {:?}", path);
                        continue;
                    }
                }
                cleanup_dir(&path, stats)?;
                // 尝试删除空目录
                let _ = fs::remove_dir(&path);
            } else if path.is_file() {
                if let Ok(metadata) = fs::metadata(&path) {
                    let file_size = metadata.len();
                    let mut expired = false;
                    if let Ok(modified) = metadata.modified() {
                        if let Ok(elapsed) = SystemTime::now().duration_since(modified) {
                            if elapsed.as_secs() > IMAGE_CACHE_TTL {
                                expired = true;
                            }
                        }
                    }
                    if expired {
                        let _ = fs::remove_file(&path);
                        stats.removed_count += 1;
                        stats.removed_size += file_size;
                        debug!("Cleaned expired cache file: {:?}", path);
                    } else {
                        stats.remaining_count += 1;
                        stats.remaining_size += file_size;
                    }
                }
            }
        }
        Ok(())
    }

    let cache_dir = Path::new(CACHE_DIR);
    let mut stats = CleanupStats {
        removed_count: 0,
        removed_size: 0,
        remaining_count: 0,
        remaining_size: 0,
    };

    if let Err(e) = cleanup_dir(cache_dir, &mut stats) {
        error!("Failed to cleanup cache directory: {}", e);
    } else {
        if stats.removed_count > 0 {
            info!("Cache cleanup completed: removed {} files, freed {} bytes",
                    stats.removed_count, stats.removed_size);
        }

        debug!("Cache stats: {} files, {} bytes total",
                stats.remaining_count, stats.remaining_size);
    }
}