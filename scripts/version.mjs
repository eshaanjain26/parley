// Sets the app version everywhere it lives: package.json, src-tauri/Cargo.toml and
// src-tauri/tauri.conf.json. Usage: npm run set-version -- 0.2.0
//   With --check, verifies all three match the given version instead (used by the release workflow).
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const SEMVER = /^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/;

export function setJsonVersion(text, version) {
  const json = JSON.parse(text);
  json.version = version;
  return JSON.stringify(json, null, 2) + "\n";
}

/** Replaces the version in Cargo.toml's [package] table only, not a dependency's. */
export function setCargoVersion(text, version) {
  const pkg = text.match(/\[package\][\s\S]*?(?=\n\[|$)/);
  if (!pkg || !/^version = ".*"$/m.test(pkg[0])) throw new Error("no version in [package]");
  const updated = pkg[0].replace(/^version = ".*"$/m, `version = "${version}"`);
  return text.replace(pkg[0], updated);
}

export function cargoVersion(text) {
  const pkg = text.match(/\[package\][\s\S]*?(?=\n\[|$)/);
  return pkg?.[0].match(/^version = "(.*)"$/m)?.[1];
}

function main() {
  const args = process.argv.slice(2);
  const check = args.includes("--check");
  const version = args.find((a) => !a.startsWith("--"))?.replace(/^v/, "");
  if (!version || !SEMVER.test(version)) {
    console.error("usage: npm run set-version -- <x.y.z> [--check]");
    process.exit(2);
  }
  const root = join(dirname(fileURLToPath(import.meta.url)), "..");
  const files = {
    pkg: join(root, "package.json"),
    cargo: join(root, "src-tauri/Cargo.toml"),
    conf: join(root, "src-tauri/tauri.conf.json"),
  };
  const read = (p) => readFileSync(p, "utf8");
  if (check) {
    const found = {
      "package.json": JSON.parse(read(files.pkg)).version,
      "Cargo.toml": cargoVersion(read(files.cargo)),
      "tauri.conf.json": JSON.parse(read(files.conf)).version,
    };
    const wrong = Object.entries(found).filter(([, v]) => v !== version);
    if (wrong.length) {
      console.error(`version ${version} expected; found ${wrong.map(([f, v]) => `${f}=${v}`).join(", ")}`);
      process.exit(1);
    }
    console.log(`all versions are ${version}`);
    return;
  }
  writeFileSync(files.pkg, setJsonVersion(read(files.pkg), version));
  writeFileSync(files.cargo, setCargoVersion(read(files.cargo), version));
  writeFileSync(files.conf, setJsonVersion(read(files.conf), version));
  console.log(`set version ${version}; run cargo check to refresh Cargo.lock`);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) main();
