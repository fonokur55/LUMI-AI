// #region agent log helper (debug session 0e7da8)
//! Egyszerű NDJSON debug logger - kizárólag a debug session idejére.
//! A teljes fájlt EL KELL TÁVOLÍTANI a debug munkamenet végén.

use serde_json::Value;
use std::io::Write;

const LOG_PATH: &str = r"C:\Users\kenye\OneDrive\Asztali gép\Áron mappája\TOTAL AI\debug-0e7da8.log";
const SESSION_ID: &str = "0e7da8";

pub fn dlog(location: &str, hypothesis_id: &str, message: &str, data: Value) {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let payload = serde_json::json!({
        "sessionId": SESSION_ID,
        "id": format!("log_{}_{}", ts, location),
        "timestamp": ts,
        "location": location,
        "hypothesisId": hypothesis_id,
        "message": message,
        "data": data,
    });
    let line = format!("{}\n", payload);
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(LOG_PATH)
    {
        let _ = f.write_all(line.as_bytes());
    }
}
// #endregion
