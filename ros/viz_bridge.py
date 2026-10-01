#!/usr/bin/env python3
"""Streaming visualization bridge: ROS topics -> VZF1 frames on stdout.

Usage:
    viz_bridge.py '<json-config>'   # subscribe and stream live topics
    viz_bridge.py --selftest        # emit three synthetic frames, exit 0

Protocol (frozen in doc/specs/2026-10-01-visualization.md):
    stdout  binary frames: b"VZF1" + u32LE header_len + JSON header +
            u32LE payload_len + payload; flushed after every frame.
    stderr  one lifecycle/status line per event.
    stdin   EOF or the line "stop" requests a clean shutdown (exit 0).

Frame headers and payloads by kind (all multi-byte values little-endian):
    pointcloud  header {topic, kind, seq, t_ns, frame_id, point_num, scale};
                payload = point_num records of 13 bytes (numpy structured
                dtype, align=False).
    image       header {topic, kind, seq, t_ns, frame_id, width, height,
                encoding, bytes_per_pixel}; payload = raw pixels converted
                to rgb8/gray8 with optional downsample/max_width reduction.
    imu         header {topic, kind, seq, t_ns, frame_id}; payload = 64
                bytes: f64 stamp (seconds) + f64[3] accel + f64[3] gyro.
"""

import importlib
import json
import math
import struct
import sys
import threading
import time

import numpy as np

MAGIC = b"VZF1"

# H2oPoint record: u32 ts, i16 x/y/z, u8 intensity/ring/tag (13 bytes).
POINT_DTYPE = np.dtype(
    [
        ("ts", "<u4"),
        ("x", "<i2"),
        ("y", "<i2"),
        ("z", "<i2"),
        ("intensity", "u1"),
        ("ring", "u1"),
        ("tag", "u1"),
    ]
)

# ROS message type per visualization kind.
KIND_TYPES = {
    "pointcloud": ("vlpos.msg", "H2oPointCloud"),
    "image": ("sensor_msgs.msg", "Image"),
    "imu": ("sensor_msgs.msg", "Imu"),
}

_write_lock = threading.Lock()


class UnsupportedEncoding(Exception):
    """Source image encoding the bridge cannot convert."""


def log(line):
    print(line, file=sys.stderr, flush=True)


def write_frame(header, payload):
    header_bytes = json.dumps(header, separators=(",", ":")).encode("utf-8")
    with _write_lock:
        stream = sys.stdout.buffer
        stream.write(MAGIC)
        stream.write(struct.pack("<I", len(header_bytes)))
        stream.write(header_bytes)
        stream.write(struct.pack("<I", len(payload)))
        stream.write(payload)
        stream.flush()


def stamp_ns(stamp):
    return stamp.sec * 1_000_000_000 + stamp.nanosec


def convert_pointcloud(topic, seq, msg):
    points = np.array(
        [
            (p.timestamp, p.x, p.y, p.z, p.intensity, p.ring, p.tag)
            for p in msg.points
        ],
        dtype=POINT_DTYPE,
    )
    header = {
        "topic": topic,
        "kind": "pointcloud",
        "seq": seq,
        "t_ns": stamp_ns(msg.header.stamp),
        "frame_id": msg.header.frame_id,
        "point_num": len(points),
        "scale": float(msg.scale),
    }
    return header, points.tobytes()


def convert_image(topic, seq, msg, options):
    encoding = msg.encoding.lower()
    width, height, step = int(msg.width), int(msg.height), int(msg.step)
    if width == 0 or height == 0 or step == 0:
        raise UnsupportedEncoding(f"invalid geometry {width}x{height} step={step}")

    bytes_per_pixel = {
        "rgb8": 3,
        "bgr8": 3,
        "rgba8": 4,
        "gray8": 1,
        "mono8": 1,
        "gray16": 2,
        "mono16": 2,
    }.get(encoding)
    if bytes_per_pixel is None:
        raise UnsupportedEncoding(f"unsupported image encoding: {msg.encoding}")
    if step < width * bytes_per_pixel:
        raise UnsupportedEncoding(f"step {step} too small for {width}px {encoding}")

    raw = np.frombuffer(bytes(msg.data), dtype=np.uint8)
    rows = raw[: step * height].reshape(height, step)
    if encoding == "rgb8":
        image, out_encoding = rows[:, : width * 3].reshape(height, width, 3), "rgb8"
    elif encoding == "bgr8":
        image = rows[:, : width * 3].reshape(height, width, 3)[:, :, ::-1]
        out_encoding = "rgb8"
    elif encoding == "rgba8":
        image = rows[:, : width * 4].reshape(height, width, 4)[:, :, :3]
        out_encoding = "rgb8"
    elif encoding in ("gray8", "mono8"):
        image, out_encoding = rows[:, :width], "gray8"
    else:  # gray16/mono16, rescaled to 8-bit
        if step % 2:
            raise UnsupportedEncoding(f"odd step {step} for {msg.encoding}")
        image = (rows.view("<u2")[:, :width] >> 8).astype(np.uint8)
        out_encoding = "gray8"

    # Effective stride: requested downsample, widened if max_width requires more.
    try:
        stride = max(1, min(8, int(options.get("downsample") or 1)))
        max_width = int(options.get("max_width") or 0)
        if max_width > 0:
            stride = max(stride, math.ceil(width / max_width))
    except (TypeError, ValueError):
        pass  # tolerate direct CLI use; the Rust side validates IPC configs
    if stride > 1:
        image = image[::stride, ::stride]

    header = {
        "topic": topic,
        "kind": "image",
        "seq": seq,
        "t_ns": stamp_ns(msg.header.stamp),
        "frame_id": msg.header.frame_id,
        "width": image.shape[1],
        "height": image.shape[0],
        "encoding": out_encoding,
        "bytes_per_pixel": 3 if out_encoding == "rgb8" else 1,
    }
    return header, image.tobytes()


