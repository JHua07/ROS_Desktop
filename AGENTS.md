# Agent instructions

Before inspecting, modifying, testing, or extending this repository, read the project documents under `doc/`.

Required reading order:

1. `doc/README.md`
2. `doc/architecture.md`
3. `doc/versions.md`
4. Every relevant file under `doc/specs/`
5. Every relevant file under `doc/plans/`

Treat the documents in `doc/` as the project plan and design context. Keep implementation decisions consistent with them, and update the relevant document when the agreed architecture, version baseline, scope, or acceptance criteria changes. Direct user instructions take precedence over these documents.

## Standing architecture decisions (details in doc/architecture.md)

- ROS graph queries (topics/nodes/services/counts) MUST go through the ros2 daemon XML-RPC endpoint (`127.0.0.1:11511 + ROS_DOMAIN_ID`, path `/ros2cli/`), never through `pixi run` subprocesses (~25 ms vs 7–10 s measured).
- The Rust side owns the daemon lifecycle: activate the Pixi environment once at startup (shell-hook style), supervise the daemon, and bypass system proxies for localhost XML-RPC calls.
- Subprocesses are reserved for streaming/long-running operations (`topic echo`, `launch`, `bag`); `pixi run` must not appear in any GUI code path.
