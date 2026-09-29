import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type ProcessKind = "topic_echo" | "launch" | "bag";
export type ProcessState = "starting" | "running" | "stopping" | "stopped" | "failed";
export interface ProcessSession { session_id: number; kind: ProcessKind; state: ProcessState }
export interface ProcessEvent extends ProcessSession {
  stream?: "stdout" | "stderr";
  line?: string;
  exit_code?: number;
  error?: string;
}
export interface ProcessError { code: string; message: string }
export interface LaunchArgument { name: string; value: string }

export const startTopicEcho = (topic: string): Promise<ProcessSession> => invoke("start_topic_echo", { topic });
export const startLaunch = (target: string, arguments_: LaunchArgument[]): Promise<ProcessSession> =>
  invoke("start_launch", { target, arguments: arguments_ });
export const listLaunchTargets = (): Promise<string[]> => invoke("list_launch_targets");
export const startBag = (topics: string[], output: string): Promise<ProcessSession> => invoke("start_bag", { topics, output });
export const stopRosProcess = (kind: ProcessKind): Promise<void> => invoke("stop_ros_process", { kind });
export const listenRosProcessEvents = (callback: (event: ProcessEvent) => void): Promise<UnlistenFn> =>
  listen<ProcessEvent>("ros-process-event", (event) => callback(event.payload));
export function describeProcessError(error: unknown): string {
  if (typeof error === "object" && error !== null && "message" in error) {
    return String((error as ProcessError).message);
  }
  return String(error);
}
