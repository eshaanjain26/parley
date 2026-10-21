import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createAutosaver, defaultTitle } from "./meetings";

describe("defaultTitle", () => {
  it("names the meeting by date and time", () => {
    expect(defaultTitle(new Date(2026, 9, 21, 9, 5), "en-US")).toBe("Meeting on Oct 21 at 9:05 AM");
  });
});

describe("createAutosaver", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("coalesces rapid edits into one save of the latest value", async () => {
    const saved: string[] = [];
    const saver = createAutosaver(async (v: string) => void saved.push(v), 800);
    saver.schedule("a");
    await vi.advanceTimersByTimeAsync(300);
    saver.schedule("ab");
    await vi.advanceTimersByTimeAsync(300);
    saver.schedule("abc");
    expect(saved).toEqual([]);
    await vi.advanceTimersByTimeAsync(800);
    expect(saved).toEqual(["abc"]);
  });

  it("flush saves immediately and cancels the timer", async () => {
    const saved: string[] = [];
    const saver = createAutosaver(async (v: string) => void saved.push(v), 800);
    saver.schedule("draft");
    await saver.flush();
    expect(saved).toEqual(["draft"]);
    await vi.advanceTimersByTimeAsync(2000);
    expect(saved).toEqual(["draft"]);
    expect(saver.hasPending()).toBe(false);
  });

  it("saves an edit made during a slow save right after it, never in parallel", async () => {
    let active = 0;
    let maxActive = 0;
    const saved: string[] = [];
    const saver = createAutosaver(async (v: string) => {
      active++;
      maxActive = Math.max(maxActive, active);
      await new Promise((r) => setTimeout(r, 500));
      saved.push(v);
      active--;
    }, 100);
    saver.schedule("first");
    await vi.advanceTimersByTimeAsync(150);
    saver.schedule("second");
    await vi.advanceTimersByTimeAsync(2000);
    expect(saved).toEqual(["first", "second"]);
    expect(maxActive).toBe(1);
  });

  it("reports save errors without throwing", async () => {
    const errors: unknown[] = [];
    const saver = createAutosaver(async () => {
      throw new Error("disk full");
    }, 10, (e) => errors.push(e));
    saver.schedule("x");
    await saver.flush();
    expect(String(errors[0])).toContain("disk full");
  });
});
