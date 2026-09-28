export type SupportedPlatform = "windows" | "linux";

export interface PlatformInfo {
  platform: SupportedPlatform;
  architecture: string;
}
