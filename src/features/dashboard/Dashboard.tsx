import { useQuery } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import {
  ArrowUpRight,
  Boxes,
  FolderOpen,
  HardDrive,
  Link2,
  Plus,
  RefreshCw,
  ShieldCheck,
  TriangleAlert,
  UsersRound,
} from "lucide-react";
import { StatusBadge } from "../../components/common/StatusBadge";
import {
  buttonPrimary,
  iconButton,
  mutedCopy,
  panelCard,
  sectionHeading,
  sectionKicker,
  shortcutIcon,
} from "../../lib/ui";
import type { FirefoxProfile, ScanResult, ShortcutView } from "../../types";

interface DashboardProps {
  scan?: ScanResult;
  shortcuts: ShortcutView[];
  onCreateShortcut: () => void;
  onOpenShortcuts: () => void;
  onScan: () => void;
  onOpenStartMenu: () => void;
  onOpenProfile: (profile: FirefoxProfile) => void;
}

export function Dashboard({
  scan,
  shortcuts,
  onCreateShortcut,
  onOpenShortcuts,
  onScan,
  onOpenStartMenu,
  onOpenProfile,
}: DashboardProps) {
  const profiles = scan?.profiles ?? [];
  const containers = profiles.reduce(
    (total, profile) => total + profile.containers.length,
    0,
  );
  const primaryProfile =
    profiles.find((profile) => profile.isDefault) ?? profiles[0];
  const appChromeStatus = useQuery({
    queryKey: ["app-chrome", primaryProfile?.id],
    queryFn: () => invoke<boolean>("app_chrome_status", { profileId: primaryProfile!.id }),
    enabled: !!primaryProfile && !primaryProfile.error,
    retry: false,
  });
  const requiredExtensions =
    primaryProfile?.extensions.filter((extension) => extension.required) ?? [];
  const missingRequired = requiredExtensions.filter(
    (extension) =>
      extension.status === "notInstalled" ||
      extension.status === "installedDisabled" ||
      extension.status === "detectionFailed",
  ).length;
  const recentShortcuts = [...shortcuts]
    .sort((left, right) => right.createdAt - left.createdAt)
    .slice(0, 4);

  return (
    <div className="grid gap-4">
      <section className="relative flex min-h-[208px] items-center overflow-hidden rounded-md border border-primary/15 bg-[radial-gradient(ellipse_at_90%_15%,color-mix(in_oklch,var(--accent-warm)_22%,transparent),transparent_29%),radial-gradient(ellipse_at_88%_96%,color-mix(in_oklch,var(--primary)_13%,transparent),transparent_38%),linear-gradient(115deg,var(--card),color-mix(in_oklch,var(--primary)_4.5%,transparent))] p-8 shadow-[var(--shadow-card)] max-[680px]:min-h-[200px] max-[680px]:p-4">
        <div className="relative z-[1] w-[min(62%,520px)] max-[680px]:w-[82%]">
          <span className={`${sectionKicker} text-primary`}>
            A better way to launch
          </span>
          <h2 className="my-2 max-w-[440px] text-[clamp(24px,2.4vw,32px)] font-semibold leading-[1.14] tracking-[-0.045em] max-[680px]:text-2xl">
            Every site, in the right container.
          </h2>
          <p className="mb-4 mt-0 max-w-[440px] text-sm text-muted-foreground">
            Create Start menu shortcuts that open websites in Firefox
            containers or directly in a profile.
          </p>
          <button
            type="button"
            className={buttonPrimary}
            onClick={onCreateShortcut}
          >
            <Plus size={17} aria-hidden="true" />
            Create a shortcut
          </button>
        </div>
        <div
          className="absolute inset-y-0 right-0 w-[40%] min-w-[240px] overflow-hidden max-[680px]:-right-32 max-[680px]:w-[64%] max-[680px]:opacity-[0.45]"
          aria-hidden="true"
        >
          <div className="absolute right-8 top-4 size-36 rounded-full bg-accent-warm/15" />
          <div className="absolute -bottom-16 right-32 size-[180px] rounded-full bg-primary/10" />
          <div className="absolute right-8 top-8 h-[140px] w-[208px] rotate-[3deg] overflow-hidden rounded-md border border-border bg-card shadow-[0_8px_32px_hsl(264_30%_20%_/_0.13)]">
            <div className="flex h-7 items-center gap-1 border-b border-border bg-muted/80 px-2">
              <span className="size-1.5 rounded-full bg-muted-foreground/40" />
              <span className="size-1.5 rounded-full bg-muted-foreground/40" />
              <span className="size-1.5 rounded-full bg-muted-foreground/40" />
              <div className="ml-1 h-2 flex-1 rounded bg-background" />
            </div>
            <div className="px-4 py-2">
              <div className="inline-flex rounded-md bg-primary/10 px-2 py-1 text-[11px] font-semibold text-primary">
                Work
              </div>
              <div className="mt-2 h-[5px] w-3/4 rounded bg-border" />
              <div className="mt-2 h-[5px] w-[56%] rounded bg-border" />
              <div className="mt-2 h-8 rounded-md bg-linear-[100deg] from-primary/10 to-accent-warm/14" />
            </div>
          </div>
        </div>
      </section>

      <section
        className="grid grid-cols-4 gap-4 max-[680px]:grid-cols-2"
        aria-label="Firefox overview"
      >
        <StatCard
          label="Firefox installations"
          value={scan?.installations.length ?? 0}
          icon={
            <span className="grid size-8 shrink-0 place-items-center rounded-md bg-primary/10 text-[13px] font-bold text-primary">
              <HardDrive size={16} aria-hidden="true" />
            </span>
          }
          detail={
            scan?.installations[0]?.version
              ? `Version ${scan.installations[0].version}`
              : "Detected on this PC"
          }
        />
        <StatCard
          label="Profiles"
          value={profiles.length}
          icon={
            <span className="grid size-8 shrink-0 place-items-center rounded-md bg-status-blue/10 text-[13px] font-bold text-status-blue">
              <UsersRound size={16} aria-hidden="true" />
            </span>
          }
          detail={primaryProfile?.name ?? "No profiles found"}
          onClick={primaryProfile ? () => onOpenProfile(primaryProfile) : undefined}
        />
        <StatCard
          label="Containers"
          value={containers}
          icon={
            <span className="grid size-8 shrink-0 place-items-center rounded-md bg-warning/12 text-[13px] font-bold text-warning">
              <Boxes size={16} aria-hidden="true" />
            </span>
          }
          detail="Across discovered profiles"
        />
        <StatCard
          label="Managed shortcuts"
          value={shortcuts.length}
          icon={
            <span className="grid size-8 shrink-0 place-items-center rounded-md bg-success/12 text-[13px] font-bold text-success">
              <Link2 size={16} aria-hidden="true" />
            </span>
          }
          detail="In your Start menu"
          onClick={onOpenShortcuts}
        />
      </section>

      <div className="grid grid-cols-[1fr_1.08fr] gap-4 max-[900px]:grid-cols-1">
        <section className={panelCard}>
          <div className={sectionHeading}>
            <div>
              <span className={sectionKicker}>Setup status</span>
              <h3 className="mb-0 mt-1 text-base font-semibold tracking-[-0.02em]">
                Firefox integration
              </h3>
            </div>
            {missingRequired === 0 && requiredExtensions.length > 0 ? (
              <span className="grid size-9 shrink-0 place-items-center rounded-md bg-success/12 text-success">
                <ShieldCheck size={18} aria-hidden="true" />
              </span>
            ) : (
              <span className="grid size-9 shrink-0 place-items-center rounded-md bg-warning/15 text-warning">
                <TriangleAlert size={18} aria-hidden="true" />
              </span>
            )}
          </div>
          {primaryProfile ? (
            <>
              <button
                type="button"
                className="flex w-full min-w-0 cursor-pointer items-center gap-2 rounded-md border border-border bg-muted/50 p-2 text-left text-foreground hover:border-primary/35"
                onClick={() => onOpenProfile(primaryProfile)}
              >
                <span className="grid size-9 shrink-0 place-items-center rounded-md bg-primary/10 text-sm font-semibold text-primary">
                  {primaryProfile.name.slice(0, 1).toUpperCase()}
                </span>
                <span className="flex min-w-0 flex-1 flex-col">
                  <strong className="overflow-hidden text-[13px] text-ellipsis whitespace-nowrap">
                    {primaryProfile.name}
                  </strong>
                  <small className="overflow-hidden text-xs text-muted-foreground text-ellipsis whitespace-nowrap">
                    {primaryProfile.path}
                  </small>
                </span>
                <ArrowUpRight size={16} aria-hidden="true" />
              </button>
              <div className="mt-4 grid gap-2">
                {requiredExtensions.length ? (
                  requiredExtensions.map((extension) => (
                    <div
                      className="flex items-center justify-between gap-2"
                      key={extension.id}
                    >
                      <span className="min-w-0 overflow-hidden text-[13px] text-ellipsis whitespace-nowrap">
                        {extension.name}
                      </span>
                      <StatusBadge status={extension.status} />
                    </div>
                  ))
                ) : (
                  <p className={mutedCopy}>
                    Extension status is not available for this profile yet.
                  </p>
                )}
                <div className="flex items-center justify-between gap-2">
                  <button
                    type="button"
                    className="min-w-0 cursor-pointer truncate p-0 text-left text-[13px] text-foreground hover:text-primary hover:underline"
                    onClick={() => onOpenProfile(primaryProfile)}
                  >
                    Compact app toolbar
                  </button>
                  <StatusBadge status={
                    primaryProfile.error || appChromeStatus.isError
                      ? "detectionFailed"
                      : appChromeStatus.isPending
                        ? "checking"
                        : appChromeStatus.data ? "installedEnabled" : "notInstalled"
                  } />
                </div>
              </div>
              {missingRequired > 0 && (
                <button
                  type="button"
                  className="mt-4 inline-flex cursor-pointer items-center gap-1 p-0 text-[13px] font-semibold text-primary hover:underline"
                  onClick={() => onOpenProfile(primaryProfile)}
                >
                  Review Firefox setup
                  <ArrowUpRight size={14} aria-hidden="true" />
                </button>
              )}
            </>
          ) : (
            <div>
              <p className={mutedCopy}>No Firefox profiles were discovered.</p>
              <button
                type="button"
                className="mt-4 inline-flex cursor-pointer items-center gap-1 p-0 text-[13px] font-semibold text-primary hover:underline"
                onClick={onScan}
              >
                Scan Firefox
                <RefreshCw size={14} aria-hidden="true" />
              </button>
            </div>
          )}
        </section>

        <section className={panelCard}>
          <div className={sectionHeading}>
            <div>
              <span className={sectionKicker}>Latest activity</span>
              <h3 className="mb-0 mt-1 text-base font-semibold tracking-[-0.02em]">
                Recent shortcuts
              </h3>
            </div>
            <button
              type="button"
              className={iconButton}
              aria-label="Open all shortcuts"
              onClick={onOpenShortcuts}
            >
              <ArrowUpRight size={17} aria-hidden="true" />
            </button>
          </div>
          {recentShortcuts.length ? (
            <div className="grid">
              {recentShortcuts.map((shortcut) => (
                <button
                  className="group flex min-w-0 cursor-pointer items-center gap-2 border-b border-border bg-transparent py-2 text-left text-foreground last:border-b-0"
                  type="button"
                  key={shortcut.id}
                  onClick={onOpenShortcuts}
                >
                  <ShortcutIcon shortcut={shortcut} />
                  <span className="flex min-w-0 flex-1 flex-col">
                    <strong className="overflow-hidden text-[13px] text-ellipsis whitespace-nowrap group-hover:text-primary">
                      {shortcut.name}
                    </strong>
                    <small className="text-xs text-muted-foreground">
                      {shortcut.profileName} ·{" "}
                      {shortcut.containerName ?? "No container"}
                    </small>
                  </span>
                  <span className="max-w-36 overflow-hidden text-xs text-muted-foreground text-ellipsis whitespace-nowrap">
                    {domain(shortcut.targetUrl)}
                  </span>
                </button>
              ))}
            </div>
          ) : (
            <div>
              <p className={mutedCopy}>Your new shortcuts will appear here.</p>
              <button
                type="button"
                className="mt-4 inline-flex items-center gap-1 p-0 text-[13px] font-semibold text-primary hover:underline"
                onClick={onCreateShortcut}
              >
                Create your first shortcut
                <Plus size={14} aria-hidden="true" />
              </button>
            </div>
          )}
        </section>
      </div>

      <section className="grid grid-cols-3 gap-4 max-[900px]:grid-cols-1">
        <button
          type="button"
          className="flex min-w-0 cursor-pointer items-center gap-2 rounded-md border border-border bg-card p-4 text-left text-foreground transition-colors duration-[120ms] hover:border-primary/40 hover:bg-primary/5"
          onClick={onCreateShortcut}
        >
          <span className="grid size-9 shrink-0 place-items-center rounded-md bg-primary/10 text-primary">
            <Plus size={17} aria-hidden="true" />
          </span>
          <span className="grid min-w-0 flex-1 gap-1">
            <strong className="overflow-hidden text-[13px] text-ellipsis whitespace-nowrap">
              Create a shortcut
            </strong>
            <small className="overflow-hidden text-xs text-muted-foreground text-ellipsis whitespace-nowrap">
              Open a website in a container
            </small>
          </span>
          <ArrowUpRight className="shrink-0 text-muted-foreground" size={16} aria-hidden="true" />
        </button>
        <button
          type="button"
          className="flex min-w-0 cursor-pointer items-center gap-2 rounded-md border border-border bg-card p-4 text-left text-foreground transition-colors duration-[120ms] hover:border-primary/40 hover:bg-primary/5"
          onClick={onScan}
        >
          <span className="grid size-9 shrink-0 place-items-center rounded-md bg-status-blue/10 text-status-blue">
            <RefreshCw size={17} aria-hidden="true" />
          </span>
          <span className="grid min-w-0 flex-1 gap-1">
            <strong className="overflow-hidden text-[13px] text-ellipsis whitespace-nowrap">
              Scan Firefox
            </strong>
            <small className="overflow-hidden text-xs text-muted-foreground text-ellipsis whitespace-nowrap">
              Refresh installations and containers
            </small>
          </span>
          <ArrowUpRight className="shrink-0 text-muted-foreground" size={16} aria-hidden="true" />
        </button>
        <button
          type="button"
          className="flex min-w-0 cursor-pointer items-center gap-2 rounded-md border border-border bg-card p-4 text-left text-foreground transition-colors duration-[120ms] hover:border-primary/40 hover:bg-primary/5"
          onClick={onOpenStartMenu}
        >
          <span className="grid size-9 shrink-0 place-items-center rounded-md bg-warning/12 text-warning">
            <FolderOpen size={17} aria-hidden="true" />
          </span>
          <span className="grid min-w-0 flex-1 gap-1">
            <strong className="overflow-hidden text-[13px] text-ellipsis whitespace-nowrap">
              Open Start menu folder
            </strong>
            <small className="overflow-hidden text-xs text-muted-foreground text-ellipsis whitespace-nowrap">
              View your managed shortcuts
            </small>
          </span>
          <ArrowUpRight className="shrink-0 text-muted-foreground" size={16} aria-hidden="true" />
        </button>
      </section>

      {scan?.diagnostics.length ? (
        <section className="flex items-start gap-2 rounded-md border border-warning/20 bg-warning-soft p-4 text-[13px] text-warning">
          <TriangleAlert className="mt-1 shrink-0" size={15} aria-hidden="true" />
          <span>
            {scan.diagnostics.length} Firefox discovery{" "}
            {scan.diagnostics.length === 1 ? "note" : "notes"} need attention.
          </span>
        </section>
      ) : null}
    </div>
  );
}

