use crate::services::memory_service::MemoryManager;
use crate::config::settings::MemoryConfig;

#[tokio::test]
async fn test_memory_status_serialization() {
    let config = MemoryConfig {
        threshold_mb: 500,
        check_interval_secs: 30,
        gc_cooldown_secs: 30,
    };
    let manager = MemoryManager::new(config);

    // 获取内存状态
    let status = manager.get_memory_status().await;
    assert!(status.is_ok());

    if let Ok(status) = status {
        // 测试序列化为JSON
        let json_status = serde_json::json!({
            "current_memory_mb": status.current_mb,
            "threshold_mb": status.threshold_mb,
            "memory_pressure": match status.pressure {
                crate::services::memory_service::MemoryPressure::Low => "low",
                crate::services::memory_service::MemoryPressure::Medium => "medium",
                crate::services::memory_service::MemoryPressure::High => "high",
                crate::services::memory_service::MemoryPressure::Critical => "critical",
            },
            "memory_usage_percentage": (status.current_mb as f64 / status.threshold_mb as f64 * 100.0).round(),
            "time_since_last_gc_secs": status.time_since_last_gc_secs,
            "is_monitoring": status.is_monitoring,
        });

        println!("Memory status JSON: {}", serde_json::to_string_pretty(&json_status).unwrap());
        
        // 验证JSON结构
        assert!(json_status["current_memory_mb"].is_number());
        assert!(json_status["threshold_mb"].is_number());
        assert!(json_status["memory_pressure"].is_string());
        assert!(json_status["memory_usage_percentage"].is_number());
        assert!(json_status["time_since_last_gc_secs"].is_number());
        assert!(json_status["is_monitoring"].is_boolean());
    }
}

#[tokio::test]
async fn test_memory_pressure_levels() {
    let config = MemoryConfig {
        threshold_mb: 100, // 低阈值便于测试
        check_interval_secs: 30,
        gc_cooldown_secs: 30,
    };
    let manager = MemoryManager::new(config);

    // 测试不同内存使用量对应的压力等级
    let test_cases = vec![
        (30, "low"),      // 30% < 60%
        (70, "medium"),   // 70% 在 60%-80%
        (90, "high"),     // 90% 在 80%-100%
        (120, "critical"), // 120% > 100%
    ];

    for (usage_mb, expected_pressure) in test_cases {
        manager.update_memory_pressure(usage_mb).await;
        let status = manager.get_memory_status().await.unwrap();
        
        let pressure_str = match status.pressure {
            crate::services::memory_service::MemoryPressure::Low => "low",
            crate::services::memory_service::MemoryPressure::Medium => "medium",
            crate::services::memory_service::MemoryPressure::High => "high",
            crate::services::memory_service::MemoryPressure::Critical => "critical",
        };
        
        assert_eq!(pressure_str, expected_pressure, 
            "Memory usage {}MB should result in {} pressure", usage_mb, expected_pressure);
        
        println!("✓ {}MB usage -> {} pressure", usage_mb, pressure_str);
    }
}
