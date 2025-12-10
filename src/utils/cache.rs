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
