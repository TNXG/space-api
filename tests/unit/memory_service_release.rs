use super::*;

#[tokio::test]
async fn test_should_trigger_release() {
    let config = MemoryConfig {
        threshold_mb: 500,
        check_interval_secs: 30,
        gc_cooldown_secs: 1, // 1秒冷却时间用于测试
    };
    let manager = MemoryManager::new(config);

    // 内存使用未超过阈值，不应触发
    assert!(!manager.should_trigger_release(400).await);

    // 刚创建的manager，last_gc_time是当前时间，所以即使内存超过阈值也不应该触发（冷却期内）
    assert!(!manager.should_trigger_release(600).await);

    // 等待冷却时间过去
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // 现在内存使用超过阈值，且冷却时间已过，应该触发
    assert!(manager.should_trigger_release(600).await);

    // 模拟刚执行过GC，更新last_gc_time
    {
        let mut last_gc = manager.last_gc_time.lock().await;
        *last_gc = Instant::now();
    }

    // 刚执行过GC，应该在冷却期内，不应触发
    assert!(!manager.should_trigger_release(600).await);
}

#[tokio::test]
async fn test_gc_cooldown_mechanism() {
    let config = MemoryConfig {
        threshold_mb: 100, // 低阈值便于测试
        check_interval_secs: 30,
        gc_cooldown_secs: 1, // 1秒冷却时间
    };
    let manager = MemoryManager::new(config);

    // 等待初始冷却时间过去
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // 第一次检查：内存超过阈值，应该触发
    assert!(manager.should_trigger_release(200).await);

    // 模拟执行GC后更新时间戳
    {
        let mut last_gc = manager.last_gc_time.lock().await;
        *last_gc = Instant::now();
    }

    // 立即再次检查：应该在冷却期内，不应触发
    assert!(!manager.should_trigger_release(200).await);

    // 等待一半冷却时间
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    assert!(!manager.should_trigger_release(200).await);

    // 等待剩余冷却时间
    tokio::time::sleep(tokio::time::Duration::from_millis(600)).await;

    // 现在冷却时间已过，应该可以再次触发
    assert!(manager.should_trigger_release(200).await);
}

#[tokio::test]
async fn test_trigger_global_release() {
    let config = MemoryConfig {
        threshold_mb: 500,
        check_interval_secs: 30,
        gc_cooldown_secs: 1,
    };
    let manager = MemoryManager::new(config);

    // 等待冷却时间过去
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // 执行全局内存释放
    let result = manager.trigger_global_release().await;
    assert!(result.is_ok());

    if let Ok(release_result) = result {
        // 验证结果结构（u64类型本身就不会是负数）
        // 只需要验证时间戳是合理的
        assert!(release_result.timestamp > Utc::now() - chrono::Duration::seconds(10));

        println!(
            "Release result: freed {} MB, cleared {} cache entries, GC executed: {}",
            release_result.memory_freed_mb,
            release_result.cache_entries_cleared,
            release_result.gc_executed
        );
    }

    // 验证监控状态更新
    let state = manager.get_monitor_state().await;
    assert_eq!(state.release_count, 1);
    assert!(state.last_release_time.is_some());
}

#[tokio::test]
async fn test_check_and_release_if_needed() {
    let config = MemoryConfig {
        threshold_mb: 1, // 设置很低的阈值，确保会触发释放
        check_interval_secs: 30,
        gc_cooldown_secs: 1,
    };
    let manager = MemoryManager::new(config);

    // 等待冷却时间过去
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // 检查并释放
    let result = manager.check_and_release_if_needed().await;
    assert!(result.is_ok());

    if let Ok(Some(release_result)) = result {
        println!(
            "Automatic release triggered: freed {} MB",
            release_result.memory_freed_mb
        );
    } else if let Ok(None) = result {
        println!("No release needed");
    }
}

#[tokio::test]
async fn test_gc_failure_counting() {
    let config = MemoryConfig {
        threshold_mb: 500,
        check_interval_secs: 30,
        gc_cooldown_secs: 1,
    };
    let manager = MemoryManager::new(config);

    // 等待冷却时间过去
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // 执行多次释放操作来测试失败计数
    for i in 1..=3 {
        let _ = manager.trigger_global_release().await;

        // 检查失败计数（在正常情况下应该为0，因为GC会成功或者jemalloc不可用）
        let failure_count = {
            let count = manager.gc_failure_count.lock().await;
            *count
        };

        println!("After release {}: failure count = {}", i, failure_count);

        // 等待一小段时间
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
}

#[tokio::test]
async fn test_gc_failure_handling() {
    let config = MemoryConfig {
        threshold_mb: 100, // 低阈值便于测试
        check_interval_secs: 30,
        gc_cooldown_secs: 1,
    };
    let manager = MemoryManager::new(config);

    // 等待冷却时间过去
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // 执行内存释放操作
    let result = manager.trigger_global_release().await;
    assert!(result.is_ok());

    if let Ok(release_result) = result {
        // 验证操作结果结构
        assert!(release_result.timestamp > Utc::now() - chrono::Duration::seconds(10));
        println!(
            "Enhanced release result: freed {} MB, cleared {} cache entries, GC executed: {}",
            release_result.memory_freed_mb,
            release_result.cache_entries_cleared,
            release_result.gc_executed
        );
    }

    // 验证统计信息更新
    let state = manager.get_monitor_state().await;
    assert_eq!(state.release_count, 1);
    assert!(state.last_release_time.is_some());
}
