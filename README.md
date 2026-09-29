# ROS Desktop

Cross-platform ROS 2 desktop GUI foundation built with Tauri, React, TypeScript, Vite, Rust, and Pixi/RoboStack.

## Current milestone

The repository contains a runnable frontend shell, a Tauri 2.12 Rust shell with typed environment and graph IPC commands, and module boundaries for ROS, environment, process, and platform work. The Pixi manifest targets `win-64` and `linux-64` through RoboStack Jazzy and includes `ros-jazzy-ros-core` as the baseline for daemon-backed graph inspection.

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

With Pixi 0.81.0 installed (`~/.pixi/bin/pixi.exe`), run `pixi install` to materialize the ROS Jazzy environment (about 1.5 GB). The resulting `pixi.lock` is committed. The GUI captures `pixi shell-hook --json` once, starts or reuses the ros2 daemon, and sends graph queries to its localhost XML-RPC endpoint. Add further `ros-jazzy-*` packages only after the corresponding GUI workflow has been selected.

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

The frontend typecheck and production build are verified in the Windows checkout; a frozen-lockfile install also passed with pnpm 12.7.0. The Tauri CLI resolves to 2.12.0 and the native configuration parses successfully. `cargo check` passes with Rust 1.98.1 and MSVC Build Tools 2022 (see `src-tauri/Cargo.lock`). The Pixi/RoboStack Jazzy environment installs with `pixi install`, and the daemon was verified with `ros2 daemon start` and `ros2 daemon status` on Windows on 2026-09-29.

## Next step

The next step is to connect the frontend overview to `environment_status` and `ros_graph_snapshot`, then add streaming process workflows such as `topic echo`, `launch`, and `bag` behind the existing process boundary.
