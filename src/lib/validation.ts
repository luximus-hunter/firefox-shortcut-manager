import { z } from "zod";

export const shortcutFormSchema = z.object({
  websiteName: z.string().trim().min(1, "Enter a website name.").max(120),
  targetUrl: z
    .string()
    .trim()
    .min(1, "Enter a website URL.")
    .max(8192, "The website URL is too long.")
    .refine(isSupportedWebsiteUrl, "Enter a valid HTTP or HTTPS website URL."),
  installationId: z.string().min(1, "Select a Firefox installation."),
  profileId: z.string().min(1, "Select a Firefox profile."),
  containerId: z.string().min(1, "Select an existing Firefox container.").nullable(),
  openAsApp: z.boolean(),
  shortcutName: z.string().trim().min(1, "Enter a shortcut name.").max(120),
  iconBackgroundColor: z.string().regex(/^#[0-9a-f]{6}$/i).nullable().default(null),
});

export type ShortcutFormValues = z.infer<typeof shortcutFormSchema>;

export function isSupportedWebsiteUrl(value: string): boolean {
  try {
    const url = new URL(value);
    return (
      (url.protocol === "http:" || url.protocol === "https:") &&
      url.hostname.length > 0 &&
      url.username.length === 0 &&
      url.password.length === 0
    );
  } catch {
    return false;
  }
}

export function websiteNameFromUrl(value: string): string {
  try {
    const hostname = new URL(value).hostname.replace(/^www\./i, "");
    const firstPart = hostname.split(".")[0] ?? hostname;
    return firstPart
      .replace(/[-_]+/g, " ")
      .replace(/\b\w/g, (letter) => letter.toUpperCase());
  } catch {
    return "";
  }
}

export function suggestedShortcutName(
  websiteName: string,
  containerName: string,
): string {
  if (!websiteName.trim()) return "";
  return containerName
    ? `${websiteName.trim()} (${containerName.trim()})`
    : websiteName.trim();
}
