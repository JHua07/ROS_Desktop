//! Viz bridge session management and VZF1 frame parsing.
//!
//! The Python bridge (`ros/viz_bridge.py`) is spawned from the captured
//! environment and streams length-prefixed binary frames over stdout. This
//! module parses them tolerantly and forwards `ros-viz-frame` /
//! `ros-viz-status` events; command registration lives in the crate root.

use std::{
    io::{self, BufRead, Read, Write},
    path::Path,
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::modules::{environment::CapturedEnvironment, process::ProcessError};

const MAGIC: &[u8; 4] = b"VZF1";
/// Bound allocations on a corrupt stream; a 100k-point cloud is ~1.3 MB and
/// a 4K rgb8 image is ~24 MB, so these leave generous headroom.
const MAX_HEADER_BYTES: usize = 1 << 20;
const MAX_PAYLOAD_BYTES: usize = 64 << 20;
/// Grace period after `stop\n` before the child is force-killed.
const STOP_GRACE: Duration = Duration::from_secs(3);

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VizTopicKind {
    Pointcloud,
    Image,
    Imu,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct VizTopicOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub downsample: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_width: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VizTopicConfig {
    pub topic: String,
    pub kind: VizTopicKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    #[serde(default)]
    pub options: VizTopicOptions,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VizConfig {
    pub topics: Vec<VizTopicConfig>,
}

impl VizConfig {
    /// Parse and validate a config from the IPC boundary so both malformed
    /// shapes and semantic violations surface as `invalid_viz_config`.
    pub fn from_json(value: serde_json::Value) -> Result<Self, ProcessError> {
        let config: Self = serde_json::from_value(value).map_err(|error| {
            ProcessError::new(
                "invalid_viz_config",
                format!("malformed viz config: {error}"),
            )
        })?;
        validate(&config)?;
        Ok(config)
    }
}

fn validate(config: &VizConfig) -> Result<(), ProcessError> {
    let invalid = |message: String| ProcessError::new("invalid_viz_config", message);
    if config.topics.is_empty() {
        return Err(invalid("topics must not be empty".into()));
    }
    for topic in &config.topics {
        if !valid_topic_name(&topic.topic) {
            return Err(invalid(format!(
                "topic `{}` must be an absolute ROS name without control characters",
                topic.topic
            )));
        }
        if let Some(downsample) = topic.options.downsample
            && !(1..=8).contains(&downsample)
        {
            return Err(invalid(format!(
                "topic `{}` downsample must be within 1..=8",
                topic.topic
            )));
        }
        if let Some(max_width) = topic.options.max_width
            && !(64..=3840).contains(&max_width)
        {
            return Err(invalid(format!(
                "topic `{}` max_width must be within 64..=3840",
                topic.topic
            )));
        }
    }
    Ok(())
}

fn valid_topic_name(topic: &str) -> bool {
    topic.starts_with('/') && topic.len() > 1 && !topic.chars().any(char::is_control)
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VizState {
    // IPC contract placeholder: the frontend sets `starting` locally before
    // the first event arrives; the Rust side emits `running` once spawned.
    #[allow(dead_code)]
    Starting,
    Running,
    Stopped,
    Failed,
}

#[derive(Clone, Debug, Serialize)]
pub struct VizSession {
    pub session_id: u64,
    pub state: VizState,
}

#[derive(Clone, Debug, Serialize)]
pub struct VizFrameEvent {
    pub session_id: u64,
    pub topic: String,
    pub kind: String,
    pub seq: u64,
    pub t_ns: u64,
    pub meta: serde_json::Value,
    pub data_b64: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct VizStatusEvent {
    pub session_id: u64,
    pub state: VizState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<String>,
}

struct Active {
    id: u64,
    child: Arc<Mutex<Child>>,
    stdin: Arc<Mutex<Option<ChildStdin>>>,
    stopping: Arc<AtomicBool>,
}

#[derive(Clone, Default)]
pub struct VizManager {
    active: Arc<Mutex<Option<Active>>>,
    next: Arc<AtomicU64>,
}

impl VizManager {
    pub fn start(
        &self,
        app: &AppHandle,
        environment: &CapturedEnvironment,
        workspace: &Path,
        config: &VizConfig,
    ) -> Result<VizSession, ProcessError> {
        validate(config)?;
        let mut active = self.active.lock().unwrap();
        if active.is_some() {
            return Err(ProcessError::new(
                "viz_already_running",
                "A viz bridge session is already active",
            ));
        }
        let config_json = serde_json::to_string(config).map_err(|error| {
            ProcessError::new(
                "invalid_viz_config",
                format!("Unable to serialize viz config: {error}"),
            )
        })?;
        let mut command = build_command(environment, workspace, &config_json)?;
        configure_bridge_process(&mut command);
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| {
                ProcessError::new(
                    "spawn_failed",
                    format!("Unable to start viz bridge: {error}"),
                )
            })?;
        let stdout = child.stdout.take().expect("stdout configured");
        let stderr = child.stderr.take().expect("stderr configured");
        let stdin = child.stdin.take().expect("stdin configured");
        let id = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        let child = Arc::new(Mutex::new(child));
        let stopping = Arc::new(AtomicBool::new(false));
        *active = Some(Active {
            id,
            child: child.clone(),
            stdin: Arc::new(Mutex::new(Some(stdin))),
            stopping: stopping.clone(),
        });
        drop(active);
        emit_status(app, id, VizState::Running, None);
        let frame_thread = spawn_frame_reader(app.clone(), id, stdout);
        let status_thread = spawn_status_reader(app.clone(), id, stderr);
        let app = app.clone();
        let entries = self.active.clone();
        thread::spawn(move || {
            let result = loop {
                match child.lock().unwrap().try_wait() {
                    Ok(Some(status)) => break Ok(status),
                    Err(error) => break Err(error),
                    Ok(None) => thread::sleep(Duration::from_millis(50)),
                }
            };
            // A terminal event comes only after both pipes reach EOF, so tail
            // frames and status lines are still delivered.
            let _ = frame_thread.join();
            let _ = status_thread.join();
            let mut event = VizStatusEvent {
                session_id: id,
                state: VizState::Stopped,
                line: None,
            };
            match result {
                Ok(status) => {
                    if !stopping.load(Ordering::SeqCst) && !status.success() {
                        event.state = VizState::Failed;
                        event.line = Some(format!("viz bridge exited with {status}"));
                    }
                }
                Err(error) => {
                    event.state = VizState::Failed;
                    event.line = Some(format!("Unable to wait for viz bridge: {error}"));
                }
            }
            {
                let mut active = entries.lock().unwrap();
                if active.as_ref().is_some_and(|entry| entry.id == id) {
                    *active = None;
                }
            }
            let _ = app.emit("ros-viz-status", event);
        });
        Ok(VizSession {
            session_id: id,
            state: VizState::Running,
        })
    }

    pub fn stop(&self, app: &AppHandle) -> Result<(), ProcessError> {
        let active = self.active.lock().unwrap();
        let Some(entry) = active.as_ref() else {
            return Ok(()); // idempotent
        };
        if entry.stopping.swap(true, Ordering::SeqCst) {
            return Ok(());
        }
        let id = entry.id;
        let child = entry.child.clone();
        let stdin = entry.stdin.clone();
        drop(active);
        emit_status(
            app,
            id,
            VizState::Running,
            Some("viz bridge stop requested".to_owned()),
        );
        // A failed graceful request is not fatal; forced cleanup still runs.
        if let Some(mut handle) = stdin.lock().unwrap().take() {
            let _ = handle.write_all(b"stop\n");
            let _ = handle.flush();
            drop(handle); // EOF also requests a clean shutdown
        }
        thread::spawn(move || {
            let deadline = Instant::now() + STOP_GRACE;
            while Instant::now() < deadline {
                if child.lock().unwrap().try_wait().ok().flatten().is_some() {
                    return;
                }
                thread::sleep(Duration::from_millis(50));
            }
            let _ = child.lock().unwrap().kill();
        });
        Ok(())
    }

    // Reserved for a future `viz_status` command; the frontend currently
    // tracks the session from start/stop and status events.
    #[allow(dead_code)]
    pub fn is_running(&self) -> bool {
        self.active.lock().unwrap().is_some()
    }
}

/// Build `CONDA_PREFIX/python -u <workspace>/ros/viz_bridge.py <json>` with
/// the captured environment's variable table applied.
fn build_command(
    environment: &CapturedEnvironment,
    workspace: &Path,
    config_json: &str,
) -> Result<Command, ProcessError> {
    let prefix = environment.variable("CONDA_PREFIX").ok_or_else(|| {
        ProcessError::new("environment_unavailable", "Pixi CONDA_PREFIX is missing")
    })?;
    let python = if cfg!(windows) {
        Path::new(prefix).join("python.exe")
    } else {
        Path::new(prefix).join("bin").join("python")
    };
    let python = python.to_str().ok_or_else(|| {
        ProcessError::new("environment_unavailable", "Pixi Python path is not UTF-8")
    })?;
    let mut command = environment.command(python);
    command.current_dir(workspace);
    command
        .arg("-u")
        .arg(workspace.join("ros").join("viz_bridge.py"))
        .arg(config_json);
    Ok(command)
}

/// Unlike echo/launch/bag, the bridge stops through its piped stdin and
/// never needs console signals, so no console window is created for it.
fn configure_bridge_process(command: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
}

fn emit_status(app: &AppHandle, session_id: u64, state: VizState, line: Option<String>) {
    let _ = app.emit(
        "ros-viz-status",
        VizStatusEvent {
            session_id,
            state,
            line,
        },
    );
}

#[derive(Debug)]
pub(crate) struct VizFrame {
    pub header: serde_json::Value,
    pub payload: Vec<u8>,
}

#[derive(Debug)]
pub(crate) enum FrameParse {
    /// More bytes are needed before the head of the buffer is a full frame.
    Incomplete,
    /// The head of the buffer holds a complete frame.
    Complete { frame: VizFrame, consumed: usize },
    /// The head of the buffer is not a valid frame; skip bytes to resync.
    Desync { skip: usize },
}

/// Pure incremental VZF1 parser: `VZF1` + u32LE header_len + JSON header +
/// u32LE payload_len + payload, all little-endian.
pub(crate) fn parse_frame(buffer: &[u8]) -> FrameParse {
    if buffer.len() < MAGIC.len() {
        return FrameParse::Incomplete;
    }
    if !buffer.starts_with(MAGIC) {
        return FrameParse::Desync {
            skip: resync_offset(buffer),
        };
    }
    if buffer.len() < 8 {
        return FrameParse::Incomplete;
    }
    let header_len = u32::from_le_bytes(buffer[4..8].try_into().unwrap()) as usize;
    if header_len > MAX_HEADER_BYTES {
        return FrameParse::Desync {
            skip: resync_offset(buffer),
        };
    }
    let header_end = 8 + header_len;
    if buffer.len() < header_end + 4 {
        return FrameParse::Incomplete;
    }
    let payload_len =
        u32::from_le_bytes(buffer[header_end..header_end + 4].try_into().unwrap()) as usize;
    if payload_len > MAX_PAYLOAD_BYTES {
        return FrameParse::Desync {
            skip: resync_offset(buffer),
        };
    }
    let total = header_end + 4 + payload_len;
    if buffer.len() < total {
        return FrameParse::Incomplete;
    }
    let header = match serde_json::from_slice(&buffer[8..header_end]) {
        Ok(header) => header,
        // Frame boundaries are plausible but the header is unreadable; drop
        // the whole frame instead of scanning for a new magic.
        Err(_) => {
            return FrameParse::Desync { skip: total };
        }
    };
    let payload = buffer[header_end + 4..total].to_vec();
    FrameParse::Complete {
        frame: VizFrame { header, payload },
        consumed: total,
    }
}

/// Bytes to drop when the buffer head is not a frame: everything before the
/// next magic, keeping a possible partial magic at the tail.
fn resync_offset(buffer: &[u8]) -> usize {
    buffer[1..]
        .windows(MAGIC.len())
        .position(|window| window == MAGIC.as_slice())
        .map_or_else(
            || buffer.len().saturating_sub(MAGIC.len() - 1).max(1),
            |position| position + 1,
        )
}

fn drain_frames(app: &AppHandle, session_id: u64, buffer: &mut Vec<u8>) {
    loop {
        match parse_frame(buffer) {
            FrameParse::Incomplete => return,
            FrameParse::Complete { frame, consumed } => {
                match frame_event(session_id, &frame.header, &frame.payload) {
                    Some(event) => {
                        let _ = app.emit("ros-viz-frame", event);
                    }
                    None => emit_status(
                        app,
                        session_id,
                        VizState::Running,
                        Some("skipped a viz frame with an incomplete header".to_owned()),
                    ),
                }
                buffer.drain(..consumed);
            }
            FrameParse::Desync { skip } => {
                emit_status(
                    app,
                    session_id,
                    VizState::Running,
                    Some(format!(
                        "viz stream desynchronized; dropped {skip} byte(s) and resynchronized"
                    )),
                );
                buffer.drain(..skip);
            }
        }
    }
}

/// Split a frame header into the common event fields and the kind-specific
/// `meta` object (everything except topic/kind/seq/t_ns).
fn frame_event(
    session_id: u64,
    header: &serde_json::Value,
    payload: &[u8],
) -> Option<VizFrameEvent> {
    let topic = header.get("topic")?.as_str()?.to_owned();
    let kind = header.get("kind")?.as_str()?.to_owned();
    let seq = header
        .get("seq")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let t_ns = header
        .get("t_ns")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let mut meta = serde_json::Map::new();
    if let Some(fields) = header.as_object() {
        for (key, value) in fields {
            if !matches!(key.as_str(), "topic" | "kind" | "seq" | "t_ns") {
                meta.insert(key.clone(), value.clone());
            }
        }
    }
    Some(VizFrameEvent {
        session_id,
        topic,
        kind,
        seq,
        t_ns,
        meta: serde_json::Value::Object(meta),
        data_b64: base64::engine::general_purpose::STANDARD.encode(payload),
    })
}

fn spawn_frame_reader(
    app: AppHandle,
    session_id: u64,
    stdout: impl Read + Send + 'static,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut reader = io::BufReader::new(stdout);
        let mut buffer: Vec<u8> = Vec::new();
        let mut chunk = [0u8; 16_384];
        loop {
            match reader.read(&mut chunk) {
                Ok(0) => break, // EOF: the monitor thread emits the terminal state
                Ok(read) => {
                    buffer.extend_from_slice(&chunk[..read]);
                    drain_frames(&app, session_id, &mut buffer);
                }
                Err(error) => {
                    emit_status(
                        &app,
                        session_id,
                        VizState::Failed,
                        Some(format!("viz bridge stdout read failed: {error}")),
                    );
                    break;
                }
            }
        }
    })
}

fn spawn_status_reader(
    app: AppHandle,
    session_id: u64,
    stderr: impl Read + Send + 'static,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut reader = io::BufReader::new(stderr);
        let mut line = Vec::new();
        loop {
            line.clear();
            // Bound any single logical output line, including malformed streams.
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
                let text = String::from_utf8_lossy(&line)
                    .trim_end_matches(['\r', '\n'])
                    .to_string();
                emit_status(&app, session_id, VizState::Running, Some(text));
            }
            if ended {
                break;
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::environment::CapturedEnvironment;
    use std::path::PathBuf;

    fn frame_bytes(header: &serde_json::Value, payload: &[u8]) -> Vec<u8> {
        let header = serde_json::to_vec(header).unwrap();
        let mut bytes = Vec::new();
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&(header.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&header);
        bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        bytes.extend_from_slice(payload);
        bytes
    }

    /// Mirrors the reader-thread drain loop without event emission.
    fn drain(buffer: &mut Vec<u8>) -> Vec<VizFrame> {
        let mut frames = Vec::new();
        loop {
            match parse_frame(buffer) {
                FrameParse::Incomplete => return frames,
                FrameParse::Complete { frame, consumed } => {
                    buffer.drain(..consumed);
                    frames.push(frame);
                }
                FrameParse::Desync { skip } => {
                    buffer.drain(..skip);
                }
            }
        }
    }

    fn pointcloud_header() -> serde_json::Value {
        serde_json::json!({
            "topic": "/vlpos/lidar1/h2o",
            "kind": "pointcloud",
            "seq": 1,
            "t_ns": 42,
            "frame_id": "map",
            "point_num": 2,
            "scale": 0.5,
        })
    }

    #[test]
    fn parses_consecutive_frames_and_keeps_partial_tail() {
        let imu_header = serde_json::json!({
            "topic": "/vlpos/imu", "kind": "imu", "seq": 0, "t_ns": 7, "frame_id": "imu",
        });
        let first = frame_bytes(&pointcloud_header(), b"points-payload");
        let second = frame_bytes(&imu_header, &[0u8; 64]);
        let mut partial = frame_bytes(&pointcloud_header(), b"0123456789");
        partial.truncate(partial.len() - 3);

        let mut buffer = Vec::new();
        buffer.extend_from_slice(&first);
        buffer.extend_from_slice(&second);
        buffer.extend_from_slice(&partial);

        let frames = drain(&mut buffer);
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].header["topic"], "/vlpos/lidar1/h2o");
        assert_eq!(frames[0].payload, b"points-payload");
        assert_eq!(frames[1].header["kind"], "imu");
        assert_eq!(frames[1].payload.len(), 64);
        assert_eq!(buffer, partial);

        buffer.extend_from_slice(b"789");
        let frames = drain(&mut buffer);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].payload, b"0123456789");
        assert!(buffer.is_empty());
    }

    #[test]
    fn resynchronizes_after_bad_magic() {
        let frame = frame_bytes(&pointcloud_header(), b"abc");
        let mut buffer = b"junk-bytes".to_vec();
        buffer.extend_from_slice(&frame);

        match parse_frame(&buffer) {
            FrameParse::Desync { skip } => assert_eq!(skip, b"junk-bytes".len()),
            other => panic!("expected desync, got {other:?}"),
        }
        let frames = drain(&mut buffer);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].payload, b"abc");
        assert!(buffer.is_empty());
    }

    #[test]
    fn drops_unrecoverable_garbage_but_keeps_partial_magic_tail() {
        let mut buffer = b"\x00\x01\x02garbage".to_vec();
        let frames = drain(&mut buffer);
        assert!(frames.is_empty());
        assert!(buffer.len() <= MAGIC.len() - 1); // possible start of a magic
    }

    #[test]
    fn rejects_oversized_lengths_as_desync() {
        let mut buffer = MAGIC.to_vec();
        buffer.extend_from_slice(&u32::MAX.to_le_bytes());
        assert!(matches!(parse_frame(&buffer), FrameParse::Desync { .. }));

        let header = serde_json::to_vec(&pointcloud_header()).unwrap();
        let mut buffer = MAGIC.to_vec();
        buffer.extend_from_slice(&(header.len() as u32).to_le_bytes());
        buffer.extend_from_slice(&header);
        buffer.extend_from_slice(&u32::MAX.to_le_bytes());
        assert!(matches!(parse_frame(&buffer), FrameParse::Desync { .. }));
    }

    #[test]
    fn skips_frame_whose_header_is_not_json() {
        let junk = b"{not json";
        let payload = b"payload";
        let mut buffer = MAGIC.to_vec();
        buffer.extend_from_slice(&(junk.len() as u32).to_le_bytes());
        buffer.extend_from_slice(junk);
        buffer.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        buffer.extend_from_slice(payload);
        let total = buffer.len();
        match parse_frame(&buffer) {
            FrameParse::Desync { skip } => assert_eq!(skip, total),
            other => panic!("expected desync, got {other:?}"),
        }
    }

    #[test]
    fn frame_event_splits_common_and_meta_fields() {
        let payload = b"payload-bytes";
        let event = frame_event(9, &pointcloud_header(), payload).unwrap();
        assert_eq!(event.session_id, 9);
        assert_eq!(event.topic, "/vlpos/lidar1/h2o");
        assert_eq!(event.kind, "pointcloud");
        assert_eq!(event.seq, 1);
        assert_eq!(event.t_ns, 42);
        assert_eq!(
            event.meta,
            serde_json::json!({"frame_id": "map", "point_num": 2, "scale": 0.5})
        );
        // Base64 round trip of the payload.
        let engine = base64::engine::general_purpose::STANDARD;
        assert_eq!(engine.decode(&event.data_b64).unwrap(), payload);

        assert!(frame_event(1, &serde_json::json!({"kind": "imu"}), b"").is_none());
    }

    #[test]
    fn rejects_invalid_configs() {
        let cases = [
            serde_json::json!([]),
            serde_json::json!([{ "topic": "camera", "kind": "image" }]),
            serde_json::json!([{ "topic": "/cam\nera", "kind": "image" }]),
            serde_json::json!([{ "topic": "/", "kind": "image" }]),
            serde_json::json!([{ "topic": "/camera", "kind": "video" }]),
            serde_json::json!([{ "topic": "/camera", "kind": "image", "options": { "downsample": 0 } }]),
            serde_json::json!([{ "topic": "/camera", "kind": "image", "options": { "downsample": 9 } }]),
            serde_json::json!([{ "topic": "/camera", "kind": "image", "options": { "max_width": 10 } }]),
            serde_json::json!([{ "topic": "/camera", "kind": "image", "options": { "max_width": 5000 } }]),
        ];
        for case in cases {
            let error = VizConfig::from_json(serde_json::json!({ "topics": case }))
                .err()
                .unwrap_or_else(|| panic!("accepted config {case}"));
            assert_eq!(error.code, "invalid_viz_config");
        }
    }

    #[test]
    fn accepts_valid_config() {
        let value = serde_json::json!({
            "topics": [
                { "topic": "/vlpos/lidar1/h2o", "type": "vlpos/msg/H2oPointCloud", "kind": "pointcloud" },
                { "topic": "/vlpos/camera/image_raw", "type": "sensor_msgs/msg/Image", "kind": "image",
                  "options": { "max_width": 960, "downsample": 1 } },
                { "topic": "/vlpos/imu", "kind": "imu" }
            ]
        });
        let config = VizConfig::from_json(value).unwrap();
        assert_eq!(config.topics.len(), 3);
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"kind\":\"pointcloud\""));
        assert!(json.contains("\"downsample\":1"));
    }

    fn environment() -> CapturedEnvironment {
        CapturedEnvironment::from_shell_hook_json(
            PathBuf::from("pixi"),
            PathBuf::from("workspace/pixi.toml"),
            r#"{"environment_variables":{"ROS_DISTRO":"jazzy","CONDA_PREFIX":"workspace/env"}}"#,
        )
        .unwrap()
    }

    #[test]
    fn builds_bridge_command_in_captured_environment() {
        let command =
            build_command(&environment(), Path::new("workspace"), r#"{"topics":[]}"#).unwrap();
        assert_eq!(
            command.get_program(),
            std::ffi::OsStr::new(if cfg!(windows) {
                "workspace/env\\python.exe"
            } else {
                "workspace/env/bin/python"
            })
        );
        let args: Vec<_> = command.get_args().collect();
        assert_eq!(args[0], std::ffi::OsStr::new("-u"));
        assert!(args[1].to_string_lossy().ends_with("viz_bridge.py"));
        assert_eq!(args[2].to_string_lossy(), r#"{"topics":[]}"#);
        assert_eq!(command.get_current_dir(), Some(Path::new("workspace")));
        assert_eq!(
            command
                .get_envs()
                .find(|(key, _)| *key == "ROS_DISTRO")
                .unwrap()
                .1,
            Some(std::ffi::OsStr::new("jazzy"))
        );
    }
}
