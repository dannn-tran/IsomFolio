use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct ExtensionRequest {
    pub id: u64,
    pub method: String,
    #[serde(skip_serializing_if = "serde_json::Value::is_null")]
    pub params: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct HandshakeResult {
    pub protocol_version: u32,
    pub extension_version: String,
    pub capabilities: Vec<String>,
}

/// All messages sent by an extension on stdout, discriminated by "type".
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum StdoutLine {
    Ok { id: u64, result: serde_json::Value },
    Error { id: u64, error: String },
    Ready,
    Fatal { repairable: bool, message: String },
    Progress { id: u64, percent: u8 },
}
