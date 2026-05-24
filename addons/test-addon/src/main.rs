use std::io::{self, BufRead, Write};

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Deserialize)]
struct Request {
    id: u64,
    method: String,
    params: Value,
}

#[derive(Serialize)]
struct Response {
    id: u64,
    result: Value,
}

#[derive(Serialize)]
struct ErrorResponse {
    id: u64,
    error: String,
}

fn main() {
    let stdout = io::stdout();
    let mut out = stdout.lock();

    let hello = serde_json::json!({
        "type": "hello",
        "protocol_version": 1,
        "addon_api_version": 1,
        "capabilities": ["classify"]
    });
    writeln!(out, "{}", serde_json::to_string(&hello).unwrap()).unwrap();
    out.flush().unwrap();

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match serde_json::from_str::<Request>(line) {
            Ok(req) => {
                let result = handle(&req.method, &req.params);
                let resp = match result {
                    Ok(r) => serde_json::to_string(&Response { id: req.id, result: r }).unwrap(),
                    Err(e) => serde_json::to_string(&ErrorResponse { id: req.id, error: e }).unwrap(),
                };
                writeln!(out, "{resp}").unwrap();
                out.flush().unwrap();
            }
            Err(e) => {
                eprintln!("[test-addon] parse error: {e}: {line}");
            }
        }
    }
}

fn handle(method: &str, _params: &Value) -> Result<Value, String> {
    match method {
        "classify" => Ok(serde_json::json!({
            "tags": [
                { "tag": "test", "confidence": 1.0 },
                { "tag": "placeholder", "confidence": 0.9 }
            ]
        })),
        _ => Err(format!("unknown method: {method}")),
    }
}
