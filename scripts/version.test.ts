import { describe, expect, it } from "vitest";
// @ts-expect-error plain ESM script without type declarations
import { cargoVersion, setCargoVersion, setJsonVersion } from "./version.mjs";

const CARGO = `[package]
name = "parley"
version = "0.1.0"
edition = "2024"

[dependencies]
serde = { version = "1" }
tauri-plugin-updater = "=2.13.2"
`;

describe("version script", () => {
  it("sets the package version and leaves dependency versions alone", () => {
    const out = setCargoVersion(CARGO, "0.2.0");
    expect(cargoVersion(out)).toBe("0.2.0");
    expect(out).toContain('serde = { version = "1" }');
    expect(out).toContain('tauri-plugin-updater = "=2.13.2"');
  });

  it("rejects a Cargo.toml without a package version", () => {
    expect(() => setCargoVersion("[package]\nname = \"x\"\n", "1.0.0")).toThrow();
  });

  it("updates JSON and keeps other fields", () => {
    const out = setJsonVersion('{"name":"parley","version":"0.1.0"}', "0.2.0");
    expect(JSON.parse(out)).toEqual({ name: "parley", version: "0.2.0" });
    expect(out.endsWith("\n")).toBe(true);
  });
});
