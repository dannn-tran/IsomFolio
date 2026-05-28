use std::io::{self, BufRead};

use isfx_sdk as sdk;

const VERSION: &str = "1.0.0";

fn main() {
    let stdout = io::stdout();
    let mut out = stdout.lock();

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let line = line.trim();
        if line.is_empty() { continue; }

        match serde_json::from_str::<sdk::Request>(line) {
            Ok(req) => match req.method.as_str() {
                "handshake" => {
                    sdk::send_handshake_response(&mut out, req.id, VERSION, &["classify"]);
                    sdk::send_ready(&mut out);
                }
                "ping" => sdk::send_ping_response(&mut out, req.id),
                "classify" => match handle_classify(&req.params) {
                    Ok(r) => sdk::send_response(&mut out, req.id, r),
                    Err(e) => sdk::send_error(&mut out, req.id, e),
                },
                m => sdk::send_error(&mut out, req.id, format!("unknown method: {m}")),
            },
            Err(e) => eprintln!("[test-addon] parse error: {e}"),
        }
    }
}

fn handle_classify(_params: &serde_json::Value) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "tags": [
            { "tag": "test", "confidence": 1.0 },
            { "tag": "placeholder", "confidence": 0.9 }
        ]
    }))
}
