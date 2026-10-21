import { describe, expect, it } from "vitest";
import { TranscriptLine, applyLine, formatBytes, formatTimestamp } from "./transcript";

const line = (over: Partial<TranscriptLine>): TranscriptLine => ({
  speaker: "me",
  segmentId: 0,
  isFinal: false,
  text: "hello",
  startMs: 0,
  endMs: 1000,
  ...over,
});

describe("applyLine", () => {
  it("replaces a partial with its newer partial and then its final", () => {
    let lines = applyLine([], line({ text: "the bud" }));
    lines = applyLine(lines, line({ text: "the budget is" }));
    lines = applyLine(lines, line({ text: "The budget is approved.", isFinal: true }));
    expect(lines).toHaveLength(1);
    expect(lines[0]).toMatchObject({ text: "The budget is approved.", isFinal: true });
  });

  it("keeps the same segment id from different speakers apart", () => {
    let lines = applyLine([], line({ speaker: "me", text: "a" }));
    lines = applyLine(lines, line({ speaker: "them", text: "b" }));
    expect(lines.map((l) => l.text)).toEqual(["a", "b"]);
  });

  it("orders lines by start time", () => {
    let lines = applyLine([], line({ segmentId: 1, startMs: 5000, text: "later" }));
    lines = applyLine(lines, line({ speaker: "them", segmentId: 0, startMs: 1000, text: "earlier" }));
    expect(lines.map((l) => l.text)).toEqual(["earlier", "later"]);
  });

  it("drops a segment whose final came back empty", () => {
    let lines = applyLine([], line({ text: "uh" }));
    lines = applyLine(lines, line({ text: "  ", isFinal: true }));
    expect(lines).toEqual([]);
  });
});

describe("formatting", () => {
  it("formats model sizes", () => {
    expect(formatBytes(147_964_211)).toBe("148 MB");
    expect(formatBytes(1_500_000_000)).toBe("1.5 GB");
  });

  it("formats timestamps", () => {
    expect(formatTimestamp(0)).toBe("0:00");
    expect(formatTimestamp(83_000)).toBe("1:23");
    expect(formatTimestamp(3_725_000)).toBe("62:05");
  });
});
