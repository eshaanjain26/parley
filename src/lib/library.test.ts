import { describe, expect, it } from "vitest";
import { formatMinutes, formatWhen, plainSnippet, splitSnippet, utcOffsetMinutes } from "./library";

describe("splitSnippet", () => {
  it("separates highlighted matches from plain text", () => {
    expect(splitSnippet("…owns \u0001Priya\u0002's \u0001forecast\u0002 now")).toEqual([
      { text: "…owns ", hit: false },
      { text: "Priya", hit: true },
      { text: "'s ", hit: false },
      { text: "forecast", hit: true },
      { text: " now", hit: false },
    ]);
  });

  it("handles snippets without highlights", () => {
    expect(splitSnippet("plain")).toEqual([{ text: "plain", hit: false }]);
    expect(splitSnippet("")).toEqual([]);
  });
});

describe("formatWhen", () => {
  const now = new Date(2026, 9, 21, 15, 0);

  it("says Today and Yesterday for recent meetings", () => {
    expect(formatWhen(new Date(2026, 9, 21, 9, 5).getTime(), now, "en-US")).toBe("Today 9:05 AM");
    expect(formatWhen(new Date(2026, 9, 20, 16, 30).getTime(), now, "en-US")).toBe("Yesterday 4:30 PM");
  });

  it("uses the date otherwise, adding the year only when it differs", () => {
    expect(formatWhen(new Date(2026, 9, 14, 9, 5).getTime(), now, "en-US")).toBe("Oct 14, 9:05 AM");
    expect(formatWhen(new Date(2025, 11, 31, 9, 5).getTime(), now, "en-US")).toBe("Dec 31, 2025, 9:05 AM");
  });
});

describe("formatMinutes", () => {
  it("rounds to minutes and adds hours past 60", () => {
    expect(formatMinutes(null)).toBe("");
    expect(formatMinutes(0)).toBe("");
    expect(formatMinutes(42)).toBe("1 min");
    expect(formatMinutes(1_530)).toBe("26 min");
    expect(formatMinutes(3_725)).toBe("1 h 2 min");
  });
});

describe("utcOffsetMinutes", () => {
  it("is the negated JS timezone offset", () => {
    const d = new Date();
    expect(utcOffsetMinutes(d)).toBe(-d.getTimezoneOffset());
  });
});

describe("plainSnippet", () => {
  it("removes Markdown from AI-notes snippets and keeps highlights", () => {
    const raw = "## Summary The quarterly budget review moves to **\u0001Friday\u0002**. ## Decisions - Review on \u0001Friday\u0002 ## Action items - [ ] Send numbers";
    expect(plainSnippet(raw)).toBe(
      "Summary The quarterly budget review moves to \u0001Friday\u0002. Decisions Review on \u0001Friday\u0002 Action items Send numbers",
    );
  });

  it("leaves ordinary text, hyphenated words and hashtags alone", () => {
    expect(plainSnippet("a well-known #launch plan - 3 items")).toBe("a well-known #launch plan 3 items");
    expect(plainSnippet("…owns \u0001Priya\u0002's forecast")).toBe("…owns \u0001Priya\u0002's forecast");
  });
});
