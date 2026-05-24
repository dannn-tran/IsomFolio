use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct AddonRequest {
    pub id: u64,
    pub method: String,
    pub params: serde_json::Value,
}

/// Lines read from addon stdout: either an event (has "type" field) or a response (has "id" + result/error).
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum StdoutLine {
    Event(AddonEvent),
    Response(AddonResponse),
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum AddonEvent {
    Hello {
        protocol_version: u32,
        addon_api_version: u32,
        capabilities: Vec<String>,
    },
    Progress {
        id: u64,
        percent: u8,
    },
    Log {
        level: String,
        message: String,
    },
}

#[derive(Debug, Deserialize)]
pub struct AddonResponse {
    pub id: u64,
    #[serde(flatten)]
    pub body: ResponseBody,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum ResponseBody {
    Ok { result: serde_json::Value },
    Err { error: String },
}
