import { buttonVariants } from "@/components/ui/button";

export const pageStack = "grid gap-4";
export const panelCard =
  "min-w-0 bg-card p-4 text-xs leading-relaxed text-card-foreground ring-1 ring-foreground/10";
export const sectionHeading = "mb-4 flex items-center justify-between gap-4";
export const sectionKicker =
  "text-xs font-medium tracking-[0.09em] text-muted-foreground uppercase";
export const sectionTitle = "mt-1 text-sm leading-5 font-medium";
export const badge =
  "inline-flex h-5 w-fit shrink-0 items-center gap-1 border border-border px-2 text-xs leading-4 font-medium whitespace-nowrap";
export const accessPanel = "border border-access-border bg-access-panel";

const buttonClass = "enabled:cursor-pointer";

export const buttonPrimary = buttonVariants({ className: buttonClass });
export const buttonSecondary = buttonVariants({ variant: "outline", className: buttonClass });
export const buttonGhost = buttonVariants({ variant: "ghost", className: buttonClass });
export const iconButton =
  buttonVariants({ variant: "ghost", size: "icon", className: buttonClass });
export const dangerHover = "hover:bg-danger/10 hover:text-danger dark:hover:bg-danger/20";

export const field = "mb-4 grid min-w-0 gap-2";
export const fieldLabel = "text-[13px] font-[580] text-foreground";
export const fieldControl =
  "h-8 w-full min-w-0 border border-input bg-background px-2 text-xs text-foreground transition-colors focus-visible:border-ring aria-[invalid=true]:border-danger dark:bg-input/30";
export const fieldHelp = "text-xs text-muted-foreground";
export const fieldError = "text-xs text-danger";
export const mutedCopy = "mt-2 text-[13px] text-muted-foreground";
export const emptyMessage =
  "grid justify-items-center px-4 py-8 text-center text-muted-foreground";
export const emptyIcon =
  "grid size-8 place-items-center border border-access-border bg-access-panel text-highlight";
export const shortcutIcon =
  "shrink-0 border border-border bg-muted object-contain";

