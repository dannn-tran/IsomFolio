use std::collections::HashMap;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::app_paths::models_dir;
use crate::models::AppError;

use super::manifest::AddonManifest;
use super::protocol::{AddonEvent, AddonRequest, ResponseBody, StdoutLine};

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(120);
const CALL_TIMEOUT: Duration = Duration::from_secs(60);
const SUPPORTED_PROTOCOL_VERSION: u32 = 1;

type PendingMap = Arc<Mutex<HashMap<u64, SyncSender<Result<serde_json::Value, String>>>>>;
type ProgressMap = Arc<Mutex<HashMap<u64, SyncSender<u8>>>>;

#[derive(Debug)]
pub struct AddonProcess {
    writer: Arc<Mutex<BufWriter<ChildStdin>>>,
    pending: PendingMap,
    progress_sinks: ProgressMap,
    next_id: Arc<AtomicU64>,
    pub manifest: AddonManifest,
    _child: Child,
    _reader: JoinHandle<()>,
}

struct HelloData {
    protocol_version: u32,
    #[allow(dead_code)]
    addon_api_version: u32,
    capabilities: Vec<String>,
}

impl AddonProcess {
    pub fn launch(mut manifest: AddonManifest) -> Result<Self, AppError> {
        let config_path = crate::addon::config::addon_config_path(&manifest.name);
        let mut child = Command::new(&manifest.executable)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .env("ISOMFOLIO_MODELS_DIR", models_dir())
            .env("ISOMFOLIO_ADDON_CONFIG", config_path)
            .spawn()
            .map_err(|e| AppError::Addon(format!("failed to spawn {}: {}", manifest.name, e)))?;

        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();

        let writer = Arc::new(Mutex::new(BufWriter::new(stdin)));
        let pending: PendingMap = Arc::new(Mutex::new(HashMap::new()));
        let progress_sinks: ProgressMap = Arc::new(Mutex::new(HashMap::new()));

        let (hello_tx, hello_rx) = sync_channel::<Result<HelloData, String>>(1);
        let pending_reader = Arc::clone(&pending);
        let progress_reader = Arc::clone(&progress_sinks);

        let reader = std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();

            let hello_result = if reader.read_line(&mut line).is_err() {
                Err("failed to read handshake line".to_string())
            } else {
                match serde_json::from_str::<StdoutLine>(line.trim()) {
                    Ok(StdoutLine::Event(AddonEvent::Hello {
                        protocol_version,
                        addon_api_version,
                        capabilities,
                    })) => Ok(HelloData { protocol_version, addon_api_version, capabilities }),
                    Ok(_) => Err("expected hello event as first message".to_string()),
                    Err(e) => Err(format!("invalid handshake JSON: {e}")),
                }
            };

            let ok = hello_result.is_ok();
            let _ = hello_tx.send(hello_result);
            if ok {
                reader_loop(reader, pending_reader, progress_reader);
            }
        });

        let hello = hello_rx
            .recv_timeout(HANDSHAKE_TIMEOUT)
            .map_err(|_| AppError::Addon(format!("{}: handshake timed out", manifest.name)))?
            .map_err(|e| AppError::Addon(format!("{}: handshake failed: {}", manifest.name, e)))?;

        if hello.protocol_version != SUPPORTED_PROTOCOL_VERSION {
            return Err(AppError::Addon(format!(
                "{}: unsupported protocol version {} (expected {})",
                manifest.name, hello.protocol_version, SUPPORTED_PROTOCOL_VERSION
            )));
        }
        manifest.capabilities = hello.capabilities;

        Ok(AddonProcess {
            writer,
            pending,
            progress_sinks,
            next_id: Arc::new(AtomicU64::new(1)),
            manifest,
            _child: child,
            _reader: reader,
        })
    }

    pub fn call(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, AppError> {
        self.call_timeout(method, params, CALL_TIMEOUT)
    }

    pub fn call_long(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, AppError> {
        self.call_timeout(method, params, Duration::from_secs(600))
    }

    pub fn send(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<AddonCallHandle, AppError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (result_tx, result_rx) = sync_channel(1);
        let (progress_tx, progress_rx) = sync_channel(256);
        self.pending.lock().unwrap_or_else(|e| e.into_inner()).insert(id, result_tx);
        self.progress_sinks.lock().unwrap_or_else(|e| e.into_inner()).insert(id, progress_tx);

        self.write_request(id, method, params)?;

        Ok(AddonCallHandle { id, result_rx, progress_rx, progress_sinks: Arc::clone(&self.progress_sinks) })
    }

    pub fn send_many(
        &self,
        requests: &[(&str, serde_json::Value)],
    ) -> Result<BatchHandle, AppError> {
        let total = requests.len();
        let (tx, rx) = sync_channel(total);
        for (method, params) in requests {
            let id = self.next_id.fetch_add(1, Ordering::Relaxed);
            self.pending.lock().unwrap_or_else(|e| e.into_inner()).insert(id, tx.clone());
            self.write_request(id, method, params.clone())?;
        }
        Ok(BatchHandle { rx: Arc::new(Mutex::new(rx)), total })
    }

    fn call_timeout(
        &self,
        method: &str,
        params: serde_json::Value,
        timeout: Duration,
    ) -> Result<serde_json::Value, AppError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = sync_channel(1);
        self.pending.lock().unwrap_or_else(|e| e.into_inner()).insert(id, tx);

        self.write_request(id, method, params)?;

        rx.recv_timeout(timeout)
            .map_err(|_| AppError::Addon(format!("addon '{}' timed out", self.manifest.name)))?
            .map_err(AppError::Addon)
    }

    fn write_request(&self, id: u64, method: &str, params: serde_json::Value) -> Result<(), AppError> {
        let req = AddonRequest { id, method: method.to_string(), params };
        let mut line = serde_json::to_string(&req).unwrap();
        line.push('\n');

        let mut w = self.writer.lock().unwrap_or_else(|e| e.into_inner());
        if let Err(e) = w.write_all(line.as_bytes()).and_then(|_| w.flush()) {
            self.pending.lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
            return Err(AppError::Addon(format!("write failed: {e}")));
        }
        Ok(())
    }
}

