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

The frontend remains useful in browser mode, so it can be built and styled before a ROS environment is installed. Native commands will be introduced behind typed IPC functions as each workflow becomes concrete.

## Module ownership

| Module | Frontend responsibility | Rust responsibility |
| --- | --- | --- |
| `ros` | Domain names and capability models for topics, nodes, services, actions, parameters, bags, and launch | ROS CLI/native graph adapters |
| `environment` | Display platform, Pixi, and ROS availability | Locate Pixi, resolve workspace paths, and inspect environment metadata |
| `process` | Render lifecycle and logs for managed commands | Spawn, stream, stop, and clean up child processes |
| `platform` | Supported platform and architecture labels | Windows/Linux path, shell, signal, and executable differences |

The current files only define boundaries and small types. They do not claim that ROS commands are available.

## Process rules for future work

1. Resolve an executable and workspace before starting a ROS command.
2. Keep stdout, stderr, exit status, and cancellation state separate.
3. Return structured errors over IPC with an actionable message for the UI.
4. Keep Windows and Linux command construction behind the platform module.
5. Add only the RoboStack packages required by a tested workflow, then commit the resulting `pixi.lock`.

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
