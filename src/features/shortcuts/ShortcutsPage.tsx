import {
  Copy,
  ExternalLink,
  FolderOpen,
  Link2,
  Pencil,
  Play,
  Plus,
  RefreshCw,
  Search,
  TriangleAlert,
  Trash2,
} from "lucide-react";
import { useMemo, useState } from "react";
import { StatusBadge } from "../../components/common/StatusBadge";
import {
  badge,
  buttonPrimary,
  buttonSecondary,
  dangerHover,
  emptyIcon,
  iconButton,
  pageStack,
  panelCard,
  shortcutIcon,
} from "../../lib/ui";
import type { ShortcutView } from "../../types";

interface ShortcutsPageProps {
  shortcuts: ShortcutView[];
  onOpenStartMenu: () => void;
  onCreate: () => void;
  onEdit: (shortcut: ShortcutView) => void;
  onDuplicate: (shortcut: ShortcutView) => void;
  onLaunch: (shortcut: ShortcutView) => void;
  onRefreshIcon: (shortcut: ShortcutView) => void;
  refreshingIcon: boolean;
  onOpenLocation: (shortcut: ShortcutView) => void;
  onDelete: (shortcut: ShortcutView) => void;
}

export function ShortcutsPage({
  shortcuts,
  onOpenStartMenu,
  onCreate,
  onEdit,
  onDuplicate,
  onLaunch,
  onRefreshIcon,
  refreshingIcon,
  onOpenLocation,
  onDelete,
}: ShortcutsPageProps) {
  const [search, setSearch] = useState("");
  const filtered = useMemo(() => {
    const query = search.trim().toLowerCase();
    return shortcuts.filter((shortcut) => {
      const matchesQuery =
        !query ||
        shortcut.name.toLowerCase().includes(query) ||
        shortcut.websiteName.toLowerCase().includes(query) ||
        shortcut.targetUrl.toLowerCase().includes(query) ||
        (shortcut.containerName ?? "No container")
          .toLowerCase()
          .includes(query);
      return matchesQuery;
    });
  }, [search, shortcuts]);

  return (
    <div className={pageStack}>
      <section className={`${panelCard} grid gap-2`} aria-label="Shortcut actions">
        <div className="flex flex-wrap items-center gap-2">
          <label className="flex h-8 min-w-0 flex-1 items-center gap-2 border border-input bg-background px-2 text-muted-foreground focus-within:border-ring dark:bg-input/30">
            <Search size={14} aria-hidden="true" />
            <input
              className="w-full min-w-0 border-0 bg-transparent text-foreground placeholder:text-muted-foreground focus-visible:outline-none"
              value={search}
              onChange={(event) => setSearch(event.currentTarget.value)}
              placeholder="Search shortcuts"
              aria-label="Search shortcuts"
            />
          </label>
          <button type="button" className={buttonPrimary} onClick={onCreate}>
            <Plus size={16} aria-hidden="true" />
            New shortcut
          </button>
        </div>
        <div className="flex flex-wrap items-center justify-between gap-2 text-muted-foreground">
          <span className={badge}>
            {filtered.length} {filtered.length === 1 ? "shortcut" : "shortcuts"}
          </span>
          <button type="button" className="cursor-pointer underline underline-offset-4 hover:text-foreground" onClick={onOpenStartMenu}>Open Start menu folder</button>
        </div>
      </section>

      {filtered.length ? (
        <section className="divide-y divide-border bg-card ring-1 ring-foreground/10" aria-label="Managed shortcuts">
          {filtered.map((shortcut) => (
            <article
              className="flex min-w-0 items-center justify-between gap-4 bg-card p-4 max-[680px]:items-start max-[680px]:flex-col"
              key={shortcut.id}
            >
              <div className="flex min-w-0 flex-1 items-center gap-2 max-[680px]:w-full">
                <ShortcutIcon shortcut={shortcut} sizeClass="size-10" />
                <div className="flex min-w-0 flex-1 flex-col gap-1">
                  <div className="flex items-center gap-2">
                    <h3 className="m-0 overflow-hidden text-sm font-semibold text-ellipsis whitespace-nowrap">
                      {shortcut.name}
                    </h3>
                    <StatusBadge status={shortcut.status} />
                  </div>
                  <a
                    className="inline-flex w-fit max-w-full items-center gap-1 overflow-hidden text-xs text-muted-foreground text-ellipsis whitespace-nowrap no-underline"
                    href={shortcut.targetUrl}
                    target="_blank"
                    rel="noreferrer"
                    onClick={(event) => event.preventDefault()}
                  >
                    {shortcut.targetUrl}
                    <ExternalLink size={12} aria-hidden="true" />
                  </a>
                  <div className="flex flex-wrap items-center gap-1 text-xs text-muted-foreground">
                    <span>{shortcut.openAsApp ? "App window" : "Browser tab"}</span>
                    <span className="text-border">·</span>
                    <span>{shortcut.profileName}</span>
                    <span className="text-border">·</span>
                    {shortcut.containerName ? (
                      <span className="inline-flex items-center gap-2">
                        <span className="size-2 shrink-0 rounded-full bg-muted-foreground" />
                        {shortcut.containerName}
                      </span>
                    ) : (
                      <span className="inline-flex items-center gap-2">No container</span>
                    )}
                    <span className="text-border">·</span>
                    <span className="max-w-[240px] overflow-hidden text-ellipsis whitespace-nowrap max-[680px]:max-w-40">
                      {shortcut.shortcutPath}
                    </span>
                  </div>
                  {shortcut.iconUsedFallback && (
                    <div className="flex items-center gap-1 text-xs text-warning" role="note">
                      <TriangleAlert className="shrink-0" size={12} aria-hidden="true" />
                      <span>
                        {shortcut.iconWarning ?? "A generated icon is being used."}
                      </span>
                    </div>
                  )}
                  {shortcut.status === "missingTaskbarTab" && (
                    <div className="flex items-center gap-1 text-xs text-warning" role="note">
                      <TriangleAlert className="shrink-0" size={12} aria-hidden="true" />
                      <span>
                        Edit and save this app shortcut while its Firefox profile
                        is closed to register it.
                      </span>
                    </div>
                  )}
                </div>
              </div>
              <div className="flex items-center gap-1 max-[680px]:w-full max-[680px]:flex-wrap max-[680px]:justify-end">
                <button
                  type="button"
                  className={buttonSecondary}
                  onClick={() => onLaunch(shortcut)}
                  disabled={shortcut.status !== "ready"}
                >
                  <Play size={14} aria-hidden="true" />
                  Launch
                </button>
                <button
                  type="button"
                  className={iconButton}
                  onClick={() => onRefreshIcon(shortcut)}
                  disabled={refreshingIcon}
                  title="Refresh site favicon"
                  aria-label={`Refresh site favicon for ${shortcut.name}`}
                >
                  <RefreshCw size={15} aria-hidden="true" />
                </button>
                <button
                  type="button"
                  className={iconButton}
                  onClick={() => onOpenLocation(shortcut)}
                  title="Open shortcut location"
                  aria-label={`Open ${shortcut.name} location`}
                >
                  <FolderOpen size={16} aria-hidden="true" />
                </button>
                <button
                  type="button"
                  className={iconButton}
                  onClick={() => onEdit(shortcut)}
                  title="Edit shortcut"
                  aria-label={`Edit ${shortcut.name}`}
                >
                  <Pencil size={15} aria-hidden="true" />
                </button>
                <button
                  type="button"
                  className={iconButton}
                  onClick={() => onDuplicate(shortcut)}
                  title="Duplicate shortcut"
                  aria-label={`Duplicate ${shortcut.name}`}
                >
                  <Copy size={15} aria-hidden="true" />
                </button>
                <button
                  type="button"
                  className={`${iconButton} ${dangerHover}`}
                  onClick={() => onDelete(shortcut)}
                  title="Remove shortcut"
                  aria-label={`Remove ${shortcut.name}`}
                >
                  <Trash2 size={15} aria-hidden="true" />
                </button>
              </div>
            </article>
          ))}
        </section>
      ) : (
        <section className={`${panelCard} grid justify-items-start gap-2`}>
          <div className="flex items-center gap-2">
            <span className={emptyIcon}>
              <Link2 size={16} aria-hidden="true" />
            </span>
            <h2 className="text-sm leading-5 font-medium">
              {shortcuts.length ? "No shortcuts match your search" : "No shortcuts yet"}
            </h2>
          </div>
          <p className="max-w-[54ch] text-muted-foreground">
            {shortcuts.length
              ? "Try another search."
              : "Create a shortcut to open a website in a Firefox container or directly in your profile."}
          </p>
          {shortcuts.length === 0 && (
            <button
              type="button"
              className={buttonPrimary}
              onClick={onCreate}
            >
              <Plus size={16} aria-hidden="true" />
              Create your first shortcut
            </button>
          )}
        </section>
      )}
    </div>
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
    <img className={`${shortcutIcon} ${sizeClass}`} src={shortcut.iconPreviewDataUrl} alt="" />
  ) : (
    <span
      className={`${shortcutIcon} ${sizeClass} grid place-items-center text-sm font-bold text-primary`}
    >
      {shortcut.websiteName.slice(0, 1).toUpperCase()}
    </span>
  );
}
