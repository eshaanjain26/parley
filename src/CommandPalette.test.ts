import { describe, expect, it } from "vitest";
import { matches } from "./CommandPalette";

describe("palette matching", () => {
  it("matches every typed word anywhere in the label, ignoring case", () => {
    expect(matches("New folder", "new fol")).toBe(true);
    expect(matches("Match system theme", "theme sys")).toBe(true);
    expect(matches("Start recording", "stop")).toBe(false);
    expect(matches("Settings", "")).toBe(true);
  });
});
