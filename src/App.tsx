import { AlertCircle, CheckCircle2, X } from "lucide-react";
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { AppShell, type PageKey } from "./components/layout/AppShell";
import { ExtensionsPage } from "./features/extensions/ExtensionsPage";
import { FirefoxPage } from "./features/firefox/FirefoxPage";
import {
  ShortcutDialog,
  type ShortcutDialogMode,
} from "./features/shortcuts/ShortcutDialog";
import { ShortcutsPage } from "./features/shortcuts/ShortcutsPage";
import { SettingsPage } from "./features/settings/SettingsPage";
import { iconButton } from "./lib/ui";
import type {
  AppSettings,
  CreateShortcutRequest,
  NativeError,
  ScanResult,
  ShortcutView,
} from "./types";

const defaultSettings: AppSettings = {
  schemaVersion: 1,
  theme: "system",
  defaultInstallationId: null,
  defaultProfileId: null,
  autoScan: true,
  customFirefoxPaths: [],
};

type ShortcutDialogState =
  | { mode: "create" }
  | {
      mode: Exclude<ShortcutDialogMode, "create">;
      shortcut: ShortcutView;
    };

function errorText(error: unknown): { message: string; details?: string } {
  if (typeof error === "object" && error !== null) {
    const nativeError = error as NativeError;
    if (nativeError.message) {
      return {
        message: nativeError.message,
        details: nativeError.details ?? undefined,
      };
    }
  }
  if (error instanceof Error) return { message: error.message };
  return { message: "The operation could not be completed. Try again or rescan Firefox." };
}

