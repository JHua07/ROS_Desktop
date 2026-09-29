# ROS Desktop Skeleton Design

## Goal

Create a small, buildable cross-platform desktop application in `D:\ROS_Desktop` for a future ROS 2 GUI. The first milestone proves the desktop shell, frontend build, Rust/Tauri command boundary, environment metadata, and the first daemon-backed ROS graph workflow.

## Confirmed constraints

- Primary development target: native Windows; Linux/Ubuntu remains a supported build target.
- Desktop stack: Tauri 2 + Rust + React + TypeScript + Vite.
- Package manager: pnpm, pinned through the root `package.json` `packageManager` field and a committed lockfile.
- ROS environment: Pixi with RoboStack Jazzy channels and `win-64`/`linux-64` platform declarations.
- The first concrete workflow is ROS graph inspection (topics, nodes, services, and counts), so the minimal `ros-jazzy-ros-core` dependency is part of the baseline.
- The app must build its frontend without Rust, Pixi, or ROS being installed; the native shell check is reported separately when Rust is unavailable.

## Version decisions (verified 2026-09-28)

| Component | Selected version | Reason |
| --- | --- | --- |
| Tauri | 2.12.0 | Current stable 2.x release; its MSRV is Rust 1.90, so the current Rust stable satisfies it. |
| Rust | stable, pinned by `rust-toolchain.toml` to 1.98.1 | Current stable release and compatible with Tauri 2.12. |
| Node.js | 24.21.0 LTS baseline | Active LTS line with the broadest ecosystem support for the selected frontend tools. |
| pnpm | 12.7.0 | Current published 12.x release; supports Node 24 and Windows/Linux. |
| React / React DOM | 19.3.0 | Current stable React release. |
| TypeScript | 7.0.2 | Current stable compiler. |
| Vite | 8.3.1 | Current supported Vite 8 line. |
| Pixi | 0.81.0 | Current Pixi release and the version recorded by the official installer. |
| ROS 2 / RoboStack | Jazzy / `robostack-jazzy` | Jazzy is an LTS release supported through May 2029 and RoboStack publishes Jazzy packages for `win-64` and `linux-64`. |

The exact source URLs and the verification date are recorded in `doc/versions.md`.

## Architecture

The repository is a single pnpm workspace. The frontend is an independently buildable Vite package under `frontend/`; the Tauri crate is under `src-tauri/`. Tauri serves `frontend/dist` and exposes typed IPC commands. The first native workflow is ROS graph inspection through the `ros2` daemon XML-RPC endpoint; the Rust side owns Pixi environment activation and daemon lifecycle while the frontend consumes structured graph data.

Pixi is represented by a root `pixi.toml` with RoboStack Jazzy channels, target platforms, and the minimal `ros-jazzy-ros-core` dependency required by the graph workflow. Additional `ros-jazzy-*` packages require a separate concrete workflow.

## Error and platform policy

- Frontend startup must render a local shell even when the ROS environment is absent.
- Native environment/process errors will later cross the Tauri boundary as structured error values; the initial `ping` command is intentionally infallible.
- Graph queries use plain localhost XML-RPC at `127.0.0.1:(11511 + ROS_DOMAIN_ID)/ros2cli/`; proxy environment variables are ignored for these requests.
- The Rust startup path captures `pixi shell-hook --json` once, starts or reuses the daemon, and keeps the frontend usable when Pixi or ROS is unavailable by returning structured availability errors.
- `pixi run` is not used in GUI code paths. Subprocesses remain reserved for streaming or long-running operations such as `topic echo`, `launch`, and `bag`.
- Windows uses the Tauri WebView2 runtime and MSVC Rust target. Ubuntu uses the GTK/WebKit development dependencies documented by Tauri.
- Packaging assets and installers are deferred until the shell and ROS process lifecycle are implemented.

## Verification

The initial acceptance checks are:

1. `pnpm install --frozen-lockfile` succeeds on the native Windows checkout.
2. `pnpm --dir frontend typecheck` succeeds.
3. `pnpm --dir frontend build` succeeds and emits `frontend/dist`.
4. `cargo check --manifest-path src-tauri/Cargo.toml` succeeds when the Rust toolchain is installed; if it cannot run in the current machine, the missing prerequisite is reported explicitly.
5. `pixi install` succeeds and the committed `pixi.lock` describes the Jazzy core environment.
6. A Rust unit/integration check proves XML-RPC endpoint selection, request encoding, response decoding, and proxy-independent localhost transport.
7. Git records the complete scaffold and lock/version files.
