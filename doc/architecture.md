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
