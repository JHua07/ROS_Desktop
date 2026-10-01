//! Lifecycle and event transport for the three streaming ROS workflows.
use crate::modules::platform;
use serde::Serialize;
use std::{
    collections::HashMap,
    fmt,
    io::{self, BufRead, Read},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, serde::Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessKind {
    TopicEcho,
    Launch,
    Bag,
}

#[derive(Debug, Serialize)]
pub struct ProcessError {
    pub code: &'static str,
    pub message: String,
}
impl ProcessError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}
impl fmt::Display for ProcessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl From<io::Error> for ProcessError {
    fn from(error: io::Error) -> Self {
        Self::new("process_io", error.to_string())
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessPhase {
    // IPC contract placeholder: the frontend sets `starting` locally before the
    // first event arrives; the Rust side emits `running` once the child spawns.
    #[allow(dead_code)]
    Starting,
    Running,
    Stopping,
    Stopped,
    Failed,
}

#[derive(Clone, Debug, Serialize)]
pub struct ProcessSession {
    pub session_id: u64,
    pub kind: ProcessKind,
    pub state: ProcessPhase,
}
#[derive(Clone, Debug, Serialize)]
pub struct ProcessEvent {
    pub session_id: u64,
    pub kind: ProcessKind,
    pub state: ProcessPhase,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
impl ProcessEvent {
    fn state(id: u64, kind: ProcessKind, state: ProcessPhase) -> Self {
        Self {
            session_id: id,
            kind,
            state,
            stream: None,
            line: None,
            exit_code: None,
            error: None,
        }
    }
}
struct Active {
    id: u64,
    child: Arc<Mutex<Child>>,
    stopping: Arc<AtomicBool>,
    forced: Arc<AtomicBool>,
}
#[derive(Clone, Default)]
pub struct ProcessManager {
    active: Arc<Mutex<HashMap<ProcessKind, Active>>>,
    next: Arc<AtomicU64>,
}
impl ProcessManager {
    pub fn start(
        &self,
        app: AppHandle,
        kind: ProcessKind,
        mut command: Command,
    ) -> Result<ProcessSession, ProcessError> {
        let mut active = self.active.lock().unwrap();
        if active.contains_key(&kind) {
            return Err(ProcessError::new(
                "already_running",
                format!("{kind:?} is already active"),
            ));
        }
        platform::configure_managed_process(&mut command);
        let mut child = command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                ProcessError::new("spawn_failed", format!("Unable to start {kind:?}: {e}"))
            })?;
        let stdout = child.stdout.take().expect("stdout configured");
        let stderr = child.stderr.take().expect("stderr configured");
        let id = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        let child = Arc::new(Mutex::new(child));
        let stopping = Arc::new(AtomicBool::new(false));
        let forced = Arc::new(AtomicBool::new(false));
        active.insert(
            kind,
            Active {
                id,
                child: child.clone(),
                stopping: stopping.clone(),
                forced: forced.clone(),
            },
        );
        drop(active);
        let _ = app.emit(
            "ros-process-event",
            ProcessEvent::state(id, kind, ProcessPhase::Running),
        );
        let stdout_thread = spawn_reader(app.clone(), id, kind, "stdout", stdout);
        let stderr_thread = spawn_reader(app.clone(), id, kind, "stderr", stderr);
        let entries = self.active.clone();
        thread::spawn(move || {
            let result = loop {
                let status = child.lock().unwrap().try_wait();
                match status {
                    Ok(Some(status)) => break Ok(status),
                    Err(error) => break Err(error),
                    Ok(None) => thread::sleep(Duration::from_millis(50)),
                }
            };
            // A terminal event comes only after both pipes reach EOF, including stop tail output.
            let _ = stdout_thread.join();
            let _ = stderr_thread.join();
            let mut event = ProcessEvent::state(id, kind, ProcessPhase::Stopped);
            match result {
                Ok(status) => {
                    event.exit_code = status.code();
                    if forced.load(Ordering::SeqCst) {
                        event.state = ProcessPhase::Failed;
                        event.error = Some(format!(
                            "{kind:?} was forcibly terminated; output may be incomplete"
                        ));
                    } else if !status.success() && !stopping.load(Ordering::SeqCst) {
                        event.state = ProcessPhase::Failed;
                        event.error = Some(format!("{kind:?} exited with {status}"));
                    }
                }
                Err(error) => {
                    event.state = ProcessPhase::Failed;
                    event.error = Some(format!("Unable to wait for {kind:?}: {error}"));
                }
            }
            {
                let mut map = entries.lock().unwrap();
                if map.get(&kind).is_some_and(|entry| entry.id == id) {
                    map.remove(&kind);
                }
            }
            let _ = app.emit("ros-process-event", event);
        });
        Ok(ProcessSession {
            session_id: id,
            kind,
            state: ProcessPhase::Running,
        })
    }
    pub fn stop(&self, app: &AppHandle, kind: ProcessKind) -> Result<(), ProcessError> {
        let map = self.active.lock().unwrap();
        let entry = map
            .get(&kind)
            .ok_or_else(|| ProcessError::new("not_running", "No active process of that kind"))?;
        if entry.stopping.swap(true, Ordering::SeqCst) {
            return Ok(());
        }
        let id = entry.id;
        let child = entry.child.clone();
        let forced = entry.forced.clone();
        drop(map);
        let _ = app.emit(
            "ros-process-event",
            ProcessEvent::state(id, kind, ProcessPhase::Stopping),
        );
        // A failed graceful signal is reported, but forced cleanup still runs after grace.
        let graceful = platform::request_graceful_stop(&child.lock().unwrap());
        if let Err(error) = graceful {
            let mut event = ProcessEvent::state(id, kind, ProcessPhase::Stopping);
            event.error = Some(format!(
                "Graceful stop failed; forcing termination if needed: {error}"
            ));
            let _ = app.emit("ros-process-event", event);
        }
        thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            while Instant::now() < deadline {
                if child.lock().unwrap().try_wait().ok().flatten().is_some() {
                    return;
                }
                thread::sleep(Duration::from_millis(50));
            }
            forced.store(true, Ordering::SeqCst);
            let _ = child.lock().unwrap().kill();
        });
        Ok(())
    }
    pub fn stop_all(&self, app: &AppHandle) {
        let kinds: Vec<_> = self.active.lock().unwrap().keys().copied().collect();
        for kind in kinds {
            let _ = self.stop(app, kind);
        }
    }
    pub fn wait_for_all(&self, timeout: Duration) {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if self.active.lock().unwrap().is_empty() {
                return;
            }
            thread::sleep(Duration::from_millis(50));
        }
        for entry in self.active.lock().unwrap().values() {
            entry.forced.store(true, Ordering::SeqCst);
            let _ = entry.child.lock().unwrap().kill();
        }
    }
}
fn spawn_reader<R: Read + Send + 'static>(
    app: AppHandle,
    id: u64,
    kind: ProcessKind,
    stream: &'static str,
    input: R,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut reader = io::BufReader::new(input);
        let mut line = Vec::new();
        loop {
            line.clear();
            // Bound any single logical output line, including malformed/binary streams.
            let mut ended = false;
            while line.len() < 16_384 {
                let buf = match reader.fill_buf() {
                    Ok(buf) => buf,
                    Err(_) => {
                        ended = true;
                        break;
                    }
                };
                if buf.is_empty() {
                    ended = true;
                    break;
                }
                let count = buf
                    .iter()
                    .position(|byte| *byte == b'\n')
                    .map_or(buf.len(), |index| index + 1)
                    .min(16_384 - line.len());
                line.extend_from_slice(&buf[..count]);
                reader.consume(count);
                if line.last() == Some(&b'\n') {
                    break;
                }
            }
            if !line.is_empty() {
                let mut event = ProcessEvent::state(id, kind, ProcessPhase::Running);
                event.stream = Some(stream);
                event.line = Some(
                    String::from_utf8_lossy(&line)
                        .trim_end_matches(['\r', '\n'])
                        .to_string(),
                );
                let _ = app.emit("ros-process-event", event);
            }
            if ended {
                break;
            }
        }
    })
}

/// Retained for localhost daemon-spawn traffic; the XML-RPC client uses raw TCP.
pub fn bypass_localhost_proxies(command: &mut Command) {
    command
        .env("HTTP_PROXY", "")
        .env("HTTPS_PROXY", "")
        .env("ALL_PROXY", "")
        .env("http_proxy", "")
        .env("https_proxy", "")
        .env("all_proxy", "")
        .env("NO_PROXY", "127.0.0.1,localhost")
        .env("no_proxy", "127.0.0.1,localhost");
}