def convert_imu(topic, seq, msg):
    stamp = msg.header.stamp
    accel, gyro = msg.linear_acceleration, msg.angular_velocity
    # Spec fixes the payload at 64 bytes: seven f64 values (stamp, accel xyz,
    # gyro xyz) followed by 8 bytes of zero padding.
    payload = struct.pack(
        "<7d",
        stamp.sec + stamp.nanosec * 1e-9,
        accel.x,
        accel.y,
        accel.z,
        gyro.x,
        gyro.y,
        gyro.z,
    ) + b"\x00" * 8
    header = {
        "topic": topic,
        "kind": "imu",
        "seq": seq,
        "t_ns": stamp_ns(stamp),
        "frame_id": msg.header.frame_id,
    }
    return header, payload


def run_bridge(config):
    import rclpy
    from rclpy.qos import HistoryPolicy, QoSProfile, ReliabilityPolicy

    def sensor_qos(depth):
        return QoSProfile(
            reliability=ReliabilityPolicy.BEST_EFFORT,
            history=HistoryPolicy.KEEP_LAST,
            depth=depth,
        )

    rclpy.init()
    node = rclpy.create_node("viz_bridge")
    stop = threading.Event()
    subscriptions = {}

    def handler_for(entry):
        topic, kind, options = entry["topic"], entry["kind"], entry["options"]

        def handler(msg):
            if stop.is_set():
                return
            try:
                seq = entry["seq"]
                if kind == "pointcloud":
                    header, payload = convert_pointcloud(topic, seq, msg)
                elif kind == "image":
                    header, payload = convert_image(topic, seq, msg, options)
                else:
                    header, payload = convert_imu(topic, seq, msg)
                entry["seq"] = seq + 1
                write_frame(header, payload)
            except BrokenPipeError:
                stop.set()  # the Rust side went away
            except UnsupportedEncoding as error:
                log(f"topic {topic} disabled: {error}")
                subscription = subscriptions.pop(topic, None)
                if subscription is not None:
                    node.destroy_subscription(subscription)
            except Exception as error:  # keep the bridge alive on per-message faults
                log(f"topic {topic} error: {error!r}")

        return handler

    log("viz_bridge started")
    for spec in config.get("topics") or []:
        topic, kind = spec.get("topic"), spec.get("kind")
        if not topic or kind not in KIND_TYPES:
            log(f"skipping malformed topic entry: {spec!r}")
            continue
        module_name, type_name = KIND_TYPES[kind]
        try:
            msg_type = getattr(importlib.import_module(module_name), type_name)
            qos = sensor_qos(10 if kind == "imu" else 5)
            entry = {
                "topic": topic,
                "kind": kind,
                "options": spec.get("options") or {},
                "seq": 0,
            }
            subscriptions[topic] = node.create_subscription(
                msg_type, topic, handler_for(entry), qos
            )
        except Exception as error:
            log(f"topic {topic} unavailable: {error}")
            continue
        log(
            f"subscribed {topic} ({type_name}, kind={kind}, "
            f"qos=best_effort depth={qos.depth})"
        )

    def watch_stdin():
        try:
            for line in sys.stdin:
                if line.strip() == "stop":
                    break
        except Exception:
            pass
        stop.set()

    threading.Thread(target=watch_stdin, daemon=True).start()

    try:
        while rclpy.ok() and not stop.is_set():
            rclpy.spin_once(node, timeout_sec=0.2)
    except KeyboardInterrupt:
        pass
    finally:
        try:
            node.destroy_node()
        except Exception:
            pass
        try:
            rclpy.shutdown()
        except Exception:
            pass
        log("viz_bridge stopped")
    return 0


def run_selftest():
    now = time.time_ns()

    points = np.array(
        [
            (
                i,
                (i * 73) % 400 - 200,
                (i * 151) % 400 - 200,
                (i * 31) % 400 - 200,
                i % 256,
                i % 8,
                0,
            )
            for i in range(16)
        ],
        dtype=POINT_DTYPE,
    )
    write_frame(
        {
            "topic": "/selftest/points",
            "kind": "pointcloud",
            "seq": 0,
            "t_ns": now,
            "frame_id": "selftest",
            "point_num": len(points),
            "scale": 0.001,
        },
        points.tobytes(),
    )

    width, height = 32, 24
    image = np.zeros((height, width, 3), dtype=np.uint8)
    image[..., 0] = np.arange(width, dtype=np.uint8)
    image[..., 1] = np.arange(height, dtype=np.uint8)[:, None]
    image[..., 2] = 128
    write_frame(
        {
            "topic": "/selftest/image",
            "kind": "image",
            "seq": 0,
            "t_ns": now,
            "frame_id": "selftest",
            "width": width,
            "height": height,
            "encoding": "rgb8",
            "bytes_per_pixel": 3,
        },
        image.tobytes(),
    )

    write_frame(
        {
            "topic": "/selftest/imu",
            "kind": "imu",
            "seq": 0,
            "t_ns": now,
            "frame_id": "selftest",
        },
        struct.pack(
            "<7d",
            now / 1e9,
            0.12,
            -0.34,
            9.81,
            0.005,
            -0.006,
            0.007,
        )
        + b"\x00" * 8,
    )
    return 0


def main(argv):
    if "--selftest" in argv[1:]:
        return run_selftest()
    if len(argv) != 2:
        log("usage: viz_bridge.py '<json-config>' | --selftest")
        return 2
    try:
        config = json.loads(argv[1])
    except ValueError as error:
        log(f"invalid config JSON: {error}")
        return 2
    return run_bridge(config)


if __name__ == "__main__":
    sys.exit(main(sys.argv))
