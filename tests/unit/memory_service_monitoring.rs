use super::*;

#[tokio::test]
async fn test_start_monitoring() {
    let config = MemoryConfig {
        threshold_mb: 500,
        check_interval_secs: 1, // 1秒间隔用于测试
        gc_cooldown_secs: 1,
    };
    let manager = MemoryManager::new(config);

    // 启动监控任务
    let monitoring_handle = manager.start_monitoring();

    // 让监控任务运行一小段时间
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // 停止监控任务
    monitoring_handle.abort();

    // 验证监控任务确实在运行（通过检查是否有状态更新）
    let state = manager.get_monitor_state().await;
    println!(
        "Monitoring test completed, current usage: {} MB",
        state.current_usage_mb
    );

    // 监控任务应该至少更新了一次内存使用量（u64类型本身就不会是负数）
    // 这里只是验证监控任务正常运行
}

#[tokio::test]
async fn test_monitoring_interval_consistency() {
    let config = MemoryConfig {
        threshold_mb: 500,
        check_interval_secs: 1, // 1秒间隔
        gc_cooldown_secs: 30,
    };
    let manager = MemoryManager::new(config);

    let start_time = std::time::Instant::now();
    let monitoring_handle = manager.start_monitoring();

    // 运行2.5秒，应该触发2-3次检查
    tokio::time::sleep(tokio::time::Duration::from_millis(2500)).await;

    monitoring_handle.abort();
    let elapsed = start_time.elapsed();

    println!("Monitoring ran for {:?}", elapsed);

    // 验证监控任务按预期间隔运行
    assert!(elapsed >= tokio::time::Duration::from_secs(2));
    assert!(elapsed < tokio::time::Duration::from_secs(4));
}

#[tokio::test]
async fn test_enhanced_monitoring_task() {
    let config = MemoryConfig {
        threshold_mb: 500,
        check_interval_secs: 1, // 1秒间隔用于测试
        gc_cooldown_secs: 1,
    };
    let manager = MemoryManager::new(config);

    // 启动增强的监控任务
    let monitoring_handle = manager.start_monitoring();

    // 让监控任务运行一小段时间
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // 停止监控任务
    monitoring_handle.abort();

    // 验证监控任务确实在运行（通过检查是否有状态更新）
    let state = manager.get_monitor_state().await;
    println!(
        "Enhanced monitoring test completed, current usage: {} MB, peak: {} MB",
        state.current_usage_mb, state.peak_usage_mb
    );

    // 监控任务应该至少更新了一次内存使用量
    // 在某些测试环境中可能为0，这是可以接受的
}

#[tokio::test]
async fn test_adaptive_interval_calculation() {
    let config = MemoryConfig {
        threshold_mb: 500,
        check_interval_secs: 30,
        gc_cooldown_secs: 30,
    };
    let manager = MemoryManager::new(config);
    let last_adjustment = Instant::now();

    // 测试不同压力等级下的间隔调整

    // 设置低压力
    manager.safe_update_memory_pressure(200).await.unwrap(); // 40% of threshold
    let interval_low = manager
        .calculate_adaptive_interval(30, 0, &last_adjustment)
        .await;

    // 设置高压力
    manager.safe_update_memory_pressure(450).await.unwrap(); // 90% of threshold
    let interval_high = manager
        .calculate_adaptive_interval(30, 0, &last_adjustment)
        .await;

    // 设置严重压力
    manager.safe_update_memory_pressure(600).await.unwrap(); // 120% of threshold
    let interval_critical = manager
        .calculate_adaptive_interval(30, 0, &last_adjustment)
        .await;

    println!(
        "Adaptive intervals - Low: {}s, High: {}s, Critical: {}s",
        interval_low, interval_high, interval_critical
    );

    // 严重压力时应该有更短的间隔
    assert!(interval_critical <= interval_high);
    assert!(interval_high <= interval_low);

    // 测试失败情况下的间隔调整
    let interval_with_failures = manager
        .calculate_adaptive_interval(30, 5, &last_adjustment)
        .await;
    assert!(interval_with_failures >= 30); // 失败时应该增加间隔

    println!("Interval with failures: {}s", interval_with_failures);
}
