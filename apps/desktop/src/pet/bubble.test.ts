// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from "vitest";

// jsdom has no requestAnimationFrame; run callbacks on the fake clock.
globalThis.requestAnimationFrame = (cb) => window.setTimeout(() => cb(0), 16);
import { Bubble } from "./bubble";
import { Lines } from "../lines";

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

  it("stays quiet on state changes in do-not-disturb, but answers clicks", () => {
    bubble.quiet = true;
    bubble.onState({ state: "waiting" });
    expect(el.textContent).not.toBe("needs you");
    bubble.onInteraction("click");
    expect(el.textContent).toBe("yes?");
    bubble.quiet = false;
  });

  it("shows the oldest prompt, the queue behind it and the shortcuts", () => {
    const answers: boolean[] = [];
    const p = { id: 1, key: "claude-code:a", tool: "Bash", summary: "rm -rf build", expires_in_ms: 15_000 };
    bubble.askApproval(p, 2, { allow: "⌃⌥Y", deny: null }, "sidepaw", (a) => answers.push(a));
    expect(el.querySelector(".q span")?.textContent).toBe("Claude Code in sidepaw wants to run Bash");
    expect(el.querySelector(".more")?.textContent).toBe("+2 waiting");
    expect(el.querySelector("code")?.textContent).toBe("rm -rf build");
    const [allow, deny] = Array.from(el.querySelectorAll("button"));
    expect(allow.querySelector("kbd")?.textContent).toBe("⌃⌥Y");
    expect(deny.querySelector("kbd")).toBeNull();
    // Same prompt on the next poll: only the count changes.
    bubble.askApproval(p, 0, {}, null, () => {});
    expect(el.querySelector(".more")?.textContent).toBe("");
    expect(el.querySelector("kbd")?.textContent).toBe("⌃⌥Y");
    deny.click();
    expect(answers).toEqual([false]);
    bubble.askApproval({ ...p, id: 2, summary: "npm publish" }, 0, {}, null, () => {});
    expect(el.querySelector("code")?.textContent).toBe("npm publish");
    bubble.clearApproval();
  });

  it("reports a nudge as answered on a click and dismissed on the cross, once", () => {
    const got: boolean[] = [];
    expect(bubble.nudge("Two hours straight. Stretch?", (a) => got.push(a))).toBe(true);
    expect(el.classList.contains("nudge")).toBe(true);
    el.querySelector<HTMLButtonElement>(".x")!.click();
    el.click();
    expect(got).toEqual([false]);
    bubble.nudge("Welcome back.", (a) => got.push(a));
    el.click();
    expect(got).toEqual([false, true]);
    bubble.say("plain line");
    expect(el.classList.contains("nudge")).toBe(false);
    expect(el.querySelector(".x")).toBeNull();
  });

  it("answers clicks", () => {
    bubble.onInteraction("click");
    expect(el.textContent).toBe("yes?");
    bubble.onInteraction("drag");
    expect(el.textContent).toBe("yes?"); // no drag lines in this pack: keeps the last line;
  });

  it("borrows click lines for a drop and always shows the file name", () => {
    bubble.onInteraction("drop", "a.txt");
    expect(el.textContent).toBe("a.txt\nyes?");
    const quiet = new Bubble(el, Lines.empty(), () => clock);
    quiet.onInteraction("drop", "b.txt");
    expect(el.textContent).toBe("b.txt");
  });
});
