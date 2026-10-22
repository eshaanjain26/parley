import { describe, expect, it } from "vitest";
// @ts-expect-error plain ESM script without type declarations
import { rewriteFeed } from "./feed-urls.mjs";

const API = "https://api.github.com/repos/o/r/releases/assets/";
const assets = [
  { apiUrl: `${API}1`, name: "Parley_0.2.0_aarch64.app.tar.gz" },
  { apiUrl: `${API}2`, name: "Parley_0.2.0_x64.app.tar.gz" },
];
const feed = {
  version: "0.2.0",
  notes: "n",
  platforms: {
    "darwin-aarch64": { signature: "s1", url: `${API}1` },
    "darwin-x86_64": { signature: "s2", url: `${API}2` },
  },
};

describe("rewriteFeed", () => {
  it("swaps API asset URLs for release download URLs and keeps signatures", () => {
    const out = rewriteFeed(feed, assets, "o/r", "v0.2.0");
    expect(out.platforms["darwin-aarch64"]).toEqual({
      signature: "s1",
      url: "https://github.com/o/r/releases/download/v0.2.0/Parley_0.2.0_aarch64.app.tar.gz",
    });
    expect(out.platforms["darwin-x86_64"].url).toBe(
      "https://github.com/o/r/releases/download/v0.2.0/Parley_0.2.0_x64.app.tar.gz",
    );
    expect(out.version).toBe("0.2.0");
  });

  it("leaves URLs that are already download URLs alone", () => {
    const once = rewriteFeed(feed, assets, "o/r", "v0.2.0");
    expect(rewriteFeed(once, assets, "o/r", "v0.2.0")).toEqual(once);
  });

  it("fails when an API URL has no matching asset", () => {
    expect(() => rewriteFeed(feed, assets.slice(0, 1), "o/r", "v0.2.0")).toThrow(/darwin-x86_64/);
  });
});