function App() {
  const queryClient = useQueryClient();
  const [activePage, setActivePage] = useState<PageKey>("shortcuts");
  const [shortcutDialog, setShortcutDialog] = useState<ShortcutDialogState>();
  const [feedback, setFeedback] = useState<{
    kind: "success" | "error";
    message: string;
    details?: string;
  }>();

  const settingsQuery = useQuery({
    queryKey: ["settings"],
    queryFn: () => invoke<AppSettings>("get_settings"),
    retry: false,
  });
  const settings = settingsQuery.data ?? defaultSettings;
  const scanQuery = useQuery({
    queryKey: ["firefox"],
    queryFn: () => invoke<ScanResult>("scan_firefox"),
    enabled: settingsQuery.isSuccess && settings.autoScan,
    staleTime: 30_000,
    retry: false,
  });
  const shortcutsQuery = useQuery({
    queryKey: ["shortcuts"],
    queryFn: () => invoke<ShortcutView[]>("list_managed_shortcuts"),
    enabled: settingsQuery.isSuccess,
    staleTime: 15_000,
    retry: false,
  });

  useEffect(() => {
    const systemTheme = window.matchMedia("(prefers-color-scheme: dark)");
    const updateTheme = () => {
      const isDark = settings.theme === "dark" ||
        (settings.theme === "system" && systemTheme.matches);
      document.documentElement.classList.toggle("dark", isDark);
    };

    updateTheme();
    if (settings.theme === "system") {
      systemTheme.addEventListener("change", updateTheme);
      return () => systemTheme.removeEventListener("change", updateTheme);
    }
  }, [settings.theme]);

  const scanMutation = useMutation({
    mutationFn: () => invoke<ScanResult>("scan_firefox"),
    onSuccess: (scan) => {
      queryClient.setQueryData(["firefox"], scan);
      void queryClient.invalidateQueries({ queryKey: ["shortcuts"] });
      setFeedback({ kind: "success", message: "Firefox scan completed." });
    },
    onError: (error) => setFeedback({ kind: "error", ...errorText(error) }),
  });

  const updateMutation = useMutation({
    mutationFn: ({
      shortcutId,
      request,
    }: {
      shortcutId: string;
      request: CreateShortcutRequest;
    }) =>
      invoke<ShortcutView>("update_shortcut", {
        shortcutId,
        request,
      }),
    onSuccess: (shortcut) => {
      void queryClient.invalidateQueries({ queryKey: ["shortcuts"] });
      setShortcutDialog(undefined);
      setFeedback({
        kind: "success",
        message: `Updated “${shortcut.name}” in the Start menu.`,
      });
    },
    onError: (error) => setFeedback({ kind: "error", ...errorText(error) }),
  });

  const createMutation = useMutation({
    mutationFn: (request: CreateShortcutRequest) =>
      invoke<ShortcutView>("create_shortcut", { request }),
    onSuccess: (shortcut) => {
      void queryClient.invalidateQueries({ queryKey: ["shortcuts"] });
      setShortcutDialog(undefined);
      setFeedback({
        kind: "success",
        message: `Created “${shortcut.name}” in the Start menu.`,
      });
    },
    onError: (error) => setFeedback({ kind: "error", ...errorText(error) }),
  });

  const refreshIconMutation = useMutation({
    mutationFn: (shortcut: ShortcutView) =>
      invoke<ShortcutView>("refresh_shortcut_icon", {
        shortcutId: shortcut.id,
      }),
    onSuccess: (updatedShortcut) => {
      queryClient.setQueryData<ShortcutView[]>(["shortcuts"], (current) =>
        current?.map((shortcut) =>
          shortcut.id === updatedShortcut.id ? updatedShortcut : shortcut,
        ),
      );
      setFeedback(
        updatedShortcut.iconUsedFallback
          ? {
              kind: "error",
              message:
                updatedShortcut.iconWarning ??
                "The site favicon could not be retrieved; the generated icon remains.",
            }
          : {
              kind: "success",
              message: `Updated the site favicon for “${updatedShortcut.name}”.`,
            },
      );
    },
    onError: (error) => setFeedback({ kind: "error", ...errorText(error) }),
  });

  const deleteMutation = useMutation({
    mutationFn: (shortcut: ShortcutView) =>
      invoke<void>("delete_shortcut", { shortcutId: shortcut.id }),
    onSuccess: (_, shortcut) => {
      void queryClient.invalidateQueries({ queryKey: ["shortcuts"] });
      setFeedback({
        kind: "success",
        message: `Removed “${shortcut.name}” from the Start menu.`,
      });
    },
    onError: (error) => setFeedback({ kind: "error", ...errorText(error) }),
  });

  const launchMutation = useMutation({
    mutationFn: (shortcut: ShortcutView) =>
      invoke<void>("launch_shortcut", { shortcutId: shortcut.id }),
    onSuccess: (_, shortcut) => {
      setFeedback({
        kind: "success",
        message: `Asked Firefox to open “${shortcut.name}”.`,
      });
    },
    onError: (error) => setFeedback({ kind: "error", ...errorText(error) }),
  });

  const locationMutation = useMutation({
    mutationFn: (shortcut: ShortcutView) =>
      invoke<void>("open_shortcut_location", { shortcutId: shortcut.id }),
    onError: (error) => setFeedback({ kind: "error", ...errorText(error) }),
  });

  const startMenuMutation = useMutation({
    mutationFn: () => invoke<void>("open_start_menu_folder"),
    onError: (error) => setFeedback({ kind: "error", ...errorText(error) }),
  });

  const settingsMutation = useMutation({
    mutationFn: (nextSettings: AppSettings) =>
      invoke<void>("save_settings", { settings: nextSettings }),
    onSuccess: (_, nextSettings) => {
      queryClient.setQueryData(["settings"], nextSettings);
      void queryClient.invalidateQueries({ queryKey: ["firefox"] });
      void queryClient.invalidateQueries({ queryKey: ["shortcuts"] });
      setFeedback({ kind: "success", message: "Settings saved." });
    },
    onError: (error) => setFeedback({ kind: "error", ...errorText(error) }),
  });

  const openExtensionMutation = useMutation({
    mutationFn: ({
      installationId,
      profileId,
      url,
    }: {
      installationId: string;
      profileId: string;
      url: string;
    }) =>
      invoke<void>("open_extension_page", {
        installationId,
        profileId,
        url,
      }),
    onError: (error) => setFeedback({ kind: "error", ...errorText(error) }),
  });

  const openAddonsPageMutation = useMutation({
    mutationFn: (url: string) => openUrl(url),
    onError: (error) => setFeedback({ kind: "error", ...errorText(error) }),
  });

  function openShortcutDialog(dialog: ShortcutDialogState) {
    createMutation.reset();
    updateMutation.reset();
    setShortcutDialog(dialog);
    setFeedback(undefined);
  }

  function openCreateDialog() {
    openShortcutDialog({ mode: "create" });
  }

  function duplicateShortcut(shortcut: ShortcutView) {
    openShortcutDialog({
      mode: "duplicate",
      shortcut: {
        ...shortcut,
        name: `${shortcut.name} copy`,
      },
    });
  }

  function editShortcut(shortcut: ShortcutView) {
    openShortcutDialog({ mode: "edit", shortcut });
  }

  function confirmDelete(shortcut: ShortcutView) {
    if (
      window.confirm(
        `Remove “${shortcut.name}” from the Start menu? This only deletes this managed shortcut.`,
      )
    ) {
      deleteMutation.mutate(shortcut);
    }
  }

  const scan = scanQuery.data;
  const shortcuts = shortcutsQuery.data ?? [];
  const startupError = settingsQuery.error
    ? errorText(settingsQuery.error)
    : undefined;
  const scanError = scanQuery.error ? errorText(scanQuery.error) : undefined;
  const shortcutsError = shortcutsQuery.error
    ? errorText(shortcutsQuery.error)
    : undefined;
  const dialogInitial =
    shortcutDialog && shortcutDialog.mode !== "create"
      ? shortcutDialog.shortcut
      : undefined;
  const dialogError =
    shortcutDialog?.mode === "edit"
      ? updateMutation.error
      : createMutation.error;

  return (
    <AppShell
      activePage={activePage}
      onNavigate={setActivePage}
      onScan={() => scanMutation.mutate()}
      scanning={scanMutation.isPending || scanQuery.isFetching}
    >
      {feedback ? (
        <div
          className={`mb-4 flex items-start gap-2 border p-4 ${
            feedback.kind === "success"
              ? "border-success/20 bg-success-soft text-success"
              : "border-danger/20 bg-danger-soft text-danger"
          }`}
          role={feedback.kind === "error" ? "alert" : "status"}
        >
          {feedback.kind === "error" ? (
            <AlertCircle size={17} aria-hidden="true" />
          ) : (
            <CheckCircle2 size={17} aria-hidden="true" />
          )}
          <div className="grid min-w-0 flex-1 gap-1 text-[13px]">
            <span>{feedback.message}</span>
            {feedback.details ? (
              <details className="text-xs text-muted-foreground">
                <summary className="w-fit cursor-pointer">Technical details</summary>
                <code className="mt-1 block [overflow-wrap:anywhere] font-mono">
                  {feedback.details}
                </code>
              </details>
            ) : null}
          </div>
          <button
            type="button"
            className={iconButton}
            aria-label="Dismiss message"
            onClick={() => setFeedback(undefined)}
          >
            <X size={15} aria-hidden="true" />
          </button>
        </div>
      ) : null}

      {startupError ? (
        <ErrorBanner
          message={`Settings could not be loaded: ${startupError.message}`}
          details={startupError.details}
        />
      ) : null}
      {scanError ? (
        <ErrorBanner
          message={`Firefox scan failed: ${scanError.message}`}
          details={scanError.details}
        />
      ) : null}
      {shortcutsError ? (
        <ErrorBanner
          message={`Shortcuts could not be loaded: ${shortcutsError.message}`}
          details={shortcutsError.details}
        />
      ) : null}

      {activePage === "shortcuts" && (
        <ShortcutsPage
          shortcuts={shortcuts}
          onOpenStartMenu={() => startMenuMutation.mutate()}
          onCreate={() => openCreateDialog()}
          onEdit={editShortcut}
          onDuplicate={duplicateShortcut}
          onLaunch={(shortcut) => launchMutation.mutate(shortcut)}
          onRefreshIcon={(shortcut) => refreshIconMutation.mutate(shortcut)}
          refreshingIcon={refreshIconMutation.isPending}
          onOpenLocation={(shortcut) => locationMutation.mutate(shortcut)}
          onDelete={confirmDelete}
        />
      )}
      {activePage === "firefox" && (
        <FirefoxPage
          scan={scan}
          settings={settings}
          onDefaultProfile={(profileId) =>
            settingsMutation.mutate({ ...settings, defaultProfileId: profileId })
          }
        />
      )}
      {activePage === "extensions" && (
        <ExtensionsPage
          scan={scan}
          settings={settings}
          onOpenAddonsPage={(url) => openAddonsPageMutation.mutate(url)}
          onOpenExtension={(installationId, profileId, url) =>
            openExtensionMutation.mutate({ installationId, profileId, url })
          }
        />
      )}
      {activePage === "settings" && (
        <SettingsPage
          settings={settings}
          scan={scan}
          saving={settingsMutation.isPending}
          onSave={(nextSettings) => settingsMutation.mutate(nextSettings)}
        />
      )}

      <ShortcutDialog
        open={Boolean(shortcutDialog)}
        mode={shortcutDialog?.mode ?? "create"}
        scan={scan}
        settings={settings}
        initial={dialogInitial}
        pending={createMutation.isPending || updateMutation.isPending}
        error={dialogError ? errorText(dialogError).message : undefined}
        onClose={() => {
          setShortcutDialog(undefined);
          createMutation.reset();
          updateMutation.reset();
        }}
        onSave={(request) => {
          if (shortcutDialog?.mode === "edit") {
            updateMutation.mutate({
              shortcutId: shortcutDialog.shortcut.id,
              request,
            });
          } else if (shortcutDialog) {
            createMutation.mutate(request);
          }
        }}
      />
    </AppShell>
  );
}

function ErrorBanner({
  message,
  details,
}: {
  message: string;
  details?: string;
}) {
  return (
    <div
      className="mb-4 flex items-start gap-2 border border-danger/20 bg-danger-soft p-4 text-danger"
      role="alert"
    >
      <AlertCircle size={16} aria-hidden="true" />
      <div className="grid min-w-0 flex-1 gap-1">
        <span>{message}</span>
        {details ? (
          <details className="text-xs text-muted-foreground">
            <summary className="w-fit cursor-pointer">Technical details</summary>
            <code className="mt-1 block [overflow-wrap:anywhere] font-mono">
              {details}
            </code>
          </details>
        ) : null}
      </div>
    </div>
  );
}

export default App;
