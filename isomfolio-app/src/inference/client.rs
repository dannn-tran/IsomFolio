use std::time::{Duration, Instant};

use reqwest::Url;

use super::{EmbedFile, EmbedRequest, EmbedResponse, InferenceError, ManagedInferenceProcess};

/// HTTP client for the face inference engine. Wraps an optional managed child
/// process — when present, the process is killed when the client is dropped.
#[derive(Debug)]
pub struct InferenceClient {
    http: reqwest::Client,
    base_url: Url,
    _process: Option<ManagedInferenceProcess>,
}

impl InferenceClient {
    /// Connect to a remote engine at a user-supplied base URL (e.g. a
    /// self-hosted InsightFace container). No process is managed.
    // Wired up by the Custom-URL Settings option (step 7).
    #[allow(dead_code)]
    pub fn remote(base_url: &str) -> Result<Self, InferenceError> {
        let base_url = Url::parse(base_url).map_err(|e| InferenceError::Http(e.to_string()))?;
        Ok(Self { http: build_http()?, base_url, _process: None })
    }

    /// Wrap a managed local process, connecting to `localhost:<port>`.
    pub fn managed(process: ManagedInferenceProcess) -> Result<Self, InferenceError> {
        let base_url = Url::parse(&format!("http://127.0.0.1:{}", process.port()))
            .map_err(|e| InferenceError::Http(e.to_string()))?;
        Ok(Self { http: build_http()?, base_url, _process: Some(process) })
    }

    /// Poll `GET /health` until it returns 200 or `timeout` elapses. Generous
    /// timeouts are expected on first run (model download).
    pub async fn wait_healthy(&self, timeout: Duration) -> Result<(), InferenceError> {
        let url = self.base_url.join("health").map_err(|e| InferenceError::Http(e.to_string()))?;
        let deadline = Instant::now() + timeout;
        loop {
            if let Ok(resp) = self.http.get(url.clone()).send().await {
                if resp.status().is_success() {
                    return Ok(());
                }
            }
            if Instant::now() >= deadline {
                return Err(InferenceError::HealthTimeout(timeout));
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }

    /// `POST /embed` for a batch of files. Files with no faces (or that failed
    /// engine-side) come back with an empty `faces` list.
    pub async fn embed(&self, files: Vec<EmbedFile>) -> Result<EmbedResponse, InferenceError> {
        let url = self.base_url.join("embed").map_err(|e| InferenceError::Http(e.to_string()))?;
        let resp = self
            .http
            .post(url)
            .json(&EmbedRequest { files })
            .send()
            .await
            .map_err(|e| InferenceError::Http(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(InferenceError::Http(format!("engine returned {}", resp.status())));
        }
        resp.json::<EmbedResponse>().await.map_err(|e| InferenceError::Http(e.to_string()))
    }
}

fn build_http() -> Result<reqwest::Client, InferenceError> {
    reqwest::Client::builder()
        // No request timeout: a 50-file batch can take many seconds of CPU
        // inference. Connection-level failures still surface promptly.
        .build()
        .map_err(|e| InferenceError::Http(e.to_string()))
}
