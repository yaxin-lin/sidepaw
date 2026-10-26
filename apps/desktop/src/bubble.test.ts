// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from "vitest";

// jsdom has no requestAnimationFrame; run callbacks on the fake clock.
globalThis.requestAnimationFrame = (cb) => window.setTimeout(() => cb(0), 16);
import { Bubble } from "./bubble";
import { Lines } from "./lines";

const file = {
  version: 1,
  thinking: ["t"],
  tool: ["w"],
  waiting: ["needs you"],
  done: ["done"],
  click: ["yes?"],
};

describe("Bubble", () => {
  let el: HTMLElement;
  let clock: number;
  let bubble: Bubble;

  beforeEach(() => {
    vi.useFakeTimers();
    el = document.createElement("div");
    el.hidden = true;
    clock = 1_000_000;
    bubble = new Bubble(el, new Lines(file, () => 0), () => clock);
  });

  it("speaks on a transition and hides again", () => {
    bubble.onState({ state: "waiting" });
    expect(el.hidden).toBe(false);
    expect(el.textContent).toBe("needs you");
    vi.advanceTimersByTime(40);
    expect(el.classList.contains("show")).toBe(true);
    vi.advanceTimersByTime(10_000);
    expect(el.hidden).toBe(true);
    expect(el.classList.contains("show")).toBe(false);
  });

  it("does not repeat for the same state on every poll", () => {
    bubble.onState({ state: "waiting" });
    el.textContent = "";
    bubble.onState({ state: "waiting" });
    expect(el.textContent).toBe("");
  });

  it("throttles chatty states but not the ones that need the user", () => {
    bubble.onState({ state: "tool", detail: "shell" });
    expect(el.textContent).toBe("w");
    bubble.onState({ state: "thinking" });
    clock += 1_000;
    bubble.onState({ state: "tool", detail: "edit" });
    expect(el.textContent).toBe("t"); // tool again within a minute stays quiet;
    clock += 61_000;
    bubble.onState({ state: "thinking" });
    bubble.onState({ state: "tool", detail: "edit" });
    expect(el.textContent).toBe("w");

    bubble.onState({ state: "waiting" });
    bubble.onState({ state: "done" });
    clock += 1;
    bubble.onState({ state: "waiting" });
    expect(el.textContent).toBe("needs you");
  });

  it("answers clicks", () => {
    bubble.onInteraction("click");
    expect(el.textContent).toBe("yes?");
    bubble.onInteraction("drag");
    expect(el.textContent).toBe("yes?"); // no drag lines in this pack: keeps the last line;
  });
});
