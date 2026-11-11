import { AlertCircle, ArrowRight, Globe2, X } from "lucide-react";
import { FormEvent, useEffect, useState } from "react";
import { shortcutFormSchema, suggestedShortcutName, websiteNameFromUrl } from "../../lib/validation";
import {
  accessPanel,
  buttonGhost,
  buttonPrimary,
  field,
  fieldControl,
  fieldError,
  fieldHelp,
  fieldLabel,
  iconButton,
  sectionKicker,
} from "../../lib/ui";
import type {
  AppSettings,
  CreateShortcutRequest,
  FirefoxProfile,
  ScanResult,
  ShortcutView,
} from "../../types";

const noInstallations: ScanResult["installations"] = [];
const noProfiles: ScanResult["profiles"] = [];
const NO_CONTAINER_OPTION = "__no_container__";

export type ShortcutDialogMode = "create" | "duplicate" | "edit";

interface ShortcutDialogProps {
  open: boolean;
  mode: ShortcutDialogMode;
  scan?: ScanResult;
  settings: AppSettings;
  initial?: ShortcutView;
  pending: boolean;
  error?: string;
  onClose: () => void;
  onSave: (request: CreateShortcutRequest) => void;
}

export function ShortcutDialog({
  open,
  mode,
  scan,
  settings,
  initial,
  pending,
  error,
  onClose,
  onSave,
}: ShortcutDialogProps) {
  const installations = scan?.installations ?? noInstallations;
  const profiles = scan?.profiles ?? noProfiles;
  const [targetUrl, setTargetUrl] = useState("");
  const [websiteName, setWebsiteName] = useState("");
  const [shortcutName, setShortcutName] = useState("");
  const [installationId, setInstallationId] = useState("");
  const [profileId, setProfileId] = useState("");
  const [containerId, setContainerId] = useState(NO_CONTAINER_OPTION);
  const [openAsApp, setOpenAsApp] = useState(false);
  const [removeIconBackground, setRemoveIconBackground] = useState(false);
  const [iconBackgroundColor, setIconBackgroundColor] = useState("#ffffff");
  const [websiteNameTouched, setWebsiteNameTouched] = useState(false);
  const [shortcutNameTouched, setShortcutNameTouched] = useState(false);
  const [fieldErrors, setFieldErrors] = useState<Record<string, string>>({});

  const selectedProfile = profiles.find((profile) => profile.id === profileId);
  const selectedContainer = selectedProfile?.containers.find(
    (container) => container.id === containerId,
  );
  const usableProfiles = profiles.filter((profile) => !profile.error);
  const defaultProfile =
    profiles.find((profile) => profile.id === settings.defaultProfileId) ??
    profiles.find((profile) => profile.isDefault) ??
    profiles[0];
  const defaultInstallation =
    installations.find(
      (installation) => installation.id === settings.defaultInstallationId,
    ) ?? installations[0];
  const formDisabled =
    pending ||
    !installations.length ||
    !usableProfiles.length ||
    !selectedProfile ||
    Boolean(selectedProfile.error);

  useEffect(() => {
    if (!open) return;
    const profile =
      profiles.find((candidate) => candidate.id === initial?.profileId) ??
      defaultProfile;
    const installation =
      installations.find(
        (candidate) => candidate.id === initial?.installationId,
      ) ?? defaultInstallation;
    const nextWebsiteName =
      initial?.websiteName ?? (initial ? "" : websiteNameFromUrl(""));
    const nextContainer =
      initial?.containerId === null
        ? undefined
        : profile?.containers.find(
            (container) => container.id === initial?.containerId,
          ) ?? profile?.containers[0];
    setTargetUrl(initial?.targetUrl ?? "");
    setWebsiteName(nextWebsiteName);
    setShortcutName(initial?.name ?? "");
    setInstallationId(installation?.id ?? "");
    setProfileId(profile?.id ?? "");
    setContainerId(nextContainer?.id ?? NO_CONTAINER_OPTION);
    setOpenAsApp(initial?.openAsApp ?? false);
    setRemoveIconBackground(Boolean(initial?.iconBackgroundColor));
    setIconBackgroundColor(initial?.iconBackgroundColor ?? "#ffffff");
    setWebsiteNameTouched(Boolean(initial?.websiteName));
    setShortcutNameTouched(Boolean(initial?.name));
    setFieldErrors({});
  }, [
    defaultInstallation,
    defaultProfile,
    initial,
    installations,
    mode,
    open,
    profiles,
  ]);

  if (!open) return null;

  function updateWebsiteUrl(value: string) {
    setTargetUrl(value);
    const inferredName = websiteNameFromUrl(value);
    if (!websiteNameTouched) {
      setWebsiteName(inferredName);
      if (!shortcutNameTouched) {
        setShortcutName(
          suggestedShortcutName(inferredName, selectedContainer?.name ?? ""),
        );
      }
    }
    setFieldErrors((current) => ({ ...current, targetUrl: "" }));
  }

  function updateWebsiteName(value: string) {
    setWebsiteName(value);
    setWebsiteNameTouched(true);
    if (!shortcutNameTouched) {
      setShortcutName(
        suggestedShortcutName(value, selectedContainer?.name ?? ""),
      );
    }
    setFieldErrors((current) => ({ ...current, websiteName: "" }));
  }

  function updateProfile(profile: FirefoxProfile) {
    setProfileId(profile.id);
    const nextContainer = profile.containers[0];
    setContainerId(nextContainer?.id ?? NO_CONTAINER_OPTION);
    if (!shortcutNameTouched) {
      setShortcutName(
        suggestedShortcutName(websiteName, nextContainer?.name ?? ""),
      );
    }
    setFieldErrors((current) => ({ ...current, profileId: "", containerId: "" }));
  }

  function updateContainer(value: string) {
    setContainerId(value);
    const container = selectedProfile?.containers.find(
      (candidate) => candidate.id === value,
    );
    if (!shortcutNameTouched) {
      setShortcutName(suggestedShortcutName(websiteName, container?.name ?? ""));
    }
    setFieldErrors((current) => ({ ...current, containerId: "" }));
  }

  function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const result = shortcutFormSchema.safeParse({
      websiteName,
      targetUrl,
      installationId,
      profileId,
      containerId:
        containerId === NO_CONTAINER_OPTION ? null : containerId,
      openAsApp,
      shortcutName,
      iconBackgroundColor: removeIconBackground ? iconBackgroundColor : null,
    });
    if (!result.success) {
      const nextErrors: Record<string, string> = {};
      for (const issue of result.error.issues) {
        const key = String(issue.path[0] ?? "form");
        nextErrors[key] ??= issue.message;
      }
      setFieldErrors(nextErrors);
      return;
    }
    onSave(result.data);
  }

  return (
    <div
      className="fixed inset-0 z-20 grid place-items-center overflow-y-auto bg-black/45 px-4 py-8 max-[680px]:place-items-start max-[680px]:p-2"
      role="presentation"
      onMouseDown={(event) => {
        if (event.currentTarget === event.target && !pending) onClose();
      }}
    >
      <section
        className="max-h-[min(880px,calc(100vh-64px))] w-[min(43rem,100%)] overflow-auto bg-card ring-1 ring-foreground/10 max-[680px]:max-h-[calc(100vh-16px)]"
        role="dialog"
        aria-modal="true"
        aria-labelledby="shortcut-dialog-title"
      >
        <header className="sticky top-0 z-[2] flex items-center gap-2 border-b border-border bg-card p-4">
          <Globe2 className="size-5 shrink-0 text-highlight" aria-hidden="true" />
          <div>
            <span className={sectionKicker}>
              {mode === "edit"
                ? "Edit Start menu shortcut"
                : mode === "duplicate"
                  ? "Duplicate shortcut"
                  : "New Start menu shortcut"}
            </span>
            <h2 className="mt-1 text-xl leading-tight font-semibold tracking-[-0.045em]" id="shortcut-dialog-title">
              {mode === "edit"
                ? "Edit shortcut"
                : mode === "duplicate"
                  ? "Create a copy"
                  : "Create a shortcut"}
            </h2>
          </div>
          <button
            type="button"
            className={`${iconButton} ml-auto`}
            onClick={onClose}
            disabled={pending}
            aria-label="Close dialog"
          >
            <X size={18} aria-hidden="true" />
          </button>
        </header>

        <form className="p-4" onSubmit={submit} noValidate>
          <label className={field}>
            <span className={fieldLabel}>Website URL</span>
            <input
              className={fieldControl}
              type="url"
              value={targetUrl}
              onChange={(event) => updateWebsiteUrl(event.currentTarget.value)}
              placeholder="https://example.com"
              autoFocus
              aria-invalid={Boolean(fieldErrors.targetUrl)}
              aria-describedby={fieldErrors.targetUrl ? "target-url-error" : undefined}
            />
            {fieldErrors.targetUrl ? (
              <small className={fieldError} id="target-url-error">
                {fieldErrors.targetUrl}
              </small>
            ) : (
              <small className={fieldHelp}>
                Use a website address beginning with https:// or http://.
              </small>
            )}
          </label>

          <label className={field}>
            <span className={fieldLabel}>Display name</span>
            <input
              className={fieldControl}
              value={websiteName}
              onChange={(event) => updateWebsiteName(event.currentTarget.value)}
              placeholder="Website name"
              maxLength={120}
              aria-invalid={Boolean(fieldErrors.websiteName)}
            />
            {fieldErrors.websiteName ? (
              <small className={fieldError}>{fieldErrors.websiteName}</small>
            ) : (
              <small className={fieldHelp}>
                Suggested from the website address; you can change it.
              </small>
            )}
          </label>

          <div className="grid grid-cols-2 gap-4 max-[680px]:grid-cols-1 max-[680px]:gap-0">
            <label className={field}>
              <span className={fieldLabel}>Firefox installation</span>
              <select
                className={fieldControl}
                value={installationId}
                onChange={(event) => setInstallationId(event.currentTarget.value)}
                disabled={!installations.length}
                aria-invalid={Boolean(fieldErrors.installationId)}
              >
                {installations.length ? (
                  installations.map((installation) => (
                    <option value={installation.id} key={installation.id}>
                      {installation.name}
                    </option>
                  ))
                ) : (
                  <option value="">No installation found</option>
                )}
              </select>
              {fieldErrors.installationId && (
                <small className={fieldError}>{fieldErrors.installationId}</small>
              )}
            </label>

            <label className={field}>
              <span className={fieldLabel}>Firefox profile</span>
              <select
                className={fieldControl}
                value={profileId}
                onChange={(event) => {
                  const profile = profiles.find(
                    (candidate) => candidate.id === event.currentTarget.value,
                  );
                  if (profile) updateProfile(profile);
                }}
                disabled={!usableProfiles.length}
                aria-invalid={Boolean(fieldErrors.profileId)}
              >
                {usableProfiles.length ? (
                  usableProfiles.map((profile) => (
                    <option value={profile.id} key={profile.id}>
                      {profile.name}
                      {profile.isDefault ? " · Firefox default" : ""}
                    </option>
                  ))
                ) : (
                  <option value="">No available profile</option>
                )}
              </select>
              {fieldErrors.profileId && (
                <small className={fieldError}>{fieldErrors.profileId}</small>
              )}
            </label>
          </div>

          <label className={field}>
            <span className={fieldLabel}>Open in</span>
            <select
              className={fieldControl}
              value={containerId}
              onChange={(event) => updateContainer(event.currentTarget.value)}
              disabled={!selectedProfile}
              aria-invalid={Boolean(fieldErrors.containerId)}
            >
              <option value={NO_CONTAINER_OPTION}>
                No container · regular browsing
              </option>
              {selectedProfile?.containers.length ? (
                <optgroup label={selectedProfile.name}>
                  {selectedProfile.containers.map((container) => (
                    <option value={container.id} key={container.id}>
                      {container.name}
                    </option>
                  ))}
                </optgroup>
              ) : null}
            </select>
            {fieldErrors.containerId ? (
              <small className={fieldError}>{fieldErrors.containerId}</small>
            ) : (
              <small className={fieldHelp}>
                {containerId === NO_CONTAINER_OPTION
                  ? openAsApp
                    ? "Opens in the selected profile's app window without a container."
                    : "Opens directly in the selected Firefox profile."
                  : openAsApp
                    ? "Firefox opens this existing container directly in the app window."
                    : "Only existing containers are listed. Firefox profile files are never changed."}
              </small>
            )}
          </label>

          <label className="mb-4 flex cursor-pointer items-center justify-between gap-4 border-t border-border pt-4">
            <span className="flex min-w-0 flex-col gap-1">
              <strong className="text-[13px]">
                Open in a separate app window
              </strong>
              <small className="text-xs text-muted-foreground">
                Registers a Firefox Taskbar Tab in this profile. Taskbar Tabs
                must be enabled, and Firefox must be closed if its registration
                needs to be added or updated. The selected container is passed
                directly—no extension is required.
              </small>
            </span>
            <input
              className="check-control"
              type="checkbox"
              checked={openAsApp}
              onChange={(event) => setOpenAsApp(event.currentTarget.checked)}
            />
          </label>

          <div className="grid grid-cols-2 gap-4 max-[680px]:grid-cols-1 max-[680px]:gap-0">
            <label className={field}>
              <span className={fieldLabel}>Shortcut name</span>
              <input
                className={fieldControl}
                value={shortcutName}
                onChange={(event) => {
                  setShortcutName(event.currentTarget.value);
                  setShortcutNameTouched(true);
                  setFieldErrors((current) => ({ ...current, shortcutName: "" }));
                }}
                placeholder="Website (Container)"
                maxLength={120}
                aria-invalid={Boolean(fieldErrors.shortcutName)}
              />
              {fieldErrors.shortcutName && (
                <small className={fieldError}>{fieldErrors.shortcutName}</small>
              )}
            </label>
            <div className={field}>
              <span className={fieldLabel}>Start menu folder</span>
              <input className={fieldControl} value="Firefox Shortcuts" readOnly />
              <small className={fieldHelp}>
                Saved under the current user&apos;s Start Menu\Programs folder.
              </small>
            </div>
          </div>

          <fieldset className="mb-4 border border-border p-4">
            <legend className="px-2 text-xs font-medium">Shortcut icon</legend>
            <label className="flex cursor-pointer items-center gap-2">
              <input
                className="check-control"
                type="checkbox"
                checked={removeIconBackground}
                onChange={(event) => setRemoveIconBackground(event.currentTarget.checked)}
              />
              <span className={fieldLabel}>Remove background color</span>
            </label>
            {removeIconBackground && (
              <label className="mt-2 flex items-center gap-2">
                <span className={fieldLabel}>Background color</span>
                <input
                  className="size-8 cursor-pointer border border-input bg-background p-1"
                  type="color"
                  value={iconBackgroundColor}
                  onChange={(event) => setIconBackgroundColor(event.currentTarget.value)}
                />
              </label>
            )}
            <p className={`mt-2 ${fieldHelp}`}>
              Removes matching color connected to the icon’s outer edge.
            </p>
          </fieldset>

          {error ? (
            <div
              className="mb-4 flex items-start gap-2 border border-danger/20 bg-danger-soft p-2 text-[13px] text-danger"
              role="alert"
            >
              <AlertCircle className="shrink-0" size={16} aria-hidden="true" />
              <span>{error}</span>
            </div>
          ) : null}

          <div className={`${accessPanel} flex items-start gap-2 px-2 py-2 text-xs leading-relaxed text-muted-foreground`}>
            <ArrowRight className="mt-0.5 shrink-0 text-highlight" size={15} aria-hidden="true" />
            <span>
              {openAsApp
                ? "Firefox opens this website in its registered app window using the selected profile and container."
                : "Firefox is launched with the selected profile in an isolated instance. If that profile is already open, Firefox may ask you to close it first rather than opening the wrong profile."}
            </span>
          </div>

          <footer className="-mx-4 mt-4 flex justify-end gap-2 border-t border-border px-4 pt-4">
            <button
              type="button"
              className={buttonGhost}
              onClick={onClose}
              disabled={pending}
            >
              Cancel
            </button>
            <button
              type="submit"
              className={buttonPrimary}
              disabled={formDisabled}
            >
              {pending
                ? mode === "edit"
                  ? "Saving…"
                  : "Creating…"
                : mode === "edit"
                  ? "Save changes"
                  : "Create shortcut"}
              {!pending && <ArrowRight size={16} aria-hidden="true" />}
            </button>
          </footer>
        </form>
      </section>
    </div>
  );
}
