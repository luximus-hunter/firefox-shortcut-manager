export type ThemePreference = "system" | "light" | "dark";

export type ExtensionDetectionState =
  | "installedEnabled"
  | "installedDisabled"
  | "installedUnknown"
  | "notInstalled"
  | "detectionFailed";

export type ShortcutStatus =
  | "ready"
  | "missingShortcut"
  | "missingFirefox"
  | "missingProfile"
  | "missingContainer"
  | "missingTaskbarTab"
  | "missingHandler"
  | "detectionFailed";

export interface AppSettings {
  schemaVersion: number;
  theme: ThemePreference;
  defaultInstallationId: string | null;
  defaultProfileId: string | null;
  autoScan: boolean;
  customFirefoxPaths: string[];
}

export interface FirefoxInstallation {
  id: string;
  name: string;
  executable: string;
  version: string | null;
  kind: string;
}

export interface FirefoxContainer {
  id: string;
  userContextId: number;
  name: string;
  color: string;
  icon: string;
}

export interface ExtensionStatus {
  id: string;
  name: string;
  purpose: string;
  status: ExtensionDetectionState;
  version: string | null;
  required: boolean;
  downloadUrl: string;
}

export interface FirefoxProfile {
  id: string;
  name: string;
  path: string;
  installationId: string | null;
  isDefault: boolean;
  containers: FirefoxContainer[];
  containerState: "available" | "empty" | "failed" | "notAvailable";
  extensions: ExtensionStatus[];
  error: string | null;
}

export interface DetectionIssue {
  source: string;
  message: string;
}

export interface ScanResult {
  installations: FirefoxInstallation[];
  profiles: FirefoxProfile[];
  diagnostics: DetectionIssue[];
}

export interface ShortcutView {
  id: string;
  name: string;
  websiteName: string;
  targetUrl: string;
  installationId: string;
  profileId: string;
  profileName: string;
  containerId: string | null;
  containerName: string | null;
  openAsApp: boolean;
  protocolUrl: string;
  shortcutPath: string;
  iconPath: string;
  iconPreviewDataUrl: string | null;
  iconBackgroundColor: string | null;
  iconUsedFallback: boolean;
  iconWarning: string | null;
  status: ShortcutStatus;
  createdAt: number;
}

export interface CreateShortcutRequest {
  websiteName: string;
  targetUrl: string;
  installationId: string;
  profileId: string;
  containerId: string | null;
  openAsApp: boolean;
  shortcutName: string;
  iconBackgroundColor: string | null;
}

export interface NativeError {
  code?: string;
  message?: string;
  details?: string | null;
}
