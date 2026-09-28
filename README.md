# ROS Desktop

Cross-platform ROS 2 desktop GUI foundation built with Tauri, React, TypeScript, Vite, Rust, and Pixi/RoboStack.

## Current milestone

The repository contains a runnable frontend shell, a Tauri 2.12 Rust shell with a `ping` IPC command, and module boundaries for ROS, environment, process, and platform work. The Pixi manifest targets `win-64` and `linux-64` through RoboStack Jazzy and now includes `ros-jazzy-ros-core` — the baseline for the first concrete workflow: running `ros2` CLI commands (topic/node/service inspection) from the GUI.

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

With Pixi 0.81.0 installed (`~/.pixi/bin/pixi.exe`), run `pixi install` to materialize the ROS Jazzy environment (about 1.5 GB) and then, for example, `pixi run ros2 topic list`. The resulting `pixi.lock` is committed. Add further `ros-jazzy-*` packages only after the corresponding GUI workflow has been selected.

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

The frontend typecheck and production build are verified in the Windows checkout; a frozen-lockfile install also passed with pnpm 12.7.0. The Tauri CLI resolves to 2.12.0 and the native configuration parses successfully. `cargo check` passes with Rust 1.98.1 and MSVC Build Tools 2022 (see `src-tauri/Cargo.lock`). The Pixi/RoboStack Jazzy environment installs with `pixi install` and `pixi run ros2 topic list`, `ros2 pkg list` (157 packages), and `ros2 interface show` were verified on Windows on 2026-09-29.

## Next step

Implement the environment discovery module first: locate Pixi, inspect the selected workspace, and return structured platform and ROS availability data through Tauri IPC. That step should choose the smallest ROS package set needed by the first topic/node/service workflow.
