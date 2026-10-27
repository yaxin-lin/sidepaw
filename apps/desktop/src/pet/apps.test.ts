import { describe, expect, it } from "vitest";
import { categorize } from "./apps";

describe("categorize", () => {
  it("knows common apps on both platforms, by id, case-insensitively", () => {
    expect(categorize({ name: "zoom.us", id: "us.zoom.xos" })).toBe("meeting");
    expect(categorize({ name: "Zoom", id: "Zoom.exe" })).toBe("meeting");
    expect(categorize({ name: "腾讯会议", id: "com.tencent.meeting" })).toBe("meeting");
    expect(categorize({ name: "Code", id: "com.microsoft.VSCode" })).toBe("editor");
    expect(categorize({ name: "IntelliJ IDEA", id: "com.jetbrains.intellij" })).toBe("editor");
    expect(categorize({ name: "WindowsTerminal", id: "WindowsTerminal.exe" })).toBe("terminal");
    expect(categorize({ name: "Spotify", id: "com.spotify.client" })).toBe("music");
    expect(categorize({ name: "Finder", id: "com.apple.finder" })).toBeNull();
    expect(categorize(null)).toBeNull();
  });
});
