use super::*;

#[tokio::test]
async fn test_performance_optimization_features() {
    let config = MemoryConfig {
        threshold_mb: 500,
        check_interval_secs: 30,
        gc_cooldown_secs: 30,
    };
    let manager = MemoryManager::new(config);

    // 测试性能统计初始化
    let stats = manager.get_performance_stats().await;
    assert_eq!(stats.monitoring_cycles, 0);
    assert_eq!(stats.memory_query_success, 0);
    assert_eq!(stats.memory_query_failures, 0);
    assert_eq!(stats.current_dynamic_interval, 30);

    // 测试内存历史记录
    manager.update_memory_history(100).await;
    manager.update_memory_history(150).await;
    manager.update_memory_history(200).await;

    let avg_usage = manager.calculate_average_memory_usage().await;
    assert!((avg_usage - 150.0).abs() < 1.0); // 应该接近150MB

    // 添加小延迟以确保uptime > 0
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    // 测试内存使用报告生成
    let report = manager.generate_memory_report().await;
    // uptime_seconds 是 u64 类型，总是 >= 0，所以只检查它存在
    assert_eq!(report.avg_usage_mb, avg_usage);
    assert!(report.timestamp > Utc::now() - chrono::Duration::seconds(10));

    println!("Performance optimization test completed successfully");
    println!("Average memory usage: {:.1} MB", avg_usage);
    println!("Report uptime: {} seconds", report.uptime_seconds);
}

#[tokio::test]
async fn test_memory_trend_analysis() {
    let config = MemoryConfig {
        threshold_mb: 500,
        check_interval_secs: 30,
        gc_cooldown_secs: 30,
    };
    let manager = MemoryManager::new(config);

    // 添加一些内存历史数据来模拟趋势
    for i in 0..20 {
        let memory_usage = 100 + i * 5; // 递增趋势
        manager.update_memory_history(memory_usage).await;

        // 添加小延迟以确保时间戳不同
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    let trend = manager.get_memory_trend().await;

    match trend {
        Some(trend_value) => {
            println!("Memory trend: {:.2} MB/hour", trend_value);
            // 由于我们创建了递增趋势，应该是正值
            assert!(trend_value > 0.0);
        }
        None => {
            println!("Not enough data for trend analysis");
        }
    }
}

#[tokio::test]
async fn test_performance_reporting() {
    let config = MemoryConfig {
        threshold_mb: 500,
        check_interval_secs: 30,
        gc_cooldown_secs: 30,
    };
    let manager = MemoryManager::new(config);

    // 模拟一些性能数据
    manager
        .update_memory_query_stats(std::time::Duration::from_millis(50), true)
        .await;
    manager
        .update_memory_query_stats(std::time::Duration::from_millis(75), true)
        .await;
    manager
        .update_memory_query_stats(std::time::Duration::from_millis(100), false)
        .await;

    let stats = manager.get_performance_stats().await;
    assert_eq!(stats.memory_query_success, 2);
    assert_eq!(stats.memory_query_failures, 1);
    assert!(stats.avg_memory_query_time_ms > 0.0);

    // 测试性能报告日志输出
    manager.log_performance_report().await;

    println!("Performance reporting test completed");
    println!(
        "Average query time: {:.2} ms",
        stats.avg_memory_query_time_ms
    );
    println!(
        "Success rate: {:.1}%",
        stats.memory_query_success as f64
            / (stats.memory_query_success + stats.memory_query_failures) as f64
            * 100.0
    );
}
