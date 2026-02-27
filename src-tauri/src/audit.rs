//! JSONL audit logging for agent tool executions.
//!
//! Every tool the agent calls is logged as a single line in
//! `{app_config_dir}/agent-logs/YYYY-MM-DD.jsonl`. Best-effort — never
//! panics or fails the caller.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::Value;

#[derive(Serialize)]
struct ToolAuditEntry<'a> {
    ts: u64,
    conversation_id: Option<&'a str>,
    tool: &'a str,
    input: &'a Value,
    ok: bool,
    message: &'a str,
    duration_ms: u64,
}

/// Log a single tool execution to today's JSONL audit file.
///
/// This is best-effort: failures are silently ignored so they never
/// affect the caller's control flow.
pub fn log_tool_call(
    app_config_dir: &Path,
    conversation_id: Option<&str>,
    tool: &str,
    input: &Value,
    result: Result<&str, &str>,
    duration: Duration,
) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let (ok, message) = match result {
        Ok(msg) => (true, msg),
        Err(e) => (false, e),
    };

    let entry = ToolAuditEntry {
        ts: now,
        conversation_id,
        tool,
        input,
        ok,
        message,
        duration_ms: u64::try_from(duration.as_millis()).unwrap_or(u64::MAX),
    };

    let dir = crate::paths::agent_logs_dir(app_config_dir);
    let _ = fs::create_dir_all(&dir);

    let filename = format!("{}.jsonl", date_from_epoch(now));
    let path = dir.join(filename);

    if let Ok(json) = serde_json::to_string(&entry) {
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
            let _ = writeln!(file, "{json}");
        }
    }
}

/// Format epoch seconds as `YYYY-MM-DD` without external deps.
#[allow(clippy::unreadable_literal, clippy::cast_possible_wrap, clippy::cast_sign_loss)]
fn date_from_epoch(epoch_secs: u64) -> String {
    // Civil date from day count (algorithm from Howard Hinnant)
    let days = (epoch_secs / 86400) as i64;
    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = (z - era * 146097) as u64; // day of era [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_date_from_epoch() {
        // 2025-02-24 00:00:00 UTC = 1740355200
        assert_eq!(date_from_epoch(1_740_355_200), "2025-02-24");
        // Unix epoch
        assert_eq!(date_from_epoch(0), "1970-01-01");
        // 2000-01-01 00:00:00 UTC = 946684800
        assert_eq!(date_from_epoch(946_684_800), "2000-01-01");
        // End of day rounds correctly
        assert_eq!(date_from_epoch(1_740_355_200 + 86399), "2025-02-24");
    }

    #[test]
    fn test_log_tool_call_no_panic() {
        // Logging to a non-existent dir should silently fail, not panic
        let bogus = Path::new("/tmp/vibelights-test-nonexistent-1234567890");
        let input = serde_json::json!({"key": "value"});
        log_tool_call(
            bogus,
            Some("conv-1"),
            "test_tool",
            &input,
            Ok("ok"),
            Duration::from_millis(5),
        );
        // No panic = pass
    }
}
