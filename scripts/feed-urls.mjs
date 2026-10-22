// Rewrites the update feed (latest.json) so each platform downloads from
// github.com/<repo>/releases/download/<tag>/<asset> instead of an api.github.com asset URL.
// tauri-action writes API URLs for draft releases; those count against GitHub's
// unauthenticated API rate limit, so every updating Mac behind one IP shares 60 requests an hour.
// Usage: node scripts/feed-urls.mjs <latest.json> <assets.json> <owner/repo> <tag>
//   assets.json is `gh release view <tag> --json assets`.
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

export function rewriteFeed(feed, assets, repo, tag) {
  const byApiUrl = new Map(assets.map((a) => [a.apiUrl, a.name]));
  const platforms = {};
  for (const [platform, entry] of Object.entries(feed.platforms ?? {})) {
    let url = entry.url;
    if (url.startsWith("https://api.github.com/")) {
      const name = byApiUrl.get(url);
      if (!name) throw new Error(`${platform}: no release asset for ${url}`);
      url = `https://github.com/${repo}/releases/download/${encodeURIComponent(tag)}/${encodeURIComponent(name)}`;
    }
    platforms[platform] = { ...entry, url };
  }
  return { ...feed, platforms };
}

function main() {
  const [feedPath, assetsPath, repo, tag] = process.argv.slice(2);
  if (!tag) {
    console.error("usage: node scripts/feed-urls.mjs <latest.json> <assets.json> <owner/repo> <tag>");
    process.exit(2);
  }
  const feed = JSON.parse(readFileSync(feedPath, "utf8"));
  const { assets } = JSON.parse(readFileSync(assetsPath, "utf8"));
  const out = rewriteFeed(feed, assets, repo, tag);
  writeFileSync(feedPath, JSON.stringify(out, null, 2) + "\n");
  for (const [p, e] of Object.entries(out.platforms)) console.log(`${p}: ${e.url}`);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) main();
