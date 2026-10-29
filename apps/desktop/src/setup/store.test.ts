import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/plugin-http", () => ({ fetch: vi.fn() }));

const { author, filterManifest, searchUrl } = await import("./store");

describe("petdex store", () => {
  it("asks the search API for one page, leaving out empty filters", () => {
    const url = new URL(searchUrl({ q: "", sort: "popular", kind: "" }, 24));
    expect(url.origin + url.pathname).toBe("https://petdex.dev/api/pets/search");
    expect(Object.fromEntries(url.searchParams)).toEqual({
      sort: "popular",
      limit: "24",
      cursor: "24",
      includeMeta: "0",
    });
    const narrowed = new URL(searchUrl({ q: "猫 cat", sort: "recent", kind: "creature" }, 0)).searchParams;
    expect(narrowed.get("q")).toBe("猫 cat");
    expect(narrowed.get("kinds")).toBe("creature");
  });

  it("reads the author from either shape", () => {
    const base = { slug: "a", displayName: "A", zipUrl: "z" };
    expect(author({ ...base, submittedBy: { name: "Miro" } })).toBe("Miro");
    expect(author({ ...base, submittedBy: "Serhat" })).toBe("Serhat");
    expect(author({ ...base, submittedBy: null })).toBe("");
  });

  it("filters the manifest by kind and name when the API is down", () => {
    const pets = [
      { slug: "mallow", displayName: "Mallow", kind: "creature", zipUrl: "" },
      { slug: "mecha-xiaobai", displayName: "机甲小白", kind: "character", zipUrl: "" },
      { slug: "mug", displayName: "Mug", kind: "object", zipUrl: "" },
    ];
    expect(filterManifest(pets, { q: "", sort: "popular", kind: "" })).toHaveLength(3);
    expect(filterManifest(pets, { q: "MAL", sort: "popular", kind: "" }).map((p) => p.slug)).toEqual(["mallow"]);
    expect(filterManifest(pets, { q: "小白", sort: "popular", kind: "character" }).map((p) => p.slug)).toEqual([
      "mecha-xiaobai",
    ]);
    expect(filterManifest(pets, { q: "m", sort: "popular", kind: "object" }).map((p) => p.slug)).toEqual(["mug"]);
  });
});
