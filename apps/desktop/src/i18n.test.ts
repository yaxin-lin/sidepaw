import { describe, expect, it } from "vitest";
import { detectLang, t } from "./i18n";

describe("i18n", () => {
  it("picks Chinese for any zh tag and English otherwise", () => {
    expect(detectLang("zh-CN")).toBe("zh-CN");
    expect(detectLang("zh-Hant-TW")).toBe("zh-CN");
    expect(detectLang("en-US")).toBe("en");
    expect(detectLang("ja")).toBe("en");
  });

  it("substitutes variables and falls back to English", () => {
    expect(t("level.saved", { n: 3 }, "zh-CN")).toBe("档位 3 已写入 config.toml。");
    expect(t("level.saved", { n: 3 }, "en")).toBe("Level 3 saved to config.toml.");
  });
});
