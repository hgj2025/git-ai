/// Handle the flush-logs command — no-op (third-party telemetry removed)
pub fn handle_flush_logs(_args: &[String]) {
    eprintln!("flush-logs: third-party telemetry has been removed.");
    std::process::exit(0);
}