pub struct BatchHandle {
    pub rx: Arc<Mutex<std::sync::mpsc::Receiver<Result<serde_json::Value, String>>>>,
    pub total: usize,
}

pub struct AddonCallHandle {
    #[allow(dead_code)]
    id: u64,
    pub result_rx: std::sync::mpsc::Receiver<Result<serde_json::Value, String>>,
    pub progress_rx: std::sync::mpsc::Receiver<u8>,
    progress_sinks: ProgressMap,
}

impl Drop for AddonCallHandle {
    fn drop(&mut self) {
        self.progress_sinks.lock().unwrap_or_else(|e| e.into_inner()).remove(&self.id);
    }
}

impl Drop for AddonProcess {
    fn drop(&mut self) {
        // Drop the writer to close stdin — addon sees EOF and can flush/exit
        if let Ok(mut w) = self.writer.lock() {
            let _ = w.flush();
        }
        // Give the process a short grace period to exit before killing
        std::thread::sleep(Duration::from_millis(200));
        if self._child.try_wait().ok().flatten().is_none() {
            let _ = self._child.kill();
        }
    }
}

fn reader_loop(reader: BufReader<impl std::io::Read>, pending: PendingMap, progress_sinks: ProgressMap) {
    for line in reader.lines() {
        let Ok(line) = line else { break };
        match serde_json::from_str::<StdoutLine>(&line) {
            Ok(StdoutLine::Response(resp)) => {
                progress_sinks.lock().unwrap_or_else(|e| e.into_inner()).remove(&resp.id);
                if let Some(tx) = pending.lock().unwrap_or_else(|e| e.into_inner()).remove(&resp.id) {
                    let result = match resp.body {
                        ResponseBody::Ok { result } => Ok(result),
                        ResponseBody::Err { error } => Err(error),
                    };
                    let _ = tx.send(result);
                }
            }
            Ok(StdoutLine::Event(AddonEvent::Log { level, message })) => {
                eprintln!("[addon] [{level}] {message}");
            }
            Ok(StdoutLine::Event(AddonEvent::Progress { id, percent })) => {
                if let Some(tx) = progress_sinks.lock().unwrap_or_else(|e| e.into_inner()).get(&id) {
                    let _ = tx.try_send(percent);
                }
            }
            Ok(StdoutLine::Event(AddonEvent::Hello { .. })) => {
                eprintln!("[addon] unexpected hello after handshake");
            }
            Err(e) => {
                eprintln!("[addon] unrecognised stdout line: {e}: {line}");
            }
        }
    }
    for (_, tx) in pending.lock().unwrap_or_else(|e| e.into_inner()).drain() {
        let _ = tx.send(Err("addon exited".to_string()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    fn write_test_addon(dir: &std::path::Path, script: &str) -> std::path::PathBuf {
        let exe = dir.join("isomfolio-test");
        fs::write(&exe, format!("#!/bin/sh\n{}", script)).unwrap();
        let mut perms = fs::metadata(&exe).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&exe, perms).unwrap();
        exe
    }

    fn make_manifest(exe: std::path::PathBuf) -> AddonManifest {
        AddonManifest {
            name: "test".to_string(),
            protocol_version: 1,
            addon_api_version: 1,
            capabilities: vec!["echo".to_string()],
            description: "test addon".to_string(),
            config_schema: vec![],
            executable: exe,
        }
    }

    #[test]
    fn launch_and_call_echo_addon() {
        let tmp = TempDir::new().unwrap();
        let script = r#"printf '{"type":"hello","protocol_version":1,"addon_api_version":1,"capabilities":["echo"]}\n'
while IFS= read -r line; do
    id=$(echo "$line" | sed 's/.*"id":\([0-9]*\).*/\1/')
    printf '{"id":%s,"result":{"ok":true}}\n' "$id"
done
"#;
        let exe = write_test_addon(tmp.path(), script);
        let proc = AddonProcess::launch(make_manifest(exe)).expect("launch failed");
        let result = proc.call("echo", serde_json::json!({"msg": "hello"})).expect("call failed");
        assert_eq!(result["ok"], true);
    }

    #[test]
    fn launch_fails_on_wrong_protocol_version() {
        let tmp = TempDir::new().unwrap();
        let script = r#"printf '{"type":"hello","protocol_version":99,"addon_api_version":1,"capabilities":[]}\n'"#;
        let exe = write_test_addon(tmp.path(), script);
        let err = AddonProcess::launch(make_manifest(exe)).unwrap_err();
        assert!(err.to_string().contains("unsupported protocol version"));
    }

    #[test]
    fn call_returns_error_on_addon_exit() {
        let tmp = TempDir::new().unwrap();
        let script = r#"printf '{"type":"hello","protocol_version":1,"addon_api_version":1,"capabilities":[]}\n'"#;
        let exe = write_test_addon(tmp.path(), script);
        let proc = AddonProcess::launch(make_manifest(exe)).expect("launch failed");
        std::thread::sleep(Duration::from_millis(100));
        let err = proc.call("echo", serde_json::json!({})).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("addon exited") || msg.contains("timed out") || msg.contains("write failed"),
            "unexpected error: {msg}"
        );
    }
}
