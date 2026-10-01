# ROS Desktop

Cross-platform ROS 2 desktop GUI foundation built with Tauri, React, TypeScript, Vite, Rust, and Pixi/RoboStack.

## Current milestone

The four-workflow MVP connects the daemon-backed graph overview and provides managed topic echo, workspace launch, and bag recording. Rust supervises streaming child processes and emits typed state/output events; browser preview remains usable without native IPC. See [MVP design](doc/specs/2026-09-29-four-workflow-mvp.md) and [execution plan](doc/plans/2026-09-29-four-workflow-mvp.md).

## Toolchain

The selected versions and source links are in [doc/versions.md](doc/versions.md). The repository records the Node baseline in `.node-version`, the Rust toolchain in `rust-toolchain.toml`, and the JavaScript dependency graph in `pnpm-lock.yaml`.

Windows development also needs Microsoft C++ Build Tools and WebView2. Ubuntu development needs the GTK/WebKit development packages listed in the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).

## Start developing

From `D:\ROS_Desktop`:

```powershell
pnpm install --frozen-lockfile
pnpm --dir frontend typecheck
pnpm --dir frontend build
pnpm tauri dev
```

The first three commands do not need ROS, Pixi, or Rust. `pnpm tauri dev` needs the native Rust and platform prerequisites. On machines where local security software fails the Tauri CLI's `beforeDevCommand` spawn with Windows error 231 ("all pipe instances are busy"), start the pieces directly instead: `pnpm --dir frontend dev` in one terminal and `cargo run --manifest-path src-tauri/Cargo.toml` in another; the debug build loads the same dev server.

To inspect the declared versions and tools visible in the current PowerShell session:

```powershell
pnpm check:versions
```

With Pixi 0.81.0 installed (`~/.pixi/bin/pixi.exe`), run `pixi install` to materialize the ROS Jazzy environment (about 1.5 GB). The resulting `pixi.lock` is committed. The GUI captures `pixi shell-hook --json` once, starts or reuses the ros2 daemon, and sends graph queries to its localhost XML-RPC endpoint. The bag workflow additionally requires `ros-jazzy-ros2bag`, and bag playback with static TF requires `ros-jazzy-tf2-msgs`; add further packages only for demonstrated needs. The `ros/vlpos` interface package provides the VLPos custom message types (`vlpos/msg/H2oPointCloud` and friends) needed to play and deserialize VLPos bags; rebuild it after changing the `.msg` files with `bash ros/build_vlpos.sh` (uses the separate `build` pixi environment with colcon and the system MSVC toolchain). CLI access to vlpos types needs the install overlay on top of the pixi environment: `AMENT_PREFIX_PATH` prefixed with `ros\install\vlpos`, `PYTHONPATH` prefixed with `ros\install\vlpos\Lib\site-packages`, and `PATH` prefixed with `ros\install\vlpos\bin`; the GUI does not wire this overlay yet.

## Repository layout

```text
frontend/       React/Vite user interface and typed feature seams
src-tauri/      Tauri configuration, Rust entry point, and IPC boundary
doc/            Version baseline, architecture, plans, and design records
scripts/        Developer checks
ros/            ROS interface packages (vlpos custom messages) and launch assets
pixi.toml       Cross-platform Pixi/RoboStack workspace metadata
```

## Verification boundary

On Windows (2026-10-01), `pnpm --dir frontend typecheck`, `pnpm --dir frontend build` (including `frontend/dist/index.html`), `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`, `cargo test --manifest-path src-tauri/Cargo.toml` (20 passed, one live test ignored by default), and `cargo check --manifest-path src-tauri/Cargo.toml` pass. The ignored live daemon test passes when explicitly enabled. The symlink-escape launch test verifies the created link and skips when local security software silently degrades symlink creation into a regular file. A Pixi-environment CLI smoke confirmed topic publication and echo output, a workspace launch ready line, and graceful bag stop with `metadata.yaml` and five recorded messages. That smoke exercises the ROS executable and Windows stop helper rather than the GUI. A 2026-10-01 desktop run (dev server plus `cargo run`) launched the app, captured the Pixi environment, supervised a daemon cold start that exceeded the five-second startup probe and self-healed on the first graph snapshot, and served the first real GUI-to-IPC graph query. The browser preview was verified with Playwright: all four panels render, every native action is disabled with an explanation, and the console is clean. Interactive echo/launch/bag use inside the desktop window and Ubuntu runtime remain unverified. A frozen-lockfile install passed earlier with pnpm 12.7.0.

## Workflows

Use Overview to refresh Pixi/ROS/daemon status and topic/node/service lists. Echo selects one graph topic. Launch scans workspace-contained `.launch.py`, `.launch.xml`, and `.launch.yaml` targets, passing optional `name:=value` arguments directly. Bag records selected topics to a new, non-existing output path with an existing parent. Each workflow has status, bounded logs, and stop control. A forced stop may leave incomplete bag output, so inspect its metadata.

Visualization subscribes to selected point-cloud (`vlpos/msg/H2oPointCloud`), camera (`sensor_msgs/msg/Image`), and IMU (`sensor_msgs/msg/Imu`) topics through a Python bridge subprocess; the frontend renders 3D point clouds with three.js, camera frames on a canvas, and IMU waveforms with uPlot. See the [visualization design](doc/specs/2026-10-01-visualization.md).
