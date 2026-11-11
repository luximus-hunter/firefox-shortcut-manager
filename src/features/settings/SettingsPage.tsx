import { Plus, Save, Trash2 } from "lucide-react";
import { FormEvent, useEffect, useState } from "react";
import {
  buttonPrimary,
  buttonSecondary,
  dangerHover,
  field,
  fieldControl,
  fieldLabel,
  iconButton,
  mutedCopy,
  pageStack,
  panelCard,
  sectionHeading,
  sectionKicker,
  sectionTitle,
} from "../../lib/ui";
import type { AppSettings, ScanResult } from "../../types";

interface SettingsPageProps {
  settings: AppSettings;
  scan?: ScanResult;
  saving: boolean;
  onSave: (settings: AppSettings) => void;
}

export function SettingsPage({
  settings,
  scan,
  saving,
  onSave,
}: SettingsPageProps) {
  const [draft, setDraft] = useState(settings);
  const [customPath, setCustomPath] = useState("");

  useEffect(() => {
    setDraft(settings);
  }, [settings]);

  const profiles = scan?.profiles ?? [];
  const installations = scan?.installations ?? [];

  function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    onSave(draft);
  }

  function addCustomPath() {
    const path = customPath.trim();
    if (!path || draft.customFirefoxPaths.includes(path)) return;
    setDraft({
      ...draft,
      customFirefoxPaths: [...draft.customFirefoxPaths, path],
    });
    setCustomPath("");
  }

  function removeCustomPath(path: string) {
    setDraft({
      ...draft,
      customFirefoxPaths: draft.customFirefoxPaths.filter(
        (candidate) => candidate !== path,
      ),
    });
  }

  return (
    <form className={pageStack} onSubmit={submit}>
      <section className={panelCard}>
        <div className={sectionHeading}>
          <div>
            <span className={sectionKicker}>Appearance</span>
            <h2 className={sectionTitle}>
              Theme
            </h2>
          </div>
        </div>
        <div className="grid grid-cols-2 gap-4 max-[680px]:grid-cols-1 max-[680px]:gap-0">
          <label className={field}>
            <span className={fieldLabel}>Color theme</span>
            <select
              className={fieldControl}
              value={draft.theme}
              onChange={(event) =>
                setDraft({
                  ...draft,
                  theme: event.currentTarget.value as AppSettings["theme"],
                })
              }
            >
              <option value="system">Follow Windows</option>
              <option value="light">Light</option>
              <option value="dark">Dark</option>
            </select>
          </label>
          <div className={`${field} justify-center`}>
            <span className={fieldLabel}>Current behavior</span>
            <p className="m-0 text-[13px] text-muted-foreground">
              System theme is selected by default.
            </p>
          </div>
        </div>
      </section>

      <section className={panelCard}>
        <div className={sectionHeading}>
          <div>
            <span className={sectionKicker}>Shortcut defaults</span>
            <h2 className={sectionTitle}>
              Firefox and Start menu
            </h2>
          </div>
        </div>
        <div className="grid grid-cols-2 gap-4 max-[680px]:grid-cols-1 max-[680px]:gap-0">
          <label className={field}>
            <span className={fieldLabel}>Default Firefox installation</span>
            <select
              className={fieldControl}
              value={draft.defaultInstallationId ?? ""}
              onChange={(event) =>
                setDraft({
                  ...draft,
                  defaultInstallationId: event.currentTarget.value || null,
                })
              }
            >
              <option value="">Choose automatically</option>
              {installations.map((installation) => (
                <option value={installation.id} key={installation.id}>
                  {installation.name}
                  {installation.version ? ` · ${installation.version}` : ""}
                </option>
              ))}
            </select>
          </label>
          <label className={field}>
            <span className={fieldLabel}>Default Firefox profile</span>
            <select
              className={fieldControl}
              value={draft.defaultProfileId ?? ""}
              onChange={(event) =>
                setDraft({
                  ...draft,
                  defaultProfileId: event.currentTarget.value || null,
                })
              }
            >
              <option value="">Use Firefox's detected default</option>
              {profiles.map((profile) => (
                <option value={profile.id} key={profile.id}>
                  {profile.name}
                </option>
              ))}
            </select>
          </label>
        </div>
        <div className={`${field} justify-center`}>
          <span className={fieldLabel}>Start menu folder</span>
          <p className="m-0 text-[13px] text-muted-foreground">
            The current user&apos;s Programs\Firefox Shortcuts folder.
          </p>
        </div>
        <label className="flex cursor-pointer items-center justify-between gap-4 border-t border-border pt-4">
          <span className="flex min-w-0 flex-col gap-1">
            <strong className="text-[13px]">Rescan Firefox on startup</strong>
            <small className="text-xs text-muted-foreground">
              Refresh installations, profiles, containers, and add-ons.
            </small>
          </span>
          <input
            className="check-control"
            type="checkbox"
            checked={draft.autoScan}
            onChange={(event) =>
              setDraft({ ...draft, autoScan: event.currentTarget.checked })
            }
          />
        </label>
      </section>

      <section className={panelCard}>
        <div className={sectionHeading}>
          <div>
            <span className={sectionKicker}>Manual discovery</span>
            <h2 className={sectionTitle}>
              Custom Firefox paths
            </h2>
          </div>
        </div>
        <p className="mb-4 mt-0 text-[13px] text-muted-foreground">
          Add a Firefox executable path for portable or non-standard
          installations. The app only reads the executable and profile
          metadata.
        </p>
        <div className="flex gap-2">
          <input
            className={fieldControl}
            value={customPath}
            onChange={(event) => setCustomPath(event.currentTarget.value)}
            placeholder="C:\Firefox\firefox.exe"
            aria-label="Custom Firefox executable path"
          />
          <button
            type="button"
            className={buttonSecondary}
            onClick={addCustomPath}
            disabled={!customPath.trim()}
          >
            <Plus size={16} aria-hidden="true" />
            Add path
          </button>
        </div>
        {draft.customFirefoxPaths.length ? (
          <ul className="mt-4 grid list-none gap-2 p-0">
            {draft.customFirefoxPaths.map((path) => (
              <li
                className="flex items-center justify-between gap-2 bg-muted/55 px-2 py-1"
                key={path}
              >
                <code className="flex-1 overflow-hidden text-xs text-muted-foreground text-ellipsis whitespace-nowrap">
                  {path}
                </code>
                <button
                  type="button"
                  className={`${iconButton} ${dangerHover}`}
                  onClick={() => removeCustomPath(path)}
                  aria-label={`Remove ${path}`}
                >
                  <Trash2 size={15} aria-hidden="true" />
                </button>
              </li>
            ))}
          </ul>
        ) : (
          <p className={mutedCopy}>No custom paths configured.</p>
        )}
      </section>

      <div className="flex items-center justify-between gap-4 max-[680px]:items-start max-[680px]:flex-col">
        <p className="m-0 text-xs text-muted-foreground">
          Settings are stored locally in your Windows user profile.
        </p>
        <button type="submit" className={buttonPrimary} disabled={saving}>
          <Save size={16} aria-hidden="true" />
          {saving ? "Saving…" : "Save settings"}
        </button>
      </div>
    </form>
  );
}
