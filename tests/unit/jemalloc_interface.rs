#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jemalloc_availability() {
        let is_available = JemallocInterface::is_available();
        
        #[cfg(not(target_os = "windows"))]
        assert!(is_available);
        
        #[cfg(target_os = "windows")]
        assert!(!is_available);
    }

    #[test]
    fn test_jemalloc_stats() {
        if JemallocInterface::is_available() {
            // 测试获取已分配内存
            let allocated = JemallocInterface::get_allocated_bytes();
            assert!(allocated.is_ok());
            
            // 测试获取完整统计信息
            let stats = JemallocInterface::get_stats();
            assert!(stats.is_ok());
            
            if let Ok(stats) = stats {
                assert!(stats.allocated_bytes > 0);
                println!("Jemalloc stats: allocated={} MB, active={} MB", 
                    stats.allocated_bytes / 1024 / 1024,
                    stats.active_bytes / 1024 / 1024);
            }
        } else {
            // 在不支持的平台上，应该返回NotAvailable错误
            let allocated = JemallocInterface::get_allocated_bytes();
            assert!(matches!(allocated, Err(JemallocError::NotAvailable)));
        }
    }

    #[test]
    fn test_jemalloc_purge() {
        if JemallocInterface::is_available() {
            // 测试垃圾回收
            let result = JemallocInterface::purge_dirty_pages();
            assert!(result.is_ok());
            
            // 测试强制GC
            let gc_result = JemallocInterface::force_gc();
            assert!(gc_result.is_ok());
        } else {
            // 在不支持的平台上，应该返回NotAvailable错误
            let result = JemallocInterface::purge_dirty_pages();
            assert!(matches!(result, Err(JemallocError::NotAvailable)));
        }
    }

    #[test]
    fn test_config_validation() {
        let validation = JemallocInterface::validate_config();
        
        if JemallocInterface::is_available() {
            assert!(validation.is_ok());
        } else {
            assert!(matches!(validation, Err(JemallocError::NotAvailable)));
        }
    }
}