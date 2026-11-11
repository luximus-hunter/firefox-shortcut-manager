import { PanelsTopLeft, RefreshCw } from "lucide-react";
import type { ReactNode } from "react";
import { Button } from "@/components/ui/button";
import { sectionKicker } from "@/lib/ui";

export type PageKey = "shortcuts" | "firefox" | "extensions" | "settings";

const navigation: { id: PageKey; label: string }[] = [
  { id: "shortcuts", label: "Shortcuts" },
  { id: "firefox", label: "Firefox" },
  { id: "extensions", label: "Extensions" },
  { id: "settings", label: "Settings" },
];

interface AppShellProps {
  activePage: PageKey;
  onNavigate: (page: PageKey) => void;
  onScan: () => void;
  scanning: boolean;
  children: ReactNode;
}

export function AppShell({ activePage, onNavigate, onScan, scanning, children }: AppShellProps) {
  return (
    <div className="mx-auto max-w-[54rem] p-4 sm:p-8">
      <header className="mb-4">
        <div className="mb-2 flex items-center gap-2">
          <PanelsTopLeft className="size-5 text-highlight" aria-hidden="true" />
          <p className={sectionKicker}>Firefox Shortcut Manager</p>
        </div>
        <h1 className="mb-4">{navigation.find(({ id }) => id === activePage)?.label}</h1>
        <div className="flex flex-wrap items-center justify-between gap-2">
          <nav className="flex flex-wrap gap-2" aria-label="Main navigation">
            {navigation.map(({ id, label }) => (
              <Button
                key={id}
                type="button"
                variant={activePage === id ? "default" : "outline"}
                className="enabled:cursor-pointer"
                onClick={() => onNavigate(id)}
                aria-current={activePage === id ? "page" : undefined}
              >
                {label}
              </Button>
            ))}
          </nav>
          <Button type="button" variant="outline" className="enabled:cursor-pointer" onClick={onScan} disabled={scanning}>
            <RefreshCw className={scanning ? "size-3.5 animate-spin" : "size-3.5"} data-icon="inline-start" aria-hidden="true" />
            {scanning ? "Scanning…" : "Rescan Firefox"}
          </Button>
        </div>
      </header>
      <main>{children}</main>
    </div>
  );
}
