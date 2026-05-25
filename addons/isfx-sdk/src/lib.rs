use std::io::Write;

use serde::{Deserialize, Serialize};
pub use serde_json::Value;

#[derive(Deserialize)]
pub struct Request {
    pub id: u64,
    pub method: String,
    pub params: Value,
}

#[derive(Serialize)]
pub struct Response {
    pub id: u64,
    pub result: Value,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub id: u64,
    pub error: String,
}

pub fn emit_log(out: &mut impl Write, level: &str, msg: &str) {
    let _ = writeln!(out, "{}", serde_json::json!({"type":"log","level":level,"message":msg}));
    let _ = out.flush();
}

pub fn emit_progress(out: &mut impl Write, id: u64, percent: u32) {
    let _ = writeln!(out, "{}", serde_json::json!({"type":"progress","id":id,"percent":percent}));
    let _ = out.flush();
}

pub fn send_hello(out: &mut impl Write, capabilities: &[&str]) {
    let _ = writeln!(out, "{}", serde_json::json!({
        "type": "hello",
        "protocol_version": 1,
        "addon_api_version": 1,
        "capabilities": capabilities,
    }));
    let _ = out.flush();
}

pub fn send_response(out: &mut impl Write, id: u64, result: Value) {
    let resp = serde_json::to_string(&Response { id, result }).unwrap();
    let _ = writeln!(out, "{resp}");
    let _ = out.flush();
}

pub fn send_error(out: &mut impl Write, id: u64, error: String) {
    let resp = serde_json::to_string(&ErrorResponse { id, error }).unwrap();
    let _ = writeln!(out, "{resp}");
    let _ = out.flush();
}

pub fn load_config<T: for<'de> Deserialize<'de> + Default>(out: &mut impl Write) -> T {
    let path = std::env::var("ISOMFOLIO_ADDON_CONFIG").unwrap_or_default();
    if path.is_empty() {
        return T::default();
    }
    match std::fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_else(|e| {
            emit_log(out, "warn", &format!("config parse error: {e}, using defaults"));
            T::default()
        }),
        Err(_) => T::default(),
    }
}

pub fn load_vocabulary(vocab_file: &str, default_vocab: &[&str], out: &mut impl Write) -> Vec<String> {
    if !vocab_file.is_empty() {
        match std::fs::read_to_string(vocab_file) {
            Ok(content) => {
                let tags: Vec<String> = content
                    .lines()
                    .map(|l| l.trim().to_string())
                    .filter(|l| !l.is_empty() && !l.starts_with('#'))
                    .collect();
                if !tags.is_empty() {
                    emit_log(out, "info", &format!("loaded {} tags from {}", tags.len(), vocab_file));
                    return tags;
                }
                emit_log(out, "warn", "vocabulary file empty, using defaults");
            }
            Err(e) => {
                emit_log(out, "warn", &format!("failed to read vocabulary file: {e}, using defaults"));
            }
        }
    }
    default_vocab.iter().map(|s| s.to_string()).collect()
}
