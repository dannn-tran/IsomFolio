use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

const STDERR_RING_SIZE: usize = 100;

use crate::app_paths::models_dir;
use crate::models::AppError;

use super::manifest::AddonManifest;
use super::protocol::{AddonRequest, HandshakeResult, StdoutLine};

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
const READY_TIMEOUT: Duration = Duration::from_secs(120);
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
    stderr_buf: Arc<Mutex<VecDeque<String>>>,
    pub manifest: AddonManifest,
    _child: Child,
    _reader: JoinHandle<()>,
    _stderr_reader: JoinHandle<()>,
}

impl AddonProcess {
    pub fn launch(mut manifest: AddonManifest) -> Result<Self, AppError> {
        let mut child = Command::new(&manifest.executable)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("ISOMFOLIO_MODELS_DIR", models_dir())
            .spawn()
            .map_err(|e| AppError::Addon(format!("failed to spawn {}: {}", manifest.name, e)))?;

        let mut stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();

        let stderr_buf: Arc<Mutex<VecDeque<String>>> = Arc::new(Mutex::new(VecDeque::new()));
        let stderr_buf_writer = Arc::clone(&stderr_buf);
        let addon_name_for_stderr = manifest.name.clone();
        let _stderr_reader = std::thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines() {
                let Ok(line) = line else { break };
                eprintln!("[{addon_name_for_stderr}] {line}");
                let mut buf = stderr_buf_writer.lock().unwrap_or_else(|e| e.into_inner());
                buf.push_back(line);
                if buf.len() > STDERR_RING_SIZE {
                    buf.pop_front();
                }
            }
        });

        // Send handshake request before handing stdin off to the writer Arc
        let handshake_id = 1u64;
        let handshake_req = serde_json::to_string(&AddonRequest {
            id: handshake_id,
            method: "handshake".to_string(),
            params: serde_json::Value::Null,
        })
        .unwrap();
        writeln!(stdin, "{handshake_req}")
            .and_then(|_| stdin.flush())
            .map_err(|e| AppError::Addon(format!("{}: failed to send handshake: {e}", manifest.name)))?;

        let writer = Arc::new(Mutex::new(BufWriter::new(stdin)));
        let pending: PendingMap = Arc::new(Mutex::new(HashMap::new()));
        let progress_sinks: ProgressMap = Arc::new(Mutex::new(HashMap::new()));

        let (handshake_tx, handshake_rx) = sync_channel::<Result<HandshakeResult, String>>(1);
        let (ready_tx, ready_rx) = sync_channel::<Result<(), String>>(1);
        let pending_reader = Arc::clone(&pending);
        let progress_reader = Arc::clone(&progress_sinks);
        let addon_name_for_reader = manifest.name.clone();

        let reader = std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();

            // Phase 1: wait for handshake Ok response
            let mut handshake_ok = false;
            for line in lines.by_ref() {
                let Ok(line) = line else {
                    let _ = handshake_tx.send(Err("addon exited during handshake".to_string()));
                    return;
                };
                match serde_json::from_str::<StdoutLine>(line.trim()) {
                    Ok(StdoutLine::Ok { id, result }) if id == handshake_id => {
                        match serde_json::from_value::<HandshakeResult>(result) {
                            Ok(h) => {
                                let _ = handshake_tx.send(Ok(h));
                                handshake_ok = true;
                                break;
                            }
                            Err(e) => {
                                let _ = handshake_tx.send(Err(format!("invalid handshake result: {e}")));
                                return;
                            }
                        }
                    }
                    Ok(StdoutLine::Error { id, error }) if id == handshake_id => {
                        let _ = handshake_tx.send(Err(error));
                        return;
                    }
                    Ok(StdoutLine::Log { level, message }) => {
                        eprintln!("[{addon_name_for_reader}] [{level}] {message}");
                    }
                    Ok(_) => {}
                    Err(e) => eprintln!("[{addon_name_for_reader}] parse error during handshake: {e}"),
                }
            }

            if !handshake_ok {
                return;
            }

            // Phase 2: wait for ready event
            for line in lines.by_ref() {
                let Ok(line) = line else {
                    let _ = ready_tx.send(Err("addon exited before ready".to_string()));
                    return;
                };
                match serde_json::from_str::<StdoutLine>(line.trim()) {
                    Ok(StdoutLine::Ready) => {
                        let _ = ready_tx.send(Ok(()));
                        break;
                    }
                    Ok(StdoutLine::Log { level, message }) => {
                        eprintln!("[{addon_name_for_reader}] [{level}] {message}");
                    }
                    Ok(_) => {}
                    Err(e) => eprintln!("[{addon_name_for_reader}] parse error during startup: {e}"),
                }
            }

            reader_loop(lines, pending_reader, progress_reader, &addon_name_for_reader);
        });

        let handshake = handshake_rx
            .recv_timeout(HANDSHAKE_TIMEOUT)
            .map_err(|_| AppError::Addon(format!("{}: handshake timed out", manifest.name)))?
            .map_err(|e| AppError::Addon(format!("{}: handshake failed: {}", manifest.name, e)))?;

        if handshake.protocol_version != SUPPORTED_PROTOCOL_VERSION {
            return Err(AppError::Addon(format!(
                "{}: unsupported protocol version {} (expected {})",
                manifest.name, handshake.protocol_version, SUPPORTED_PROTOCOL_VERSION
            )));
        }
        manifest.capabilities = handshake.capabilities;

        ready_rx
            .recv_timeout(READY_TIMEOUT)
            .map_err(|_| AppError::Addon(format!("{}: ready timed out (model loading took too long)", manifest.name)))?
            .map_err(|e| AppError::Addon(format!("{}: startup failed: {}", manifest.name, e)))?;

        Ok(AddonProcess {
            writer,
            pending,
            progress_sinks,
            next_id: Arc::new(AtomicU64::new(2)), // 1 was used for handshake
            stderr_buf,
            manifest,
            _child: child,
            _reader: reader,
            _stderr_reader,
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

    pub fn last_stderr(&self) -> Vec<String> {
        self.stderr_buf.lock().unwrap_or_else(|e| e.into_inner()).iter().cloned().collect()
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
        if let Ok(mut w) = self.writer.lock() {
            let _ = w.flush();
        }
        std::thread::sleep(Duration::from_millis(200));
        if self._child.try_wait().ok().flatten().is_none() {
            let _ = self._child.kill();
        }
    }
}

fn reader_loop(
    lines: impl Iterator<Item = std::io::Result<String>>,
    pending: PendingMap,
    progress_sinks: ProgressMap,
    addon_name: &str,
) {
    for line in lines {
        let Ok(line) = line else { break };
        match serde_json::from_str::<StdoutLine>(&line) {
            Ok(StdoutLine::Ok { id, result }) => {
                progress_sinks.lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
                if let Some(tx) = pending.lock().unwrap_or_else(|e| e.into_inner()).remove(&id) {
                    let _ = tx.send(Ok(result));
                }
            }
            Ok(StdoutLine::Error { id, error }) => {
                progress_sinks.lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
                if let Some(tx) = pending.lock().unwrap_or_else(|e| e.into_inner()).remove(&id) {
                    let _ = tx.send(Err(error));
                }
            }
            Ok(StdoutLine::Log { level, message }) => {
                eprintln!("[{addon_name}] [{level}] {message}");
            }
            Ok(StdoutLine::Progress { id, percent }) => {
                if let Some(tx) = progress_sinks.lock().unwrap_or_else(|e| e.into_inner()).get(&id) {
                    let _ = tx.try_send(percent);
                }
            }
            Ok(StdoutLine::Ready) => {
                eprintln!("[{addon_name}] unexpected ready after startup");
            }
            Err(e) => {
                eprintln!("[{addon_name}] unrecognised stdout line: {e}: {line}");
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
            version: "1.0.0".to_string(),
            capabilities: vec!["echo".to_string()],
            description: "test addon".to_string(),
            config_schema: vec![],
            executable: exe,
        }
    }

    // Minimal addon script: responds to handshake, sends ready, echoes requests
    fn echo_script() -> &'static str {
        r#"IFS= read -r hs_line
hs_id=$(printf '%s' "$hs_line" | sed 's/.*"id":\([0-9]*\).*/\1/')
printf '{"type":"ok","id":%s,"result":{"protocol_version":1,"addon_version":"1.0.0","capabilities":["echo"]}}\n' "$hs_id"
printf '{"type":"ready"}\n'
while IFS= read -r line; do
    id=$(printf '%s' "$line" | sed 's/.*"id":\([0-9]*\).*/\1/')
    printf '{"type":"ok","id":%s,"result":{"ok":true}}\n' "$id"
done
"#
    }

    #[test]
    fn launch_and_call_echo_addon() {
        let tmp = TempDir::new().unwrap();
        let exe = write_test_addon(tmp.path(), echo_script());
        let proc = AddonProcess::launch(make_manifest(exe)).expect("launch failed");
        let result = proc.call("echo", serde_json::json!({"msg": "hello"})).expect("call failed");
        assert_eq!(result["ok"], true);
    }

    #[test]
    fn launch_fails_on_wrong_protocol_version() {
        let tmp = TempDir::new().unwrap();
        let script = r#"IFS= read -r hs_line
hs_id=$(printf '%s' "$hs_line" | sed 's/.*"id":\([0-9]*\).*/\1/')
printf '{"type":"ok","id":%s,"result":{"protocol_version":99,"addon_version":"1.0.0","capabilities":[]}}\n' "$hs_id"
printf '{"type":"ready"}\n'
"#;
        let exe = write_test_addon(tmp.path(), script);
        let err = AddonProcess::launch(make_manifest(exe)).unwrap_err();
        assert!(err.to_string().contains("unsupported protocol version"));
    }

    #[test]
    fn send_many_returns_all_responses() {
        let tmp = TempDir::new().unwrap();
        let exe = write_test_addon(tmp.path(), echo_script());
        let proc = AddonProcess::launch(make_manifest(exe)).expect("launch failed");
        let requests: Vec<(&str, serde_json::Value)> = (0..5)
            .map(|i| ("echo", serde_json::json!({"n": i})))
            .collect();
        let handle = proc.send_many(&requests).expect("send_many failed");
        assert_eq!(handle.total, 5);
        let mut count = 0;
        while count < 5 {
            let rx = handle.rx.lock().unwrap();
            let result = rx.recv_timeout(Duration::from_secs(5)).expect("recv timed out");
            assert!(result.is_ok());
            count += 1;
        }
    }

    #[test]
    fn send_many_crash_mid_batch() {
        let tmp = TempDir::new().unwrap();
        let script = r#"IFS= read -r hs_line
hs_id=$(printf '%s' "$hs_line" | sed 's/.*"id":\([0-9]*\).*/\1/')
printf '{"type":"ok","id":%s,"result":{"protocol_version":1,"addon_version":"1.0.0","capabilities":["echo"]}}\n' "$hs_id"
printf '{"type":"ready"}\n'
count=0
while IFS= read -r line; do
    id=$(printf '%s' "$line" | sed 's/.*"id":\([0-9]*\).*/\1/')
    count=$((count + 1))
    if [ "$count" -ge 3 ]; then
        exit 1
    fi
    printf '{"type":"ok","id":%s,"result":{"ok":true}}\n' "$id"
done
"#;
        let exe = write_test_addon(tmp.path(), script);
        let proc = AddonProcess::launch(make_manifest(exe)).expect("launch failed");
        let requests: Vec<(&str, serde_json::Value)> = (0..5)
            .map(|i| ("echo", serde_json::json!({"n": i})))
            .collect();
        let handle = proc.send_many(&requests).expect("send_many failed");
        let mut ok_count = 0;
        let mut err_count = 0;
        for _ in 0..5 {
            let rx = handle.rx.lock().unwrap();
            match rx.recv_timeout(Duration::from_secs(5)) {
                Ok(Ok(_)) => ok_count += 1,
                Ok(Err(_)) => err_count += 1,
                Err(_) => break,
            }
        }
        assert!(ok_count >= 2, "expected at least 2 successes, got {ok_count}");
        assert!(err_count > 0 || ok_count < 5, "expected some failures from crash");
    }

    #[test]
    fn send_with_progress_events() {
        let tmp = TempDir::new().unwrap();
        let script = r#"IFS= read -r hs_line
hs_id=$(printf '%s' "$hs_line" | sed 's/.*"id":\([0-9]*\).*/\1/')
printf '{"type":"ok","id":%s,"result":{"protocol_version":1,"addon_version":"1.0.0","capabilities":["echo"]}}\n' "$hs_id"
printf '{"type":"ready"}\n'
while IFS= read -r line; do
    id=$(printf '%s' "$line" | sed 's/.*"id":\([0-9]*\).*/\1/')
    printf '{"type":"progress","id":%s,"percent":50}\n' "$id"
    printf '{"type":"progress","id":%s,"percent":100}\n' "$id"
    printf '{"type":"ok","id":%s,"result":{"done":true}}\n' "$id"
done
"#;
        let exe = write_test_addon(tmp.path(), script);
        let proc = AddonProcess::launch(make_manifest(exe)).expect("launch failed");
        let handle = proc.send("echo", serde_json::json!({})).expect("send failed");
        let mut progress_values = Vec::new();
        loop {
            match handle.progress_rx.recv_timeout(Duration::from_secs(5)) {
                Ok(p) => progress_values.push(p),
                Err(_) => break,
            }
        }
        assert_eq!(progress_values, vec![50, 100]);
        let result = handle.result_rx.recv_timeout(Duration::from_secs(5)).expect("no result");
        assert!(result.is_ok());
        assert_eq!(result.unwrap()["done"], true);
    }

    #[test]
    fn stderr_captured_in_ring_buffer() {
        let tmp = TempDir::new().unwrap();
        let script = r#"echo "startup error log" >&2
echo "warning line" >&2
IFS= read -r hs_line
hs_id=$(printf '%s' "$hs_line" | sed 's/.*"id":\([0-9]*\).*/\1/')
printf '{"type":"ok","id":%s,"result":{"protocol_version":1,"addon_version":"1.0.0","capabilities":[]}}\n' "$hs_id"
printf '{"type":"ready"}\n'
while IFS= read -r line; do
    echo "processing" >&2
    id=$(printf '%s' "$line" | sed 's/.*"id":\([0-9]*\).*/\1/')
    printf '{"type":"ok","id":%s,"result":{}}\n' "$id"
done
"#;
        let exe = write_test_addon(tmp.path(), script);
        let proc = AddonProcess::launch(make_manifest(exe)).expect("launch failed");
        let _ = proc.call("test", serde_json::json!({}));
        std::thread::sleep(Duration::from_millis(100));
        let stderr = proc.last_stderr();
        assert!(stderr.len() >= 2, "expected stderr lines, got {:?}", stderr);
        assert!(stderr.iter().any(|l| l.contains("startup error log")));
        assert!(stderr.iter().any(|l| l.contains("warning line")));
    }

    #[test]
    fn addon_error_response_propagated() {
        let tmp = TempDir::new().unwrap();
        let script = r#"IFS= read -r hs_line
hs_id=$(printf '%s' "$hs_line" | sed 's/.*"id":\([0-9]*\).*/\1/')
printf '{"type":"ok","id":%s,"result":{"protocol_version":1,"addon_version":"1.0.0","capabilities":["echo"]}}\n' "$hs_id"
printf '{"type":"ready"}\n'
while IFS= read -r line; do
    id=$(printf '%s' "$line" | sed 's/.*"id":\([0-9]*\).*/\1/')
    printf '{"type":"error","id":%s,"error":"something went wrong"}\n' "$id"
done
"#;
        let exe = write_test_addon(tmp.path(), script);
        let proc = AddonProcess::launch(make_manifest(exe)).expect("launch failed");
        let err = proc.call("echo", serde_json::json!({})).unwrap_err();
        assert!(err.to_string().contains("something went wrong"));
    }

    #[test]
    fn send_many_empty_batch() {
        let tmp = TempDir::new().unwrap();
        let exe = write_test_addon(tmp.path(), echo_script());
        let proc = AddonProcess::launch(make_manifest(exe)).expect("launch failed");
        let handle = proc.send_many(&[]).expect("send_many failed");
        assert_eq!(handle.total, 0);
    }

    #[test]
    fn multiple_sequential_calls() {
        let tmp = TempDir::new().unwrap();
        let exe = write_test_addon(tmp.path(), echo_script());
        let proc = AddonProcess::launch(make_manifest(exe)).expect("launch failed");
        for i in 0..10 {
            let result = proc.call("echo", serde_json::json!({"i": i})).expect("call failed");
            assert_eq!(result["ok"], true);
        }
    }

    #[test]
    fn call_returns_error_on_addon_exit() {
        let tmp = TempDir::new().unwrap();
        // Addon completes handshake+ready then exits immediately
        let script = r#"IFS= read -r hs_line
hs_id=$(printf '%s' "$hs_line" | sed 's/.*"id":\([0-9]*\).*/\1/')
printf '{"type":"ok","id":%s,"result":{"protocol_version":1,"addon_version":"1.0.0","capabilities":[]}}\n' "$hs_id"
printf '{"type":"ready"}\n'
"#;
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
