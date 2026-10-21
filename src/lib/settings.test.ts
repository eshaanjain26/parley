import { describe, expect, it } from "vitest";
import { hasKeyFor } from "./settings";

describe("hasKeyFor", () => {
  it("never requires a key for on-device transcription", () => {
    expect(hasKeyFor(null, "local-whisper")).toBe(true);
  });

  it("checks the matching provider's key", () => {
    const status = { deepgram: true, assemblyAi: false };
    expect(hasKeyFor(status, "deepgram")).toBe(true);
    expect(hasKeyFor(status, "assembly-ai")).toBe(false);
    expect(hasKeyFor(null, "deepgram")).toBe(false);
  });
});
