import { describe, expect, it } from "vitest";
import { dbToPercent, formatDuration } from "./recording";

describe("dbToPercent", () => {
  it("clamps silence and full scale", () => {
    expect(dbToPercent(-100)).toBe(0);
    expect(dbToPercent(-60)).toBe(0);
    expect(dbToPercent(0)).toBe(100);
    expect(dbToPercent(3)).toBe(100);
  });

  it("scales linearly in dB between the floor and 0", () => {
    expect(dbToPercent(-30)).toBe(50);
    expect(dbToPercent(-15)).toBe(75);
  });

  it("treats NaN as silence", () => {
    expect(dbToPercent(Number.NaN)).toBe(0);
  });
});

describe("formatDuration", () => {
  it("formats minutes and seconds", () => {
    expect(formatDuration(0)).toBe("0:00");
    expect(formatDuration(75.9)).toBe("1:15");
  });

  it("adds hours when needed", () => {
    expect(formatDuration(3725)).toBe("1:02:05");
  });

  it("never goes negative", () => {
    expect(formatDuration(-4)).toBe("0:00");
  });
});
