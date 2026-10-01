# VLPos bag playback and custom message support (2026-10-01)

**Goal:** Play the VLPos mcap bag (`E:/data/VLPos_Data/0902-1/260902-172656`, 51 GB, 293 s, 309,259 messages) and deserialize every topic it contains, including the custom `vlpos/msg/H2oPointCloud` lidar type.

## What the bag contains

IMU ×6 (`/vlpos/imu`, `imu1`–`imu5`, `sensor_msgs/msg/Imu`, ~200 Hz, `imu5` empty), lidar ×5 (`/vlpos/lidar1`–`lidar5` `/h2o`, `vlpos/msg/H2oPointCloud`, ~10 Hz), camera (`/vlpos/camera/image_raw`, `sensor_msgs/msg/Image`, ~10 Hz), `/tf_static` (5 messages), plus system topics. Recorded on Jazzy with embedded type descriptions (RIHS hashes).

## Changes

- `ros/vlpos/` — pure interface package (package name must stay exactly `vlpos` for `vlpos/msg/*` type resolution). Contains all 7 message definitions ported byte-identical from the VLPos source (`H2oPointCloud`, `H2oPoint`, `H2oPointf`, `CompressedPointCloud2`, `Pose6D`, `SbgImuData`, `SbgImuStatus`).
- `ros/build_vlpos.sh` — one-shot rebuild script (MSVC + pixi `build` environment + colcon).
- `pixi.toml` — new `[environments.build]` (colcon-common-extensions, ninja, ros-jazzy-ros-core) kept separate from the default runtime environment; `ros-jazzy-tf2-msgs` added to default (ros-core does not include tf2, and `/tf_static` playback needs it).
- `ros/install/vlpos/` — build output (gitignored): typesupport DLLs, Python package, share resources.

## Verification record (2026-10-01)

- `ros2 bag info` reads the 51 GB mcap in ~1.2 s; topic counts match metadata.
- Playback acceptance (default env + vlpos overlay): `ros2 topic hz /vlpos/lidar1/h2o` ≈ 10.0 Hz; `ros2 topic echo /vlpos/lidar1/h2o --once` deserializes `header/point_num/scale/points` with all seven point fields (18,849 points in the sampled frame); `ros2 topic echo /tf_static --once` outputs the TFMessage transforms; play exits 0 with zero "Ignoring a topic" warnings (previously 6: vlpos ×5 + tf_static); no leftover processes.
- IMU verified at 199.8 Hz and camera headers readable before the adaptation, using only the pre-existing runtime packages.
- The computed RIHS hash of the ported `H2oPointCloud` equals the bag's recorded `RIHS01_87ef0e517eca311634bb6f8822080997df5bd12316e56bf47bd4cb832c284c9a`.

## Environment overlay (CLI; GUI wiring pending)

RoboStack's bash shell-hook sets no ROS variables, and Git Bash cannot run the generated `.bat/.ps1` setup scripts. CLI access to vlpos types therefore needs, on top of the pixi environment: `AMENT_PREFIX_PATH` prefixed with `D:\ROS_Desktop\ros\install\vlpos` (Windows style, `;`-separated, overlay first), `PYTHONPATH` prefixed with `ros\install\vlpos\Lib\site-packages`, `PATH` prefixed with `ros\install\vlpos\bin` (typesupport DLLs), and `ROS_OS_OVERRIDE=conda:win64`. Topic arguments starting with `/` require `MSYS_NO_PATHCONV=1` in Git Bash. The Jazzy play-duration flag is `--playback-duration`. Playing the 51 GB bag at full rate emits `Message queue starved` warnings; raise `--read-ahead-queue-size` if that matters.

## Incident and lessons (2026-10-01)

Two interrupted pixi environment synchronizations (one TaskStop during a legitimate post-add reinstall, one Bash timeout during `pixi remove` of a duplicate alias) left the default environment partially synced. Symptoms: subsequent `pixi install`/`remove` attempts fail fast on "failed to move file … to .trash" for one file at a time, while the named files disappear between attempts — retries make forward progress and eventually complete the sync. Rules learned:

1. Never interrupt a pixi environment sync (add/remove/install/first shell-hook after a manifest change); a broken env fails silently (hanging shell-hooks) rather than reporting corruption.
2. Duplicate conda aliases (`ros2-tf2-msgs` vs `ros-jazzy-tf2-msgs` are the same package) must not both be pinned; this repo uses the `ros-jazzy-*` spelling.
3. Prefer direct environment construction (CONDA_PREFIX + PATH + AMENT_PREFIX_PATH) over `pixi shell-hook` when diagnosing, so a syncing environment cannot hang the probe.

## Follow-ups

1. Wire the vlpos overlay into the GUI environment capture (`src-tauri/src/modules/environment.rs`) so desktop echo/playback workflows see the custom types.
2. Add a bag playback workflow (`ros2 bag play`) to the GUI, including `--playback-duration`, rate control, and read-ahead tuning.
3. Validate the rebuilt default environment end to end (daemon, graph query, echo) after the sync repair.
