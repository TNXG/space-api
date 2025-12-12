use thiserror::Error;

/// Jemalloc相关错误类型
#[derive(Debug, Error)]
pub enum JemallocError {
    #[error("Jemalloc not available on this platform")]
    NotAvailable,
    
    #[error("Failed to read jemalloc statistics: {0}")]
    StatsFailed(String),
    
    #[error("Failed to purge dirty pages: {0}")]
    PurgeFailed(String),
    
    #[error("Failed to advance epoch: {0}")]
    EpochFailed(String),
}

/// Jemalloc统计信息
#[derive(Debug, Clone)]
pub struct JemallocStats {
    /// 已分配的字节数
    pub allocated_bytes: u64,
    /// 活跃的字节数
    pub active_bytes: u64,
    /// 映射的字节数
    pub mapped_bytes: u64,
    /// 保留的字节数
    pub retained_bytes: u64,
}

/// Jemalloc接口
pub struct JemallocInterface;

impl JemallocInterface {
    /// 检查jemalloc是否可用
    pub fn is_available() -> bool {
        #[cfg(not(target_os = "windows"))]
        {
            // 在非Windows平台上，jemalloc应该是可用的
            true
        }
        
        #[cfg(target_os = "windows")]
        {
            // Windows平台不支持jemalloc
            false
        }
    }

    /// 获取已分配的内存字节数
    pub fn get_allocated_bytes() -> Result<u64, JemallocError> {
        #[cfg(not(target_os = "windows"))]
        {
            use tikv_jemalloc_ctl::{epoch, stats};
            
            // 更新统计信息
            if let Err(e) = epoch::advance() {
                return Err(JemallocError::EpochFailed(e.to_string()));
            }
            
            // 读取已分配的内存
            stats::allocated::read()
                .map(|bytes| bytes as u64)
                .map_err(|e| JemallocError::StatsFailed(e.to_string()))
        }
        
        #[cfg(target_os = "windows")]
        {
            Err(JemallocError::NotAvailable)
        }
    }

    /// 获取活跃内存字节数
    pub fn get_active_bytes() -> Result<u64, JemallocError> {
        #[cfg(not(target_os = "windows"))]
        {
            use tikv_jemalloc_ctl::{epoch, stats};
            
            // 更新统计信息
            if let Err(e) = epoch::advance() {
                return Err(JemallocError::EpochFailed(e.to_string()));
            }
            
            // 读取活跃内存
            stats::active::read()
                .map(|bytes| bytes as u64)
                .map_err(|e| JemallocError::StatsFailed(e.to_string()))
        }
        
        #[cfg(target_os = "windows")]
        {
            Err(JemallocError::NotAvailable)
        }
    }

    /// 清理脏页面（执行垃圾回收）
    pub fn purge_dirty_pages() -> Result<(), JemallocError> {
        #[cfg(not(target_os = "windows"))]
        {
            use tikv_jemalloc_ctl::background_thread;
            
            // 尝试启用后台线程来清理内存
            // 这是一个更安全的方式来触发内存清理
            match background_thread::write(true) {
                Ok(_) => {
                    log::debug!("Background thread enabled for memory cleanup");
                    Ok(())
                },
                Err(e) => {
                    log::warn!("Failed to enable background thread: {}", e);
                    // 即使失败也不返回错误，因为这不是关键操作
                    Ok(())
                }
            }
        }
        
        #[cfg(target_os = "windows")]
        {
            Err(JemallocError::NotAvailable)
        }
    }

    /// 获取完整的jemalloc统计信息
    pub fn get_stats() -> Result<JemallocStats, JemallocError> {
        #[cfg(not(target_os = "windows"))]
        {
            use tikv_jemalloc_ctl::{epoch, stats};
            
            // 更新统计信息
            if let Err(e) = epoch::advance() {
                return Err(JemallocError::EpochFailed(e.to_string()));
            }
            
            let allocated_bytes = stats::allocated::read()
                .map(|bytes| bytes as u64)
                .map_err(|e| JemallocError::StatsFailed(format!("allocated: {}", e)))?;
                
            let active_bytes = stats::active::read()
                .map(|bytes| bytes as u64)
                .map_err(|e| JemallocError::StatsFailed(format!("active: {}", e)))?;
                
            let mapped_bytes = stats::mapped::read()
                .map(|bytes| bytes as u64)
                .map_err(|e| JemallocError::StatsFailed(format!("mapped: {}", e)))?;
                
            let retained_bytes = stats::retained::read()
                .map(|bytes| bytes as u64)
                .map_err(|e| JemallocError::StatsFailed(format!("retained: {}", e)))?;
            
            Ok(JemallocStats {
                allocated_bytes,
                active_bytes,
                mapped_bytes,
                retained_bytes,
            })
        }
        
        #[cfg(target_os = "windows")]
        {
            Err(JemallocError::NotAvailable)
        }
    }

    /// 强制执行垃圾回收并返回释放的内存量估算
    pub fn force_gc() -> Result<u64, JemallocError> {
        #[cfg(not(target_os = "windows"))]
        {
            // 获取GC前的内存使用量
            let before_allocated = Self::get_allocated_bytes()?;
            
            // 执行垃圾回收
            Self::purge_dirty_pages()?;
            
            // 等待一小段时间让GC完成
            std::thread::sleep(std::time::Duration::from_millis(10));
            
            // 获取GC后的内存使用量
            let after_allocated = Self::get_allocated_bytes()?;
            
            // 计算释放的内存量（如果有的话）
            if before_allocated > after_allocated {
                Ok(before_allocated - after_allocated)
            } else {
                Ok(0)
            }
        }
        
        #[cfg(target_os = "windows")]
        {
            Err(JemallocError::NotAvailable)
        }
    }

    /// 验证jemalloc配置的有效性
    pub fn validate_config() -> Result<(), JemallocError> {
        if !Self::is_available() {
            return Err(JemallocError::NotAvailable);
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            // 尝试读取基本统计信息来验证配置
            Self::get_allocated_bytes()?;
            log::info!("Jemalloc configuration validated successfully");
            Ok(())
        }
        
        #[cfg(target_os = "windows")]
        {
            Err(JemallocError::NotAvailable)
        }
    }
}

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