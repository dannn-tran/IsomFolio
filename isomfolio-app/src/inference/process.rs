use std::path::Path;
#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use super::InferenceError;

/// A locally-managed inference engine. Either a native child process or a
/// Docker container (Intel macOS, where ORT 1.26.0 has no osx-x64 library).
/// Torn down on drop so the engine lives exactly as long as the app session.
#[derive(Debug)]
pub struct ManagedInferenceProcess {
    /// Native child, if launched directly.
    child: Option<Child>,
    /// Docker container name, if launched in a container.
    container: Option<String>,
    port: u16,
}

impl ManagedInferenceProcess {
    /// Spawn the native engine binary bound to `port`, using `data_dir` for
    /// model storage. The caller must `InferenceClient::wait_healthy` before
    /// sending requests — spawn returns as soon as the process starts.
    #[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
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
        Ok(Self { child: Some(child), container: None, port })
    }

    /// Run the engine in a Docker container (Intel macOS). `model_cache` is
    /// bind-mounted at `/models` so downloads persist; each path in `mounts`
    /// (the catalog's photo roots) is mounted read-only at the same path so the
    /// engine resolves image paths identically inside the container.
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    pub fn spawn_docker(
        port: u16,
        model_cache: &Path,
        mounts: &[PathBuf],
    ) -> Result<Self, InferenceError> {
        const IMAGE: &str = "isomfolio-faces-inference:latest";
        const NAME: &str = "isomfolio-faces-inference";

        let image_present = Command::new("docker")
            .args(["image", "inspect", IMAGE])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !image_present {
            return Err(InferenceError::Spawn(format!(
                "Docker image {IMAGE} not found. Build it with ./scripts/build-faces-docker.sh \
                 (Intel Mac runs the engine in Docker)."
            )));
        }

        // Clear any stale container from a previous, ungraceful exit.
        let _ = Command::new("docker")
            .args(["rm", "-f", NAME])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();

        let mut cmd = Command::new("docker");
        cmd.args(["run", "-d", "--rm", "--name", NAME]);
        // Publish only to the host loopback — never exposed externally.
        cmd.arg("-p").arg(format!("127.0.0.1:{port}:{port}"));
        cmd.arg("-v").arg(format!("{}:/models", model_cache.display()));
        for m in mounts {
            let p = m.display();
            cmd.arg("-v").arg(format!("{p}:{p}:ro"));
        }
        cmd.arg(IMAGE);
        cmd.args(["--port", &port.to_string(), "--data-dir", "/models", "--host", "0.0.0.0"]);

        let status = cmd
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .status()
            .map_err(|e| InferenceError::Spawn(e.to_string()))?;
        if !status.success() {
            return Err(InferenceError::Spawn(
                "docker run failed — is Docker Desktop running?".to_string(),
            ));
        }
        Ok(Self { child: None, container: Some(NAME.to_string()), port })
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

impl Drop for ManagedInferenceProcess {
    fn drop(&mut self) {
        if let Some(child) = &mut self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(name) = &self.container {
            let _ = Command::new("docker")
                .args(["stop", name])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }
}
