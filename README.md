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

The first three commands do not need ROS, Pixi, or Rust. `pnpm tauri dev` needs the native Rust and platform prerequisites.

To inspect the declared versions and tools visible in the current PowerShell session:

```powershell
pnpm check:versions
```

With Pixi 0.81.0 installed (`~/.pixi/bin/pixi.exe`), run `pixi install` to materialize the ROS Jazzy environment (about 1.5 GB). The resulting `pixi.lock` is committed. The GUI captures `pixi shell-hook --json` once, starts or reuses the ros2 daemon, and sends graph queries to its localhost XML-RPC endpoint. The bag workflow additionally requires `ros-jazzy-ros2bag`; add further packages only for demonstrated needs.

## Repository layout

```text
frontend/       React/Vite user interface and typed feature seams
src-tauri/      Tauri configuration, Rust entry point, and IPC boundary
doc/            Version baseline, architecture, plans, and design records
scripts/        Developer checks
ros/            Reserved workspace area for ROS packages and launch assets
pixi.toml       Cross-platform Pixi/RoboStack workspace metadata
```

## Verification boundary

On Windows (2026-09-29), `pnpm --dir frontend typecheck`, `pnpm --dir frontend build` (including `frontend/dist/index.html`), `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`, `cargo test --manifest-path src-tauri/Cargo.toml` (18 passed, one live test ignored by default), and `cargo check --manifest-path src-tauri/Cargo.toml` pass. The ignored live daemon test passes when explicitly enabled. A Pixi-environment CLI smoke confirmed topic publication and echo output, a workspace launch ready line, and graceful bag stop with `metadata.yaml` and five recorded messages. This smoke exercises the same ROS executable and Windows stop helper, not the full Tauri IPC/GUI route; that route and Ubuntu runtime remain unverified. A frozen-lockfile install passed earlier with pnpm 12.7.0.

## Workflows

Use Overview to refresh Pixi/ROS/daemon status and topic/node/service lists. Echo selects one graph topic. Launch scans workspace-contained `.launch.py`, `.launch.xml`, and `.launch.yaml` targets, passing optional `name:=value` arguments directly. Bag records selected topics to a new, non-existing output path with an existing parent. Each workflow has status, bounded logs, and stop control. A forced stop may leave incomplete bag output, so inspect its metadata.
