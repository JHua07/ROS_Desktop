export interface EnvironmentStatus {
  platform: "windows" | "linux";
  distro: "jazzy";
  pixiVersion?: string;
  rosAvailable: boolean;
}
