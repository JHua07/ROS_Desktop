#!/usr/bin/env bash
# Build the vlpos interface package with colcon inside the pixi "build" env,
# using the system MSVC 2022 Build Tools toolchain.
# Usage: bash ros/build_vlpos.sh   (from repo root, Git Bash)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ROS_WS="$ROOT/ros"
VSBT="C:\\Program Files (x86)\\Microsoft Visual Studio\\2022\\BuildTools"
MSVC_VER="14.44.35207"
SDK_VER="10.0.26100.0"

# 1. Dump MSVC (vcvars64) environment if not cached in this shell
if [ -z "${MSVC_ENV_LOADED:-}" ]; then
  DUMP="$ROS_WS/.tmp_vcvars.dump"
  cat > "$ROS_WS/.tmp_vcvars.bat" <<'EOF'
@echo off
call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" >nul 2>&1
set
EOF
  cmd //c 'd:\ROS_Desktop\ros\.tmp_vcvars.bat' > "$DUMP" 2>/dev/null || true
  export INCLUDE="$(grep -m1 '^INCLUDE=' "$DUMP" | cut -d= -f2- | sed 's/\\\\/\\/g')"
  export LIB="$(grep -m1 '^LIB=' "$DUMP" | cut -d= -f2- | sed 's/\\\\/\\/g')"
  export LIBPATH="$(grep -m1 '^LIBPATH=' "$DUMP" | cut -d= -f2- | sed 's/\\\\/\\/g')"
  export PATH="/c/Program Files (x86)/Microsoft Visual Studio/2022/BuildTools/VC/Tools/MSVC/$MSVC_VER/bin/Hostx64/x64:/c/Program Files (x86)/Windows Kits/10/bin/$SDK_VER/x64:$PATH"
  rm -f "$DUMP" "$ROS_WS/.tmp_vcvars.bat"
  export MSVC_ENV_LOADED=1
fi

command -v cl >/dev/null 2>&1 || { echo "ERROR: cl.exe not found on PATH" >&2; exit 1; }

# 2. Run colcon inside the pixi "build" environment
cd "$ROS_WS"
~/.pixi/bin/pixi.exe run -e build colcon build \
  --packages-select vlpos \
  --cmake-args -DCMAKE_BUILD_TYPE=Release -G Ninja
