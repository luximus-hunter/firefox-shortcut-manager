import { AppChromeControl } from "./AppChromeControl";
import { Boxes, Check, CircleAlert, HardDrive, Star } from "lucide-react";
import { StatusBadge } from "../../components/common/StatusBadge";
import {
  badge,
  emptyIcon,
  emptyMessage,
  mutedCopy,
  pageStack,
  panelCard,
  sectionHeading,
  sectionKicker,
  sectionTitle,
} from "../../lib/ui";
import type { AppSettings, ScanResult } from "../../types";

interface FirefoxPageProps {
  scan?: ScanResult;
  settings: AppSettings;
  onDefaultProfile: (profileId: string) => void;
}

const containerColorClasses: Record<string, string> = {
  blue: "bg-[#5b8def]",
  turquoise: "bg-[#38b6b0]",
  green: "bg-[#56a76c]",
  yellow: "bg-[#d5a530]",
  orange: "bg-[#e78545]",
  red: "bg-[#db6464]",
  pink: "bg-[#d875a7]",
  purple: "bg-[#8b70d1]",
};

export function FirefoxPage({
  scan,
  settings,
  onDefaultProfile,
}: FirefoxPageProps) {
  const installations = scan?.installations ?? [];
  const profiles = scan?.profiles ?? [];

  return (
    <div className={pageStack}>
      <section className={panelCard}>
        <div className={sectionHeading}>
          <div>
            <span className={sectionKicker}>Browser installations</span>
            <h2 className={sectionTitle}>
              Firefox on this PC
            </h2>
          </div>
          <span className={badge}>
            {installations.length} found
          </span>
        </div>
        {installations.length ? (
          <div className="grid grid-cols-2 gap-4 max-[680px]:grid-cols-1 max-[680px]:gap-0">
            {installations.map((installation) => (
              <article
                className="flex min-w-0 items-start gap-2 border border-border bg-muted/45 p-4"
                key={installation.id}
              >
                <span className="grid size-9 shrink-0 place-items-center bg-primary/10 text-highlight">
                  <HardDrive size={19} aria-hidden="true" />
                </span>
                <div className="flex min-w-0 flex-1 flex-col gap-1">
                  <strong className="text-sm">{installation.name}</strong>
                  <span className="text-xs text-muted-foreground">
                    {installation.version
                      ? `Version ${installation.version}`
                      : "Version unavailable"}
                  </span>
                  <code
                    className="overflow-hidden text-xs text-muted-foreground text-ellipsis whitespace-nowrap"
                    title={installation.executable}
                  >
                    {installation.executable}
                  </code>
                </div>
                {settings.defaultInstallationId === installation.id ? (
                  <span className={`${badge} text-highlight`}>
                    Default
                  </span>
                ) : null}
              </article>
            ))}
          </div>
        ) : (
          <EmptyMessage
            title="No Firefox installation detected"
            description="Add a custom Firefox executable in Settings, then rescan."
          />
        )}
      </section>

      <section className={panelCard}>
        <div className={sectionHeading}>
          <div>
            <span className={sectionKicker}>Profiles and containers</span>
            <h2 className={sectionTitle}>
              Firefox profiles
            </h2>
          </div>
          <span className={badge}>
            {profiles.length} found
          </span>
        </div>
        {profiles.length ? (
          <div className="grid gap-4">
            {profiles.map((profile) => {
              const isDefault =
                settings.defaultProfileId === profile.id ||
                (!settings.defaultProfileId && profile.isDefault);
              return (
                <article
                  className="rounded-md border border-border bg-muted/35 p-4"
                  key={profile.id}
                >
                  <div className="flex items-center gap-2">
                    <div className="grid size-9 shrink-0 place-items-center bg-primary/10 text-sm font-semibold text-highlight">
                      {profile.name.slice(0, 1).toUpperCase()}
                    </div>
                    <div className="flex min-w-0 flex-1 flex-col">
                      <div className="flex items-center gap-2">
                        <strong className="text-sm">{profile.name}</strong>
                        {isDefault ? (
                          <span className={`${badge} text-highlight`}>
                            <Star size={12} fill="currentColor" />
                            Default
                          </span>
                        ) : (
                          <button
                            type="button"
                            className="inline-flex cursor-pointer items-center gap-1 p-0 text-[13px] font-semibold text-highlight hover:underline"
                            onClick={() => onDefaultProfile(profile.id)}
                          >
                            Set as default
                          </button>
                        )}
                      </div>
                      <code
                        className="overflow-hidden text-xs text-muted-foreground text-ellipsis whitespace-nowrap"
                        title={profile.path}
                      >
                        {profile.path}
                      </code>
                    </div>
                    {profile.error ? (
                      <span className="text-danger" title={profile.error}>
                        <CircleAlert size={17} aria-hidden="true" />
                      </span>
                    ) : null}
                  </div>
                  {profile.error ? (
                    <div className="mt-2 flex items-start gap-2 text-xs text-danger">
                      <CircleAlert className="shrink-0" size={14} aria-hidden="true" />
                      {profile.error}
                    </div>
                  ) : null}
                  <AppChromeControl profileId={profile.id} unavailable={!!profile.error} />
                  <div className="mt-4 border-t border-border pt-4">
                    <div className="flex items-center gap-2 text-xs font-semibold text-muted-foreground">
                      <Boxes size={14} aria-hidden="true" />
                      <span>Containers</span>
                      <span className="inline-grid size-6 min-w-6 place-items-center bg-muted text-muted-foreground">
                        {profile.containers.length}
                      </span>
                    </div>
                    {profile.containers.length ? (
                      <div className="mt-2 flex flex-wrap gap-2">
                        {profile.containers.map((container) => (
                          <span
                            className="inline-flex min-h-8 items-center gap-2 border border-border bg-card px-2 text-xs text-foreground"
                            key={container.id}
                          >
                            <span
                              className={`size-2 shrink-0 rounded-full ${containerColorClasses[container.color] ?? "bg-[#8b95a5]"}`}
                            />
                            {container.name}
                          </span>
                        ))}
                      </div>
                    ) : (
                      <p className={mutedCopy}>
                        {profile.containerState === "failed"
                          ? "Container data could not be read. Close Firefox and rescan."
                          : "No containers were found in this profile."}
                      </p>
                    )}
                  </div>
                  <div className="mt-4 flex items-center gap-2 text-xs text-muted-foreground max-[680px]:flex-wrap">
                    <Check className="text-success" size={14} aria-hidden="true" />
                    <span>
                      {profile.extensions.filter(
                        (extension) => extension.status === "installedEnabled",
                      ).length}{" "}
                      enabled extensions detected
                    </span>
                    <div className="ml-auto flex flex-wrap gap-1 max-[680px]:ml-0 max-[680px]:w-full">
                      {profile.extensions
                        .filter((extension) => extension.required)
                        .map((extension) => (
                          <StatusBadge
                            key={extension.id}
                            status={extension.status}
                          />
                        ))}
                    </div>
                  </div>
                </article>
              );
            })}
          </div>
        ) : (
          <EmptyMessage
            title="No Firefox profiles found"
            description="Profiles are read from Firefox's profiles.ini and installs.ini files."
          />
        )}
      </section>

      {scan?.diagnostics.length ? (
        <section className={panelCard}>
          <div className={sectionHeading}>
            <div>
              <span className={sectionKicker}>Scan notes</span>
              <h2 className={sectionTitle}>
                Items that need attention
              </h2>
            </div>
          </div>
          <ul className="m-0 grid list-none gap-2 p-0">
            {scan.diagnostics.map((issue, index) => (
              <li
                className="flex gap-2 text-[13px] text-muted-foreground"
                key={`${issue.source}-${index}`}
              >
                <CircleAlert className="shrink-0 text-warning" size={15} aria-hidden="true" />
                <span>
                  <strong className="block text-[13px] text-foreground">
                    {issue.source}
                  </strong>
                  {issue.message}
                </span>
              </li>
            ))}
          </ul>
        </section>
      ) : null}
    </div>
  );
}

function EmptyMessage({
  title,
  description,
}: {
  title: string;
  description: string;
}) {
  return (
    <div className={emptyMessage}>
      <span className={emptyIcon}>
        <Boxes size={22} aria-hidden="true" />
      </span>
      <strong className="mt-2 text-sm text-foreground">{title}</strong>
      <p className="mt-1 max-w-[440px] text-[13px]">{description}</p>
    </div>
  );
}
