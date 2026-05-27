use std::io::{self, BufRead, Write};

use serde_json::Value;

const VERSION: &str = "1.0.0";

fn main() {
    let stdout = io::stdout();
    let mut out = stdout.lock();

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let req: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[test-addon] parse error: {e}: {line}");
                continue;
            }
        };

        let id = match req.get("id").and_then(|v| v.as_u64()) {
            Some(id) => id,
            None => continue,
        };
        let method = req.get("method").and_then(|v| v.as_str()).unwrap_or("");

        match method {
            "handshake" => {
                let _ = writeln!(out, "{}", serde_json::json!({
                    "type": "ok",
                    "id": id,
                    "result": {
                        "protocol_version": 1,
                        "addon_version": VERSION,
                        "capabilities": ["classify"]
                    }
                }));
                let _ = out.flush();
                let _ = writeln!(out, r#"{{"type":"ready"}}"#);
                let _ = out.flush();
            }
            "ping" => {
                let _ = writeln!(out, "{}", serde_json::json!({"type":"ok","id":id,"result":{}}));
                let _ = out.flush();
            }
            "classify" => {
                let result = handle_classify(req.get("params").unwrap_or(&Value::Null));
                let (type_str, body) = match result {
                    Ok(r) => ("ok", serde_json::json!({"type":"ok","id":id,"result":r})),
                    Err(e) => ("error", serde_json::json!({"type":"error","id":id,"error":e})),
                };
                let _ = type_str;
                let _ = writeln!(out, "{body}");
                let _ = out.flush();
            }
            m => {
                let _ = writeln!(out, "{}", serde_json::json!({"type":"error","id":id,"error":format!("unknown method: {m}")}));
                let _ = out.flush();
            }
        }
    }
}

fn handle_classify(_params: &Value) -> Result<Value, String> {
    Ok(serde_json::json!({
        "tags": [
            { "tag": "test", "confidence": 1.0 },
            { "tag": "placeholder", "confidence": 0.9 }
        ]
    }))
}
