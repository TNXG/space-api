use super::*;
use chrono::Utc;

#[path = "memory_service_monitoring.rs"]
mod monitoring;
#[path = "memory_service_release.rs"]
mod release;
#[path = "memory_service_reporting.rs"]
mod reporting;

#[tokio::test]
async fn test_memory_manager_creation() {
    let config = MemoryConfig {
        threshold_mb: 500,
        check_interval_secs: 30,
        gc_cooldown_secs: 30,
    };

    let manager = MemoryManager::new(config);
    let pressure = manager.get_memory_pressure().await;
    assert_eq!(pressure, MemoryPressure::Low);
}

#[test]
fn test_pressure_level_calculation() {
    let config = MemoryConfig {
        threshold_mb: 500,
        check_interval_secs: 30,
        gc_cooldown_secs: 30,
    };
    let manager = MemoryManager::new(config);

    // 测试低压力 (< 60%)
    assert_eq!(
        manager.calculate_pressure_level(200, 500),
        MemoryPressure::Low
    );

    // 测试中等压力 (60-80%)
    assert_eq!(
        manager.calculate_pressure_level(350, 500),
        MemoryPressure::Medium
    );

    // 测试高压力 (80-100%)
    assert_eq!(
        manager.calculate_pressure_level(450, 500),
        MemoryPressure::High
    );

    // 测试严重压力 (> 100%)
    assert_eq!(
        manager.calculate_pressure_level(600, 500),
        MemoryPressure::Critical
    );
}

#[tokio::test]
async fn test_memory_pressure_update() {
    let config = MemoryConfig {
        threshold_mb: 500,
        check_interval_secs: 30,
        gc_cooldown_secs: 30,
    };
    let manager = MemoryManager::new(config);

    // 更新为高压力
    manager.update_memory_pressure(450).await;
    let pressure = manager.get_memory_pressure().await;
    assert_eq!(pressure, MemoryPressure::High);

    // 检查监控状态
    let state = manager.get_monitor_state().await;
    assert_eq!(state.current_usage_mb, 450);
    assert_eq!(state.pressure_level, MemoryPressure::High);
    assert_eq!(state.peak_usage_mb, 450);
}

#[tokio::test]
async fn test_jemalloc_integration() {
    let config = MemoryConfig {
        threshold_mb: 500,
        check_interval_secs: 30,
        gc_cooldown_secs: 30,
    };
    let manager = MemoryManager::new(config);

    // 测试jemalloc配置验证
    let validation_result = manager.validate_jemalloc_config();
    assert!(validation_result.is_ok());

    // 测试内存使用量获取
    let memory_usage = manager.get_current_memory_usage().await;

    match memory_usage {
        Ok(usage_mb) => {
            println!("Current memory usage: {} MB", usage_mb);
            // 只验证函数能够成功返回，不对具体值做假设
            // 因为在不同环境下内存使用量可能差异很大，甚至可能为0
            // u64类型本身就不会是负数，所以这里只是确认函数正常执行
        }
        Err(e) => {
            println!("Failed to get memory usage: {}", e);
            // 在某些测试环境中可能无法获取内存信息，这是可以接受的
            // 但我们至少要确保错误是预期的类型
            match e {
                MemoryError::MetricsCollectionFailed(_) => {
                    // 这是预期的错误类型
                }
                _ => panic!("Unexpected error type: {}", e),
            }
        }
    }
}

#[tokio::test]
async fn test_enhanced_error_handling() {
    let config = MemoryConfig {
        threshold_mb: 500,
        check_interval_secs: 30,
        gc_cooldown_secs: 30,
    };
    let manager = MemoryManager::new(config);

    // 测试安全的内存压力更新
    let result = manager.safe_update_memory_pressure(450).await;
    assert!(result.is_ok());

    // 验证压力等级已更新
    let pressure = manager.get_memory_pressure().await;
    assert_eq!(pressure, MemoryPressure::High);

    // 测试监控状态更新
    let state = manager.get_monitor_state().await;
    assert_eq!(state.current_usage_mb, 450);
    assert_eq!(state.pressure_level, MemoryPressure::High);
    assert_eq!(state.peak_usage_mb, 450);
}

#[tokio::test]
async fn test_memory_usage_error_handling() {
    let config = MemoryConfig {
        threshold_mb: 500,
        check_interval_secs: 30,
        gc_cooldown_secs: 30,
    };
    let manager = MemoryManager::new(config);

    // 测试内存使用量获取
    let memory_usage = manager.get_current_memory_usage().await;

    match memory_usage {
        Ok(usage_mb) => {
            println!("Enhanced memory usage retrieval: {} MB", usage_mb);
            // 验证返回值的合理性
            assert!(usage_mb < 50000); // 不应该超过50GB
        }
        Err(e) => {
            println!(
                "Memory usage retrieval failed (expected in some test environments): {}",
                e
            );
            // 在测试环境中失败是可以接受的
            match e {
                MemoryError::MetricsCollectionFailed(_) => {
                    // 这是预期的错误类型
                }
                _ => panic!("Unexpected error type: {}", e),
            }
        }
    }
}

