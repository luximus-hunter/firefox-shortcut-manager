import { describe, expect, it } from "vitest";
import {
  shortcutFormSchema,
  isSupportedWebsiteUrl,
  suggestedShortcutName,
  websiteNameFromUrl,
} from "./validation";

describe("shortcut form validation helpers", () => {
  it("accepts HTTP and HTTPS and rejects unsafe protocols or embedded credentials", () => {
    expect(isSupportedWebsiteUrl("https://example.com")).toBe(true);
    expect(isSupportedWebsiteUrl("http://example.com/path")).toBe(true);
    expect(isSupportedWebsiteUrl("file:///C:/Windows")).toBe(false);
    expect(isSupportedWebsiteUrl("javascript:alert(1)")).toBe(false);
    expect(isSupportedWebsiteUrl("https://user:password@example.com")).toBe(false);
  });

  it("suggests a readable name from the hostname", () => {
    expect(websiteNameFromUrl("https://www.example-tools.com")).toBe(
      "Example Tools",
    );
    expect(websiteNameFromUrl("not a URL")).toBe("");
  });

  it("allows a shortcut without a Firefox container", () => {
    const request = {
      websiteName: "Example",
      targetUrl: "https://example.com",
      installationId: "firefox",
      profileId: "default",
      containerId: null,
      openAsApp: false,
      shortcutName: "Example",
    };

    expect(shortcutFormSchema.safeParse(request).success).toBe(true);
    expect(
      shortcutFormSchema.safeParse({ ...request, containerId: "" }).success,
    ).toBe(false);
  });

  it("accepts app-window shortcuts that target a Firefox container", () => {
    const request = {
      websiteName: "Example",
      targetUrl: "https://example.com",
      installationId: "firefox",
      profileId: "default",
      containerId: "firefox-container-1",
      openAsApp: true,
      shortcutName: "Example (Work)",
    };

    expect(shortcutFormSchema.safeParse(request).success).toBe(true);
  });

  it("includes the selected container in the default shortcut name", () => {
    expect(suggestedShortcutName("Teams", "Work")).toBe("Teams (Work)");
    expect(suggestedShortcutName("Teams", "")).toBe("Teams");
  });
});
