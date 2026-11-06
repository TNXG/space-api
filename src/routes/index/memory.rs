use crate::services::memory_service::MemoryManager;
use rocket::{get, State};
use std::sync::Arc;

// API 端点用于获取详细的内存性能报告
#[get("/api/memory/report")]
pub async fn get_memory_report(
    memory_manager: &State<Arc<MemoryManager>>,
) -> rocket::serde::json::Json<serde_json::Value> {
    match memory_manager.generate_memory_report().await {
        report => {
            rocket::serde::json::Json(serde_json::json!({
                "status": "success",
                "report": report
            }))
        }
    }
}

// API 端点用于获取内存使用趋势
#[get("/api/memory/trend")]
pub async fn get_memory_trend(
    memory_manager: &State<Arc<MemoryManager>>,
) -> rocket::serde::json::Json<serde_json::Value> {
    let trend = memory_manager.get_memory_trend().await;
    let avg_usage = memory_manager.calculate_average_memory_usage().await;
    let perf_stats = memory_manager.get_performance_stats().await;
    
    rocket::serde::json::Json(serde_json::json!({
        "status": "success",
        "data": {
            "trend_mb_per_hour": trend,
            "average_usage_mb": avg_usage,
            "monitoring_cycles": perf_stats.monitoring_cycles,
            "current_interval_seconds": perf_stats.current_dynamic_interval,
            "query_success_rate": if perf_stats.memory_query_success + perf_stats.memory_query_failures > 0 {
                (perf_stats.memory_query_success as f64 / (perf_stats.memory_query_success + perf_stats.memory_query_failures) as f64 * 100.0).round()
            } else { 100.0 }
        }
    }))
}
