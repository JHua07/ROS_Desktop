# ROS Desktop Skeleton Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a minimal cross-platform Tauri/React/Pixi ROS Desktop workspace under `D:\ROS_Desktop`.

**Architecture:** A pnpm workspace contains the Vite frontend and a sibling Tauri Rust crate. The frontend has typed future module boundaries; the Rust crate exposes one `ping` command. Pixi declares only RoboStack Jazzy channels and target platforms so dependency selection remains deliberate.

**Tech Stack:** Tauri 2.12.0, Rust 1.98.1 stable, Node 24 LTS, pnpm 12.7.0, React 19.3.0, TypeScript 7.0.2, Vite 8.3.1, Pixi 0.81.0, RoboStack Jazzy.

**Spec:** `doc/specs/2026-09-28-ros-desktop-skeleton-design.md`

## Global Constraints

- The app must build its frontend without Rust, Pixi, or ROS being installed.
- Pixi must not add a ROS package until a concrete workflow establishes the package name.
- Windows and Ubuntu are the target desktop platforms.
- Tauri's frontend distribution is `../frontend/dist` relative to `src-tauri`.
- Version pins in the spec must be reflected in source manifests or version notes.

## Review Focus

- A missing ROS/Pixi installation must not prevent the frontend build; covered by the frontend-only build task.
- A Windows path with spaces must not be embedded in runtime configuration; covered by the path-independent Vite/Tauri configuration.
- The Tauri dev/build commands must resolve from a sibling `frontend` directory; covered by the Tauri configuration and cargo metadata checks.
- The Pixi manifest must remain solvable metadata without guessed ROS packages; covered by manifest inspection.
- A stale tool version must be visible to a developer; covered by the version report and lock files.

### Task 1: Repository and version contract

**Files:**
- Create: `.gitignore`, `.node-version`, `package.json`, `pnpm-workspace.yaml`, `rust-toolchain.toml`, `pixi.toml`
- Create: `doc/versions.md`, `scripts/check-versions.ps1`, `scripts/README.md`

**Interfaces:** Produces the root package-manager contract, toolchain pins, and Pixi workspace metadata consumed by later tasks.

- [x] Create the root manifests and version documentation using the exact values from the spec.
- [x] Run a read-only manifest inspection that confirms the package manager, Node baseline, Rust channel, and Pixi target platforms.
- [x] Commit as `chore: establish workspace version contract`.

### Task 2: Frontend shell and typed module boundaries

**Files:**
- Create: `frontend/package.json`, `frontend/index.html`, `frontend/vite.config.ts`, `frontend/tsconfig.json`, `frontend/tsconfig.app.json`, `frontend/tsconfig.node.json`
- Create: `frontend/src/main.tsx`, `frontend/src/App.tsx`, `frontend/src/styles.css`
- Create: `frontend/src/modules/{ros,environment,process,platform}/index.ts`

**Interfaces:** Consumes Task 1's pnpm workspace. Produces `frontend/dist` and future TypeScript module seams.

- [x] Add the minimal React/Vite source and module types.
- [x] Run `pnpm install` and `pnpm --dir frontend typecheck`; the typecheck must initially fail before the source exists and pass after implementation.
- [x] Run `pnpm --dir frontend build` and confirm `frontend/dist/index.html` exists.
- [x] Commit as `feat: add frontend shell`.

### Task 3: Tauri Rust shell and IPC seam

**Files:**
- Create: `src-tauri/Cargo.toml`, `src-tauri/build.rs`, `src-tauri/tauri.conf.json`
- Generate: `src-tauri/Cargo.lock` when Cargo is available on the development machine
- Create: `src-tauri/capabilities/default.json`, `src-tauri/src/main.rs`, `src-tauri/src/lib.rs`, `src-tauri/src/modules/mod.rs`
- Create: `src-tauri/src/modules/{ros,environment,process,platform}.rs`

**Interfaces:** Consumes Task 2's `frontend/dist`. Produces the `ping` Tauri command and a Rust module layout for future ROS/process integration.

- [x] Add the Tauri crate, configuration, capability, `ping` command, and empty module boundaries.
- [x] Run `cargo check --manifest-path src-tauri/Cargo.toml`; expect success when Rust is installed, otherwise record the exact missing-tool result.
  - Verified 2026-09-29 after installing Rust 1.98.1 + MSVC Build Tools. Two fixes were required: `tauri-build` had to move from `=2.6.0` to `=2.7.0` (tauri 2.12.0 requires `^2.7.0`), and indexmap's `std` feature is forced in `Cargo.toml` because its build.rs `has_std` autocfg probe (stdin pipe to rustc) fails in restricted environments (Windows error 231). A minimal `src-tauri/icons/icon.ico` was added because tauri-build requires it for the Windows resource file.
- [x] Commit as `feat: add tauri rust shell`.

### Task 4: Documentation and repository acceptance

**Files:**
- Create: `README.md`, `doc/architecture.md`

**Interfaces:** Documents the commands and platform prerequisites from Tasks 1–3.

- [x] Document Windows-first development, Ubuntu validation, the current verification boundary, and the future ROS module plan.
- [x] Run the complete frontend build and repository status checks.
- [x] Commit as `docs: document workspace and verification`.
