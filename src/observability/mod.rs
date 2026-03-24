use std::collections::HashMap;
use std::time::Duration;

use crate::metrics::MetricEvent;

pub mod flush;
pub mod wrapper_performance_targets;

/// Maximum events per metrics envelope
pub const MAX_METRICS_PER_ENVELOPE: usize = 250;

/// Log an error — no-op (third-party telemetry removed)
pub fn log_error(_error: &dyn std::error::Error, _context: Option<serde_json::Value>) {}

/// Log a performance metric — no-op (third-party telemetry removed)
pub fn log_performance(
    _operation: &str,
    _duration: Duration,
    _context: Option<serde_json::Value>,
    _tags: Option<HashMap<String, String>>,
) {
}

/// Log a message — no-op (third-party telemetry removed)
#[allow(dead_code)]
pub fn log_message(_message: &str, _level: &str, _context: Option<serde_json::Value>) {}

/// Spawn a background flush — no-op (third-party telemetry removed)
pub fn spawn_background_flush() {}

/// Log metric events — no-op (third-party telemetry removed)
pub fn log_metrics(
    #[allow(unused)] _events: Vec<MetricEvent>,
) {
}
