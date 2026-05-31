//! Face inference engine integration. The engine is a stateless HTTP server
//! that turns image paths into face embeddings; the host owns all persistence
//! and clustering (see `dev-docs/face-inference-engine.md`).

mod client;
mod process;

pub use client::InferenceClient;
pub use process::ManagedInferenceProcess;

use serde::{Deserialize, Serialize};

/// One file to embed. `path` must be resolvable by the engine — same path for
/// a local engine; for a remote/Docker engine the path must be mounted there.
#[derive(Debug, Clone, Serialize)]
pub struct EmbedFile {
    pub file_id: String,
    pub path: String,
    pub mtime: i64,
}

#[derive(Debug, Serialize)]
struct EmbedRequest {
    files: Vec<EmbedFile>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EmbedResponse {
    pub results: Vec<FileResult>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FileResult {
    pub file_id: String,
    #[serde(default)]
    pub faces: Vec<DetectedFace>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DetectedFace {
    pub bbox: Bbox,
    pub vec: Vec<f32>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Bbox {
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
    #[serde(default)]
    pub w: f64,
    #[serde(default)]
    pub h: f64,
}

#[derive(Debug, thiserror::Error)]
pub enum InferenceError {
    #[error("inference HTTP error: {0}")]
    Http(String),
    #[error("inference engine did not become ready within {0:?}")]
    HealthTimeout(std::time::Duration),
    #[error("failed to spawn inference engine: {0}")]
    Spawn(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embed_response_parses_faces_and_vec() {
        let json = r#"{
            "results": [
                { "file_id": "a", "faces": [
                    { "bbox": { "x": 0.1, "y": 0.2, "w": 0.3, "h": 0.4 }, "vec": [0.1, -0.2, 0.3] }
                ]},
                { "file_id": "b", "faces": [] }
            ]
        }"#;
        let resp: EmbedResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.results.len(), 2);
        assert_eq!(resp.results[0].file_id, "a");
        assert_eq!(resp.results[0].faces.len(), 1);
        assert_eq!(resp.results[0].faces[0].vec, vec![0.1, -0.2, 0.3]);
        assert_eq!(resp.results[0].faces[0].bbox.w, 0.3);
        assert!(resp.results[1].faces.is_empty());
    }

    #[test]
    fn embed_request_serializes_snake_case() {
        let req = EmbedRequest {
            files: vec![EmbedFile {
                file_id: "x".into(),
                path: "/p/i.jpg".into(),
                mtime: 1700000000,
            }],
        };
        let v = serde_json::to_value(&req).unwrap();
        assert_eq!(v["files"][0]["file_id"], "x");
        assert_eq!(v["files"][0]["path"], "/p/i.jpg");
        assert_eq!(v["files"][0]["mtime"], 1700000000_i64);
    }
}
