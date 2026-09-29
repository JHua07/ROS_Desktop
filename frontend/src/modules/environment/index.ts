export interface EnvironmentStatus {
  available: boolean;
  pixi_path?: string;
  manifest_path: string;
  ros_distro?: string;
  error?: string;
}
