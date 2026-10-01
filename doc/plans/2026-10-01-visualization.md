# Visualization execution plan and record

**Spec:** `doc/specs/2026-10-01-visualization.md`

1. Root wires the workspace install overlay into environment capture and reports overlay packages through the environment status (done first; every spawned process, including the bridge, inherits it).
2. Bridge worker builds `ros/viz_bridge.py` (rclpy subscriptions, VZF1 binary frames on stdout, lifecycle on stderr, stdin `stop`/EOF shutdown, `--selftest` mode) and `src-tauri/src/modules/viz.rs` (VizManager, resilient frame parser, base64 event emission, bounded stop) with unit tests.
3. Frontend worker builds the visualization panel: topic selection by kind, three.js point-cloud view with orbit controls and intensity coloring, canvas image view, uPlot IMU waveform, and a frame hub that bypasses React state for high-rate updates.
4. Root registers `start_viz`/`stop_viz`, stops the bridge on app exit, mounts the Visualization tab, updates docs, and runs full checks plus a live bag-play session.

## Verification record (2026-10-01)

- Rust: `cargo fmt --check` clean; `cargo test` 32 passed, 0 failed, one live daemon test ignored (13 new tests: viz parser/validation/command and environment overlay); `cargo check` free of code warnings after wiring.
- Frontend: `pnpm --dir frontend typecheck` and `build` pass (bundle-size warning from three.js is expected; code-splitting is a later option).
- Bridge selftest: three synthetic frames parse with self-consistent magic/lengths/payloads.
- Real-data end-to-end against the 51 GB VLPos bag (`--playback-duration 45`, bridge capturing ~30 s): the bridge subscribed to all three kinds, play logged zero ignored topics, and the frame stream parsed with zero trailing bytes — pointcloud 284 frames at ~9.5 Hz (15,405 points × 13 bytes exactly), image 220 frames at 612×512 rgb8 with `max_width` reduction, IMU 3,533 frames at 64 bytes. Both processes exited 0.

## Notes

- The IMU payload is 64 bytes: seven f64 values (stamp, accel xyz, gyro xy z) plus 8 bytes of zero padding, matching the spec's stated size; readers take the first seven doubles.
- The bridge spawns with `CREATE_NO_WINDOW` on Windows (piped IO only, no console needed).
- The CLI env recipe must prepend `ros/install/vlpos/bin` to `PATH`; without it the vlpos typesupport DLLs fail to load (`Could not import 'rosidl_typesupport_c' for package 'vlpos'`). The GUI overlay does this automatically.
- IMU capture measured ~118 Hz of the 200 Hz stream under simultaneous image+pointcloud load with best-effort QoS depth 10; acceptable for visualization, tunable later.

## Follow-ups

1. GUI-level visual confirmation of live rendering during bag playback (user walkthrough).
2. Code-split the three.js bundle to shrink the main chunk.
3. Consider raw-byte IPC channels instead of base64 if image streams feel heavy.
4. Bag playback controls inside the GUI (play/pause/rate), building on this bridge pattern.
