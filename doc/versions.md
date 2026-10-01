# Version baseline

Verified on 2026-09-28 for the Windows-first workspace in `D:\ROS_Desktop`.

| Tool or library | Pin | Verification/source |
| --- | --- | --- |
| Tauri | 2.12.0 | [Tauri 2.12 release](https://v2.tauri.app/blog/tauri-2.12/) |
| tauri-build | 2.7.0 | Pinned in `src-tauri/Cargo.toml`; the minimum required by Tauri 2.12.0 (`^2.7.0`) |
| Rust | 1.98.1 stable | [Rust latest release](https://blog.rust-lang.org/releases/latest/) |
| Node.js | 24.21.0 LTS baseline | [Node.js release table](https://nodejs.org/en/about/previous-releases) |
| pnpm | 12.7.0 | [pnpm 12.7 release](https://pnpm.io/blog/releases/12.7) |
| React / React DOM | 19.3.0 | [React 19.3 release](https://react.dev/blog/2026/09/09/react-19-3) |
| TypeScript | 7.0.2 | [TypeScript 7.0.2 release](https://github.com/microsoft/typescript-go/releases/tag/typescript/v7.0.2) |
| Vite | 8.3.1 | [Vite site and current release](https://vite.dev/) |
| Pixi | 0.81.0 | [Pixi installer version](https://github.com/prefix-dev/pixi/blob/main/install/install.sh) |
| ROS 2 | Jazzy Jalisco | [Jazzy LTS announcement](https://discourse.ros.org/t/ros-2-jazzy-jalisco-released/37862) |
| RoboStack | `robostack-jazzy` | [RoboStack Jazzy packages](https://robostack.github.io/jazzy.html) |
| three.js | 0.186.1 | Visualization 3D rendering; pinned in `frontend/package.json` with `@types/three` 0.186.0 |
| uPlot | 1.6.32 | IMU waveform chart; pinned in `frontend/package.json` (bundles its own types) |
| base64 (Rust) | 0.22 | Frame payload encoding; pinned in `src-tauri/Cargo.toml` |

The JavaScript package versions are also pinned in `package.json` files and resolved in `pnpm-lock.yaml`. The committed `pixi.lock` resolves `ros-jazzy-ros-core` for graph/echo/launch and `ros-jazzy-ros2bag` for recording. Add any further ROS packages only when a concrete workflow demonstrates the need.

## Platform notes

- Windows development needs Microsoft C++ Build Tools and WebView2.
- Ubuntu development needs the Tauri GTK/WebKit development packages listed in the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).
- Jazzy is the selected ROS baseline because it is an LTS release supported through May 2029 and RoboStack publishes its packages for both `win-64` and `linux-64`.
