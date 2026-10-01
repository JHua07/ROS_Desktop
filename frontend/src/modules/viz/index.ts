import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type VizKind = "pointcloud" | "image" | "imu";

export interface VizTopicConfig {
  topic: string;
  type: string;
  kind: VizKind;
  options?: { max_width?: number; downsample?: number };
}

export interface VizConfig {
  topics: VizTopicConfig[];
}

export interface VizSession {
  session_id: number;
  state: "running";
}

export interface VizFrameEvent {
  session_id: number;
  topic: string;
  kind: VizKind;
  seq: number;
  t_ns: number;
  meta: {
    frame_id?: string;
    point_num?: number;
    scale?: number;
    width?: number;
    height?: number;
    encoding?: string;
    bytes_per_pixel?: number;
  };
  data_b64: string;
}

export interface VizStatusEvent {
  session_id: number;
  state: "starting" | "running" | "stopped" | "failed";
  line?: string;
}

export interface VizError {
  code: string;
  message: string;
}

export function startViz(config: VizConfig): Promise<VizSession> {
  return invoke<VizSession>("start_viz", { config });
}

export function stopViz(): Promise<void> {
  return invoke<void>("stop_viz");
}

export function listenVizFrames(callback: (event: VizFrameEvent) => void): Promise<UnlistenFn> {
  return listen<VizFrameEvent>("ros-viz-frame", (event) => callback(event.payload));
}

export function listenVizStatus(callback: (event: VizStatusEvent) => void): Promise<UnlistenFn> {
  return listen<VizStatusEvent>("ros-viz-status", (event) => callback(event.payload));
}

export function decodeBase64(value: string): Uint8Array {
  const binary = atob(value);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes;
}

export interface VizFrameHub {
  publish(frame: VizFrameEvent): void;
  subscribe(topic: string, listener: (frame: VizFrameEvent) => void): () => void;
}

// High-rate topics (IMU runs at ~200 Hz) would thrash React state if frames flowed through
// component props. The hub fans frames out imperatively; views update canvas/WebGL buffers directly.
export function createVizFrameHub(): VizFrameHub {
  const listeners = new Map<string, Set<(frame: VizFrameEvent) => void>>();
  return {
    publish(frame) {
      const topicListeners = listeners.get(frame.topic);
      if (!topicListeners) return;
      for (const listener of topicListeners) listener(frame);
    },
    subscribe(topic, listener) {
      let topicListeners = listeners.get(topic);
      if (!topicListeners) {
        topicListeners = new Set();
        listeners.set(topic, topicListeners);
      }
      topicListeners.add(listener);
      return () => {
        topicListeners.delete(listener);
        if (topicListeners.size === 0) listeners.delete(topic);
      };
    },
  };
}

export function describeVizError(error: unknown): string {
  if (typeof error === "object" && error !== null && "message" in error) {
    return String((error as VizError).message);
  }
  return String(error);
}
