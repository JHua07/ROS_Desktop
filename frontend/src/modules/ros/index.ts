export type RosDomain = "topic" | "node" | "service" | "action" | "parameter" | "bag" | "launch";

export interface RosCapability {
  domain: RosDomain;
  label: string;
  available: boolean;
}

import { invoke } from "@tauri-apps/api/core";

export interface RosTopic {
  name: string;
  types: string[];
}

export interface RosNode {
  name: string;
  namespace: string;
}

export interface RosService {
  name: string;
  types: string[];
}

export interface RosGraphSnapshot {
  topics: RosTopic[];
  nodes: RosNode[];
  services: RosService[];
  topic_count: number;
  node_count: number;
  service_count: number;
}

export function fetchRosGraphSnapshot(): Promise<RosGraphSnapshot> {
  return invoke<RosGraphSnapshot>("ros_graph_snapshot");
}