function StatCard({
  label,
  value,
  icon,
  detail,
  onClick,
}: {
  label: string;
  value: number;
  icon: React.ReactNode;
  detail: string;
  onClick?: () => void;
}) {
  const className = `min-w-0 rounded-md border border-border bg-card p-4 text-left text-foreground shadow-[var(--shadow-card)] ${
    onClick
      ? "cursor-pointer transition-[border-color,background-color] duration-[120ms] hover:border-primary/40 hover:bg-primary/5"
      : ""
  }`;
  const content = (
    <>
      <div className="flex items-center justify-between gap-2 text-xs font-medium text-muted-foreground max-[680px]:items-start">
        <span>{label}</span>
        {icon}
      </div>
      <div className="mt-2 text-[28px] font-semibold leading-[1.1] tracking-[-0.04em] max-[680px]:text-2xl">
        {value}
      </div>
      <div className="mt-1 overflow-hidden text-xs text-muted-foreground text-ellipsis whitespace-nowrap max-[680px]:whitespace-normal">
        {detail}
      </div>
    </>
  );
  return onClick ? (
    <button className={className} type="button" onClick={onClick}>
      {content}
    </button>
  ) : (
    <div className={className}>{content}</div>
  );
}

function ShortcutIcon({
  shortcut,
  sizeClass = "size-9",
}: {
  shortcut: ShortcutView;
  sizeClass?: string;
}) {
  return shortcut.iconPreviewDataUrl ? (
    <img
      className={`${shortcutIcon} ${sizeClass}`}
      src={shortcut.iconPreviewDataUrl}
      alt=""
    />
  ) : (
    <span
      className={`${shortcutIcon} ${sizeClass} grid place-items-center text-sm font-bold text-primary`}
    >
      {shortcut.websiteName.slice(0, 1).toUpperCase()}
    </span>
  );
}

function domain(value: string): string {
  try {
    return new URL(value).hostname.replace(/^www\./i, "");
  } catch {
    return value;
  }
}
