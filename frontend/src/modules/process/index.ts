export type ProcessState = "starting" | "running" | "stopping" | "stopped" | "failed";

export interface ManagedProcess {
  id: string;
  command: string;
  state: ProcessState;
}
