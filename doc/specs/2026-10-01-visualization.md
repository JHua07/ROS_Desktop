# Real-time visualization design (2026-10-01)

**Goal:** Live visualization of raw ROS messages in the desktop GUI: 3D point clouds (`vlpos/msg/H2oPointCloud`), camera images (`sensor_msgs/msg/Image`), and IMU waveforms (`sensor_msgs/msg/Imu`), rendered in the WebView with Three.js/WebGL — no native OpenGL or Open3D embedding.

## Architecture

```
DDS ─► viz_bridge.py (rclpy, spawned per session) ─► Rust viz module (frame parse) ─► Tauri events ─► React/Three.js
```

The bridge is a streaming subprocess like echo/launch/bag, spawned from the captured Pixi environment (which now includes the `ros/install/vlpos` overlay). Frames flow to the frontend as events; the frontend renders with WebGL (point clouds), canvas (images), and a rolling time-series chart (IMU).

## Bridge protocol (frozen)

`ros/viz_bridge.py` takes a single JSON config as `argv[1]`:

```json
{
  "topics": [
    { "topic": "/vlpos/lidar1/h2o", "type": "vlpos/msg/H2oPointCloud", "kind": "pointcloud" },
    { "topic": "/vlpos/camera/image_raw", "type": "sensor_msgs/msg/Image", "kind": "image",
      "options": { "max_width": 960, "downsample": 1 } },
    { "topic": "/vlpos/imu", "type": "sensor_msgs/msg/Imu", "kind": "imu" }
  ]
}
```

- stdout: unbuffered binary frames. Each frame = `VZF1` (4 ASCII bytes) + `u32 LE header_len` + JSON header (UTF-8) + `u32 LE payload_len` + payload. Flush after every frame.
- stderr: one lifecycle/status line per event (startup, subscription confirmed, per-topic errors, shutdown).
- stdin: a reader thread treats EOF or the line `stop` as a clean shutdown request (exit 0).
- QoS: best-effort, depth 5 for `image`/`pointcloud` (throughput over fidelity), depth 10 for `imu`.

Frame headers and payloads by kind (all multi-byte values little-endian):

- `pointcloud` — header `{topic, kind, seq, t_ns, frame_id, point_num, scale}`; payload = `point_num` packed records of 13 bytes: `u32 timestamp, i16 x, i16 y, i16 z, u8 intensity, u8 ring, u8 tag` (numpy structured dtype, `align=False`). Positions are `int16 * scale` when `scale > 0`, otherwise the raw integers.
- `image` — header `{topic, kind, seq, t_ns, frame_id, width, height, encoding, bytes_per_pixel}`; payload = raw pixels after conversion to `rgb8` or `gray8` and optional `downsample`/`max_width` reduction. Unsupported source encodings disable that topic with a stderr line (MVP: rgb8, bgr8, rgba8, gray8/mono8, mono16 scaled to gray8).
- `imu` — header `{topic, kind, seq, t_ns, frame_id}`; payload = 64 bytes: `f64 stamp` + `f64[3] accel` + `f64[3] gyro`.

A `--selftest` mode emits three synthetic frames (one per kind) without DDS, for parser validation.

## IPC (root-owned files register these)

- Command `start_viz {config}` → `{session_id, state:"running"}` or `{code,message}` errors (`environment_unavailable`, `viz_already_running`, `invalid_viz_config`). One bridge at a time; starting while running rejects.
- Command `stop_viz` → `()`; idempotent; writes `stop\n` to stdin, waits boundedly, then force-kills.
- Event `ros-viz-frame`: `{session_id, topic, kind, seq, t_ns, meta, data_b64}` — `meta` is the kind-specific header fields, `data_b64` is base64 of the payload (MVP; raw-byte channels are a later optimization).
- Event `ros-viz-status`: `{session_id, state, line?}` with states `starting|running|stopped|failed`.

## File ownership

- Bridge worker: `ros/viz_bridge.py`, `src-tauri/src/modules/viz.rs` (manager + frame parser + unit tests). No edits to `lib.rs`, `mod.rs`, `process.rs`.
- Frontend worker: `frontend/src/features/VizPanel.tsx` (+ `VizPanel.css`), `frontend/src/features/viz/*` subviews, `frontend/src/modules/viz/index.ts`, `frontend/package.json` (add `three` and `uplot` with exact pins; verify typecheck/build). No edits to `App.tsx` or `styles.css`.
- Root: command/event registration, panel integration, `environment/index.ts` status field, docs, verification, commits.

## Rendering requirements

- Point cloud: `THREE.Points` with a preallocated `BufferGeometry`; update positions and colors per frame; `intensity` mapped through a simple color ramp; OrbitControls; multiple cloud topics render with distinct hues; unmount cleans up WebGL resources.
- Image: canvas `ImageData` blit, aspect preserved.
- IMU: rolling waveform (uPlot or equivalent) of accel/gyro components, ~10 s window, decimated drawing is fine.
- Browser mode renders with native actions disabled, consistent with the other panels.

## Verification

Bridge worker: Rust unit tests for the frame parser (multi-frame buffer with partial trailing bytes), config validation, and a `--selftest` round trip. Frontend worker: `pnpm --dir frontend typecheck && build`. Root: full cargo fmt/test/check, GUI run, and a live bag-play session with the visualization panel showing all three kinds.
