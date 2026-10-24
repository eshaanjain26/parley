import { describe, expect, it } from "vitest";
import { resolveMode } from "./theme";

describe("resolveMode", () => {
  it("follows the system only when asked", () => {
    expect(resolveMode("system", true)).toBe("dark");
    expect(resolveMode("system", false)).toBe("light");
    expect(resolveMode("light", true)).toBe("light");
    expect(resolveMode("dark", false)).toBe("dark");
  });
});
