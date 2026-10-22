import { describe, expect, it } from "vitest";
import { dayLabel, daysAgo, greeting, groupByDay } from "./dates";

const now = new Date(2026, 9, 21, 15, 30); // Wed Oct 21 2026, 3:30 PM local
const at = (y: number, m: number, d: number, h = 12) => new Date(y, m, d, h).getTime();

describe("dayLabel", () => {
  it("names today and yesterday", () => {
    expect(dayLabel(at(2026, 9, 21, 0), now, "en-US")).toBe("Today");
    expect(dayLabel(at(2026, 9, 20, 23), now, "en-US")).toBe("Yesterday");
  });

  it("uses weekday and date this year, and adds the year otherwise", () => {
    expect(dayLabel(at(2026, 9, 19), now, "en-US")).toBe("Monday, Oct 19");
    expect(dayLabel(at(2025, 11, 31), now, "en-US")).toBe("Dec 31, 2025");
  });

  it("counts calendar days, not 24-hour spans", () => {
    expect(daysAgo(new Date(2026, 9, 20, 23, 59), new Date(2026, 9, 21, 0, 1))).toBe(1);
  });
});

describe("groupByDay", () => {
  it("groups newest-first meetings by calendar day and keeps order", () => {
    const items = [
      { id: "a", startedAtMs: at(2026, 9, 21, 14) },
      { id: "b", startedAtMs: at(2026, 9, 21, 9) },
      { id: "c", startedAtMs: at(2026, 9, 20, 16) },
      { id: "d", startedAtMs: at(2026, 9, 19, 20) },
    ];
    const groups = groupByDay(items, now, "en-US");
    expect(groups.map((g) => g.label)).toEqual(["Today", "Yesterday", "Monday, Oct 19"]);
    expect(groups[0].items.map((i) => i.id)).toEqual(["a", "b"]);
  });

  it("returns no groups for no meetings", () => {
    expect(groupByDay([], now)).toEqual([]);
  });
});

describe("greeting", () => {
  it("follows the time of day", () => {
    expect(greeting(new Date(2026, 9, 21, 8))).toBe("Good morning");
    expect(greeting(new Date(2026, 9, 21, 13))).toBe("Good afternoon");
    expect(greeting(new Date(2026, 9, 21, 20))).toBe("Good evening");
  });
});
