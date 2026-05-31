use std::path::Path;
use std::process::{Child, Command, Stdio};

use super::InferenceError;

/// A locally-spawned inference engine child process. Killed on drop so the
/// engine lives exactly as long as the app session.
pub struct ManagedInferenceProcess {
    child: Child,
    port: u16,
}

impl ManagedInferenceProcess {
    /// Spawn the native engine binary bound to `port`, using `data_dir` for
    /// model storage. The caller must `InferenceClient::wait_healthy` before
    /// sending requests — spawn returns as soon as the process starts.
    pub fn spawn(binary: &Path, port: u16, data_dir: &Path) -> Result<Self, InferenceError> {
        let child = Command::new(binary)
            .arg("--port")
            .arg(port.to_string())
            .arg("--data-dir")
            .arg(data_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| InferenceError::Spawn(e.to_string()))?;
        Ok(Self { child, port })
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

impl Drop for ManagedInferenceProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
