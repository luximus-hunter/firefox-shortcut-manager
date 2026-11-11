import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";
import { buttonSecondary } from "../../lib/ui";

function message(error: unknown): string {
  if (typeof error === "object" && error && "message" in error) {
    return String(error.message);
  }
  return "Could not change the app toolbar. Try again after closing Firefox.";
}

export function AppChromeControl({ profileId, unavailable }: {
  profileId: string;
  unavailable: boolean;
}) {
  const client = useQueryClient();
  const [notice, setNotice] = useState<string>();
  const key = ["app-chrome", profileId];
  const status = useQuery({
    queryKey: key,
    queryFn: () => invoke<boolean>("app_chrome_status", { profileId }),
    enabled: !unavailable,
    retry: false,
  });
  const mutation = useMutation({
    mutationFn: (enabled: boolean) => invoke("set_app_chrome", { profileId, enabled }),
    onSuccess: (_, enabled) => {
      client.setQueryData(key, enabled);
      setNotice(enabled
        ? "Installed. Reopen Firefox to apply the compact app toolbar."
        : "Original files restored. Reopen Firefox to apply the change.");
    },
    onError: () => {
      // A failed/interrupted install may already have saved its backup. Read
      // the recoverable state so the user can disable and restore it.
      void client.invalidateQueries({ queryKey: key });
      setNotice(undefined);
    },
  });
  const updateMutation = useMutation({
    mutationFn: () => invoke("update_app_chrome", { profileId }),
    onSuccess: () => setNotice("Latest toolbar stylesheet copied. Fully restart Firefox to apply it."),
    onError: () => setNotice(undefined),
  });
  const error = updateMutation.error ?? mutation.error ?? status.error;
  return (
    <div className="mt-4 border-t border-border pt-4">
      <div className="flex items-center justify-between gap-4 max-[680px]:flex-wrap">
        <div>
          <strong className="text-sm">Compact app toolbar</strong>
          <p className="mb-0 mt-1 text-xs text-muted-foreground">
            Keeps extensions, the profile button, and window controls in app windows.
            Close this Firefox profile before changing. Regular browsing stays unchanged.
          </p>
          <p className="mb-0 mt-1 text-xs text-muted-foreground">
            Installation backs up existing toolbar files; disabling restores them.
          </p>
        </div>
        <div className="flex flex-wrap gap-2">
        {status.data ? (
          <button
            type="button"
            className={buttonSecondary}
            disabled={unavailable || mutation.isPending || updateMutation.isPending || status.isFetching}
            onClick={() => { setNotice(undefined); mutation.reset(); updateMutation.mutate(); }}
          >
            {updateMutation.isPending ? "Updating…" : "Update toolbar"}
          </button>
        ) : null}
        <button
          type="button"
          className={buttonSecondary}
          disabled={unavailable || !status.isSuccess || mutation.isPending || updateMutation.isPending || status.isFetching}
          onClick={() => { setNotice(undefined); updateMutation.reset(); mutation.mutate(!status.data); }}
        >
          {mutation.isPending ? "Applying…" : status.data ? "Disable and restore original" : "Install app toolbar"}
        </button>
        </div>
      </div>
      {notice ? <p role="status" className="mb-0 mt-2 text-xs text-success">{notice}</p> : null}
      {error ? <p role="alert" className="mb-0 mt-2 text-xs text-danger">{message(error)}</p> : null}
      {status.isError ? (
        <button type="button" className={`${buttonSecondary} mt-2`} onClick={() => void status.refetch()}>
          Retry status
        </button>
      ) : null}
    </div>
  );
}
