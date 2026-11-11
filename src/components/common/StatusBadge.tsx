import { badge } from "@/lib/ui";
import type { ExtensionDetectionState, ShortcutStatus } from "../../types";

type StatusValue = ExtensionDetectionState | ShortcutStatus | "checking";

const labels: Record<StatusValue, string> = {
  checking: "Checking…",
  installedEnabled: "Installed & enabled",
  installedDisabled: "Installed, disabled",
  installedUnknown: "Installed · state unknown",
  notInstalled: "Not installed",
  detectionFailed: "Couldn’t verify",
  ready: "Ready",
  missingShortcut: "Shortcut missing",
  missingFirefox: "Firefox missing",
  missingProfile: "Profile missing",
  missingContainer: "Container missing",
  missingTaskbarTab: "App not registered",
  missingHandler: "Handler missing",
};

function tone(status: StatusValue): string {
  if (status === "installedEnabled" || status === "ready") return "success";
  if (status === "installedUnknown") return "warning";
  if (
    status === "notInstalled" ||
    status === "installedDisabled" ||
    status === "missingShortcut" ||
    status === "missingFirefox" ||
    status === "missingProfile" ||
    status === "missingContainer" ||
    status === "missingTaskbarTab" ||
    status === "missingHandler"
  ) {
    return "danger";
  }
  return "neutral";
}

const toneClasses: Record<string, string> = {
  success: "text-success",
  warning: "text-warning",
  danger: "text-danger",
  neutral: "text-foreground",
};

export function StatusBadge({ status }: { status: StatusValue }) {
  return (
    <span
      className={`${badge} ${toneClasses[tone(status)]}`}
    >
      {labels[status]}
    </span>
  );
}
