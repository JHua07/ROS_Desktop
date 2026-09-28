# ROS Desktop Skeleton Design

## Goal

Create a small, buildable cross-platform desktop application skeleton in `D:\ROS_Desktop` for a future ROS 2 GUI. The first milestone proves the desktop shell, frontend build, Rust/Tauri command boundary, and environment metadata without assuming a specific ROS package set.

## Confirmed constraints

- Primary development target: native Windows; Linux/Ubuntu remains a supported build target.
- Desktop stack: Tauri 2 + Rust + React + TypeScript + Vite.
- Package manager: pnpm, pinned through the root `package.json` `packageManager` field and a committed lockfile.
- ROS environment: Pixi with RoboStack Jazzy channels and `win-64`/`linux-64` platform declarations.
- No ROS package dependency is added in the skeleton until the first concrete ROS command or message workflow is chosen.
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

The exact source URLs and the verification date are recorded in `docs/versions.md`.

## Architecture

The repository is a single pnpm workspace. The frontend is an independently buildable Vite package under `frontend/`; the Tauri crate is under `src-tauri/`. Tauri serves `frontend/dist` and exposes a single `ping` command as the first typed IPC seam. Future concerns are represented by small `ros`, `environment`, `process`, and `platform` module boundaries on both sides without implementing ROS behavior yet.

Pixi is represented by a root `pixi.toml` with only workspace metadata, channels, and target platforms. A later task can add `ros-jazzy-*` packages after confirming the exact command or message requirements.

## Error and platform policy

- Frontend startup must render a local shell even when the ROS environment is absent.
- Native environment/process errors will later cross the Tauri boundary as structured error values; the initial `ping` command is intentionally infallible.
- Windows uses the Tauri WebView2 runtime and MSVC Rust target. Ubuntu uses the GTK/WebKit development dependencies documented by Tauri.
- Packaging assets and installers are deferred until the shell and ROS process lifecycle are implemented.

## Verification

The initial acceptance checks are:

1. `pnpm install --frozen-lockfile` succeeds on the native Windows checkout.
2. `pnpm --dir frontend typecheck` succeeds.
3. `pnpm --dir frontend build` succeeds and emits `frontend/dist`.
4. `cargo check --manifest-path src-tauri/Cargo.toml` succeeds when the Rust toolchain is installed; if it cannot run in the current machine, the missing prerequisite is reported explicitly.
5. Git records the complete scaffold and lock/version files.

