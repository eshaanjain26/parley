import { describe, expect, it } from "vitest";
import { CalendarEvent, attendeeSummary, canRecordNow, startsIn } from "./calendar";

const MIN = 60_000;
const ev = (startMin: number, endMin: number, allDay = false): CalendarEvent => ({
  id: "e",
  seriesId: null,
  calendarId: "work",
  title: "Standup",
  startMs: startMin * MIN,
  endMs: endMin * MIN,
  allDay,
  attendees: [],
  url: "",
  location: "",
  callLink: null,
});

describe("calendar helpers", () => {
  it("allows recording from 10 minutes before until the end", () => {
    expect(canRecordNow(ev(60, 90), 50 * MIN)).toBe(true);
    expect(canRecordNow(ev(60, 90), 49 * MIN)).toBe(false);
    expect(canRecordNow(ev(60, 90), 90 * MIN)).toBe(false);
    expect(canRecordNow(ev(0, 24 * 60, true), 60 * MIN)).toBe(false);
  });

  it("says when an event starts", () => {
    expect(startsIn(ev(60, 90), 55 * MIN)).toBe("in 5 min");
    expect(startsIn(ev(300, 330), 60 * MIN)).toBe("in 4 h");
    expect(startsIn(ev(60, 90), 70 * MIN)).toBe("now");
    expect(startsIn(ev(60, 90), 95 * MIN)).toBe("ended");
  });

  it("summarises attendees by name, then email", () => {
    expect(attendeeSummary([])).toBe("");
    expect(attendeeSummary([{ name: "", email: "kim@example.com" }])).toBe("kim@example.com");
    expect(attendeeSummary([{ name: "Priya", email: "" }, { name: "Marco", email: "" }])).toBe("Priya and Marco");
    const many = ["Priya", "Marco", "Kim", "Ana"].map((name) => ({ name, email: "" }));
    expect(attendeeSummary(many)).toBe("Priya, Marco and 2 others");
    expect(attendeeSummary(many.slice(0, 3))).toBe("Priya, Marco and 1 other");
  });
});
