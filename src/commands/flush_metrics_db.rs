//! Handle flush-metrics-db command — no-op (third-party telemetry removed)

/// Spawn a background process to flush metrics DB — no-op
pub fn spawn_background_metrics_db_flush() {}

/// Handle the flush-metrics-db command — no-op
pub fn handle_flush_metrics_db(_args: &[String]) {
    eprintln!("flush-metrics-db: third-party telemetry has been removed.");
}
