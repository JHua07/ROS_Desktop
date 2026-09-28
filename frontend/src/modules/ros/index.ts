export type RosDomain = "topic" | "node" | "service" | "action" | "parameter" | "bag" | "launch";

export interface RosCapability {
  domain: RosDomain;
  label: string;
  available: boolean;
}
