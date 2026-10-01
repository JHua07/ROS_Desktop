export interface EnvironmentStatus {
  available: boolean;
  pixi_path?: string;
  manifest_path: string;
  ros_distro?: string;
  /** Workspace-local colcon install overlays, e.g. custom message packages. */
  overlay_packages?: string[];
  error?: string;
}
