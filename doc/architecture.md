# Architecture

## Runtime shape

```text
React + TypeScript (frontend/)
        │  Tauri invoke/events
        ▼
Rust/Tauri (src-tauri/)
        │  platform-aware child processes
        ▼
Pixi workspace (pixi.toml)
        │
        ▼
RoboStack Jazzy / ROS 2 commands
```

The frontend remains useful in browser mode, so it can be built and styled before a ROS environment is installed. Native commands sit behind typed IPC functions; the graph overview and three managed streaming workflows are the current MVP.

## Module ownership

| Module | Frontend responsibility | Rust responsibility |
| --- | --- | --- |
| `ros` | Domain names and capability models for topics, nodes, services, actions, parameters, bags, and launch | ROS CLI/native graph adapters |
| `environment` | Display platform, Pixi, and ROS availability | Locate Pixi, resolve workspace paths, and inspect environment metadata |
| `process` | Render lifecycle and logs for managed commands | Spawn, stream, stop, and clean up child processes |
| `platform` | Supported platform and architecture labels | Windows/Linux path, shell, signal, and executable differences |

Availability is reported at runtime; a successful build does not by itself prove that Pixi, ROS, or the daemon is available.

## Process rules

1. Resolve an executable and workspace before starting a ROS command.
2. Keep stdout, stderr, exit status, and cancellation state separate.
3. Return structured errors over IPC with an actionable message for the UI.
4. Keep Windows and Linux command construction behind the platform module.
5. Add only the RoboStack packages required by a tested workflow and record the resulting `pixi.lock`.

## ROS access strategy (verified 2026-09-29)

Measured on Windows with Pixi 0.81.0 and `ros-jazzy-ros-core`; the ratios are
expected to hold on Linux.

| Access path | Latency (topic graph query) | Use for |
| --- | --- | --- |
| `pixi run ros2 ...` | 7–10 s | Never from the GUI; only for one-off manual checks |
| Direct `ros2` CLI after activation | ~2 s | Long-running operations only (`topic echo`, `launch`) |
| XML-RPC to the ros2 daemon | **~25 ms** | All ROS graph queries (topics, nodes, services, counts) |

Decisions:

1. **Graph queries go over XML-RPC, not subprocesses.** The ros2 daemon is an
   XML-RPC server on `127.0.0.1:11511 + ROS_DOMAIN_ID` at path `/ros2cli/`. It
   exposes `get_topic_names_and_types`, `get_node_names_and_namespaces`,
   `get_service_names_and_types`, `count_publishers`, and related methods. The
   Rust side should call it directly with plain HTTP — no Python startup cost,
   no pixi validation cost, and it is cross-platform by construction.
2. **The GUI owns the daemon lifecycle.** The daemon exits after 2 idle hours
   and instances spawned from short-lived shells are not reliable. The Rust
   process must start and supervise it (activate once via
   `pixi shell-hook`-equivalent environment setup, then spawn
   `ros2 daemon start` detached) and restart it on demand.
3. **Activate once, never per command.** `pixi run` costs ~5.8 s per call
   (binary startup plus lockfile validation). Equivalent of `source` is
   `pixi shell-hook`: capture the environment once at app start, build the
   child-process environment block from it, and spawn `ros2.exe` /
   the daemon directly afterwards.
4. **Localhost traffic must bypass system proxies.** A local proxy such as
   Clash (`HTTPS_PROXY=127.0.0.1:7890`) hijacks requests to 11511 and returns
   502. The XML-RPC client must ignore proxy environment variables.
5. Subprocess execution remains the path for operations that stream or persist
   (`ros2 topic echo`, `ros2 launch`, `ros2 bag`), following the process rules
   above.

## Four-workflow MVP extension (2026-09-29)

The existing XML-RPC graph bridge remains the only path for topic/node/service graph reads; the overview invokes `environment_status` and `ros_graph_snapshot` and works in browser-preview mode without native IPC. The three streaming workflows use the single Rust `ProcessManager` and direct `ros2` invocation from the one-time captured Pixi environment (on Windows, unbuffered Pixi Python executes `ros2-script.py` without the `.exe` wrapper). `modules/platform.rs` owns executable choice, process-group creation and graceful stop signaling. There is at most one active session per kind (echo, launch, bag); different kinds may run together.

The process contract is in [specs/2026-09-29-four-workflow-mvp.md](specs/2026-09-29-four-workflow-mvp.md). Tauri start commands return a typed session or `{code,message}`. `ros-process-event` reports stream lines, state, exit status and failure; stdout/stderr readers finish before the terminal event. The frontend retains 500 log lines per feature. Rust checks launch target containment, arguments, topic names and bag target nonexistence; commands pass discrete argv, never a user-built shell string. Application exit requests graceful stop for all managed sessions, waits boundedly, then forces surviving children and reports forced termination as failure because output (especially a bag) may be incomplete. On Windows, an isolated helper attaches to the managed child console and sends CTRL_C, reporting a nonzero helper exit as a stop error; a bounded grace period precedes forced termination. A CLI-level live smoke on 2026-09-29 confirmed echo output, launch startup, and finalized bag `metadata.yaml` after graceful stop. Tauri IPC/GUI end-to-end validation remains separate.
