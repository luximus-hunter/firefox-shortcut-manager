import {
  ArrowUpRight,
  CircleAlert,
  ExternalLink,
  Puzzle,
} from "lucide-react";
import { useEffect, useState } from "react";
import { StatusBadge } from "../../components/common/StatusBadge";
import {
  accessPanel,
  buttonSecondary,
  emptyMessage,
  field,
  fieldControl,
  pageStack,
  panelCard,
  sectionHeading,
  sectionKicker,
  sectionTitle,
} from "../../lib/ui";
import type { AppSettings, ScanResult } from "../../types";

const extensionIcons: Record<string, string> = {
  "{f069aec0-43c5-4bbf-b6b4-df95c4326b98}": "/open-links-in-a-container.png",
  "{572d2b62-7dda-4e5e-b71f-e6480f4bd5cd}": "/container-windows-re.png",
  "persistent-containers-mv3@navchandar.github.io":
    "/persistent-containers.png",
};

interface ExtensionsPageProps {
  scan?: ScanResult;
  settings: AppSettings;
  onOpenAddonsPage: (url: string) => void;
  onOpenExtension: (
    installationId: string,
    profileId: string,
    url: string,
  ) => void;
}

export function ExtensionsPage({
  scan,
  settings,
  onOpenAddonsPage,
  onOpenExtension,
}: ExtensionsPageProps) {
  const profiles = scan?.profiles ?? [];
  const [profileId, setProfileId] = useState("");
  const profile =
    profiles.find((candidate) => candidate.id === profileId) ??
    profiles.find(
      (candidate) =>
        candidate.id === settings.defaultProfileId || candidate.isDefault,
    ) ??
    profiles[0];
  const installationId =
    settings.defaultInstallationId ??
    profile?.installationId ??
    scan?.installations[0]?.id ??
    "";

  useEffect(() => {
    if (profile && profile.id !== profileId) setProfileId(profile.id);
  }, [profile, profileId]);

  return (
    <div className={pageStack}>
      <section className={panelCard}>
        <div className={`${sectionHeading} items-end max-[680px]:items-start max-[680px]:flex-col`}>
          <div>
            <span className={sectionKicker}>Profile check</span>
            <h2 className={sectionTitle}>
              Extension status
            </h2>
          </div>
          <label className={`${field} w-[min(240px,42%)] max-[680px]:w-full`}>
            <span className="mb-1 block text-xs text-muted-foreground">
              Firefox profile
            </span>
            <select
              className={`${fieldControl} max-w-none`}
              value={profile?.id ?? ""}
              onChange={(event) => setProfileId(event.currentTarget.value)}
              disabled={!profiles.length}
            >
              {profiles.length ? (
                profiles.map((candidate) => (
                  <option value={candidate.id} key={candidate.id}>
                    {candidate.name}
                  </option>
                ))
              ) : (
                <option value="">No profiles found</option>
              )}
            </select>
          </label>
        </div>

        {profile ? (
          <div className="grid gap-2">
            {profile.extensions.map((extension) => (
              <article
                className="flex min-w-0 items-start gap-2 border border-border bg-muted/40 p-4 max-[680px]:flex-wrap"
                key={extension.id}
              >
                <div
                  className="grid size-10 shrink-0 place-items-center bg-primary/10 text-highlight"
                  aria-hidden="true"
                >
                  {extensionIcons[extension.id] ? (
                    <img
                      className="size-full object-contain"
                      src={extensionIcons[extension.id]}
                      alt=""
                    />
                  ) : (
                    <Puzzle size={19} />
                  )}
                </div>
                <div className="flex min-w-0 flex-1 flex-col gap-1">
                  <div className="flex items-center gap-2">
                    <strong className="text-sm">{extension.name}</strong>
                    {extension.required ? (
                      <span className="inline-flex min-h-6 items-center gap-1 bg-danger-soft px-2 text-xs font-semibold text-danger">
                        Required
                      </span>
                    ) : (
                      <span className="inline-flex min-h-6 items-center gap-1 bg-muted px-2 text-xs font-semibold text-muted-foreground">
                        Recommended
                      </span>
                    )}
                  </div>
                  <p className="m-0 text-[13px] text-muted-foreground">
                    {extension.purpose}
                  </p>
                  <div className="text-xs text-muted-foreground">
                    {extension.version
                      ? `Version ${extension.version}`
                      : "Version unavailable"}
                    {extension.status === "detectionFailed" &&
                      " · Close Firefox and rescan for a fresh status"}
                  </div>
                  <a
                    className="inline-flex w-fit items-center gap-1 text-xs font-semibold text-highlight no-underline hover:underline"
                    href={extension.downloadUrl}
                    onClick={(event) => {
                      event.preventDefault();
                      onOpenAddonsPage(extension.downloadUrl);
                    }}
                  >
                    View on Mozilla Add-ons
                    <ExternalLink size={14} aria-hidden="true" />
                  </a>
                </div>
                <div className="grid justify-items-end gap-2 max-[680px]:grid-cols-[1fr_auto] max-[680px]:items-center max-[680px]:justify-items-start max-[680px]:w-full">
                  <span className="max-[680px]:col-start-1 max-[680px]:row-start-1">
                    <StatusBadge status={extension.status} />
                  </span>
                  {extension.status !== "installedEnabled" && (
                    <button
                      type="button"
                      className={`${buttonSecondary} max-[680px]:col-start-2 max-[680px]:row-start-1`}
                      disabled={!installationId}
                      onClick={() =>
                        onOpenExtension(
                          installationId,
                          profile.id,
                          extension.downloadUrl,
                        )
                      }
                    >
                      Open in Firefox
                      <ArrowUpRight size={14} aria-hidden="true" />
                    </button>
                  )}
                </div>
              </article>
            ))}
            {profile.extensions.length === 0 && (
              <div className={emptyMessage}>
                <CircleAlert size={20} aria-hidden="true" />
                <strong className="mt-2 text-sm text-foreground">
                  No extension metadata found
                </strong>
                <p className="mt-1 max-w-[440px] text-[13px]">
                  Rescan after Firefox has started at least once.
                </p>
              </div>
            )}
          </div>
        ) : (
          <div className={emptyMessage}>
            <CircleAlert size={20} aria-hidden="true" />
            <strong className="mt-2 text-sm text-foreground">
              No Firefox profile selected
            </strong>
            <p className="mt-1 max-w-[440px] text-[13px]">
              Scan Firefox to discover profiles and their extensions.
            </p>
          </div>
        )}
      </section>

      <p className={`${accessPanel} flex items-start gap-2 px-2 py-2 text-xs leading-relaxed text-muted-foreground`}>
        Extension state comes from Firefox profile metadata. When Firefox is
        open, that file may be stale; the app never edits it or reads browser
        passwords and cookies.
      </p>
    </div>
  );
}
