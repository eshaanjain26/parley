# Parley: build plan

Open-source, local-first AI meeting notes for macOS. Tauri 2 + Rust core, React + TypeScript UI. Audio and notes stay on the Mac by default; cloud speech and LLM providers are opt-in with the user's own keys.

## Ground rules

- Clean-room build. No Granola code, prompts, names, icons, or copy. The teardown informs which public macOS APIs to use, nothing more.
- Own repo: `eshaanjain26/parley`, one branch + PR per milestone, never direct to `main` after the bootstrap commit.
- License: MIT (change before first release if you want AGPL).
- Bundle ID: `com.eshaanjain.parley`. Name note: "Parley Notes" exists as an Android/browser recorder, so do a trademark search before paid marketing.
- Target: macOS 14.4+ on Apple Silicon for v1 (process taps and per-process mic detection need 14.2/14.4).

## Architecture

```
React UI (TipTap editor, library, settings)
        │ Tauri commands + events
Rust core
 ├─ audio      mic (cpal) + system audio (Swift sidecar: Core Audio process tap → PCM on stdout)
 │             → 16 kHz mono resample → simple VAD → chunks
 ├─ stt        trait SpeechToText: whisper.cpp local (Metal) | Deepgram stream | AssemblyAI stream
 ├─ speakers   mic = "Me", system = "Them" (v1); on-device diarization later
 ├─ llm        trait LlmProvider: Ollama | OpenAI-compatible | Anthropic; templates are local Markdown
 ├─ store      SQLite (SQLCipher), key in macOS Keychain; FTS5 search
 ├─ detect     Core Audio process list + IsRunningInput → "Meeting detected" notification
 └─ secrets    API keys in Keychain, never in plain files
```

## Milestones

- [x] **M0 Toolchain + scaffold**: install rustup and cmake, create Tauri 2 app, CI that builds and tests on macOS. Bootstrap repo.
- [ ] **M1 Capture**: Swift tap sidecar, mic capture, mixer, permission prompts (mic + system audio), level meters. Tests on resampler and chunker.
- [ ] **M2 Live transcript**: whisper.cpp local streaming with partials, then Deepgram and AssemblyAI behind the same trait. Model download manager. Tests with fixture WAVs.
- [ ] **M3 Notes**: TipTap editor beside the live transcript; meeting record saved to encrypted SQLite.
- [ ] **M4 AI notes**: "Enhance" merges your notes + transcript through a template; streaming output; 4 built-in templates (general, 1:1, sales call, standup) plus custom. Ollama default, BYO cloud keys.
- [ ] **M5 Library**: meeting list, full-text search, ask-across-meetings chat, Markdown/PDF export, copy to clipboard.
- [ ] **M6 Detection + polish**: meeting detection notification, menu-bar item, global hotkey, onboarding, branding (icon, colors, name).
- [ ] **M7 Release**: Developer ID signing, notarization, DMG, signed auto-updater on GitHub Releases, landing README, privacy policy.

## Deferred past v1

Calendar (EventKit/Google), speaker names, on-device diarization, Windows, integrations (Slack, Notion, CRM), sync, mobile.

## What you need to provide

- Apple Developer Program membership ($99/year) for signing and notarization in M7. Builds before M7 run unsigned locally.
- Approval to install rustup (to `~/.cargo`) and cmake (Homebrew) now, and later Ollama plus a ~2 GB local model if you want local summaries tested.

## Review

### M0 (2026-10-17)

- Rust 1.99.1 (rustup) and cmake 4.4.4 (Homebrew) installed.
- Tauri 2 + React 19 + TypeScript scaffold, branded: name, bundle ID `com.eshaanjain.parley`, icon, 1100x720 window, strict CSP, macOS 14.4 minimum, MIT license.
- `settings` module in Rust with `get_settings` / `update_settings` commands; atomic write-then-rename; 5 unit tests pass.
- UI shell: sidebar, Meetings empty state, working Settings form.
- CI: GitHub Actions on macOS 14 runs typecheck + build, rustfmt, clippy `-D warnings`, tests, and an unsigned `.app` bundle.
- Verified: release `.app` is 3.9 MB, opens, shows v0.1.0, and a Settings change saved through the UI landed in `~/Library/Application Support/com.eshaanjain.parley/settings.json`.

### M1 (2026-10-17, in review)

- `parley-audiotap` Swift sidecar: Core Audio process tap + private aggregate device, mono f32 PCM on stdout, JSON status on stderr, clean teardown on stdin close or SIGTERM. Compiled by `build.rs` and bundled as a Tauri `externalBin`.
- Mic capture via cpal 0.18 on its own thread; both tracks resampled to 16 kHz (windowed-sinc low-pass + linear interpolation) and written as WAV under `~/Library/Application Support/com.eshaanjain.parley/recordings/<id>/`.
- Live level meters (`audio-levels` event every 100 ms), Start/Stop UI, mic-only fallback with a warning when system audio is unavailable.
- Tests: 17 Rust (DSP, track writer, sidecar protocol, settings) + 6 Vitest (meter and duration helpers).
- Bugs found in live testing and fixed:
  - Core Audio skips the tap's IO callback while nothing plays, so the system track came out 6 s long for a 22 s recording. The sidecar now fills gaps with silence from the host clock (second run: 20.1 s vs 21.8 s).
  - The remaining 1.7 s offset came from the sidecar starting after the mic; the system track is now padded by the mic's lead-in. Unit-tested, live check pending.
  - `start_recording` ran on the main thread and froze the UI behind the permission prompt; the commands are now async with a 60 s first-run timeout.
- Dev note: every unsigned rebuild is a new app to macOS privacy controls, so mic and system-audio prompts reappear after each build. A stable signing identity (M7) fixes this.
- Disk: dependency debug info is off in dev builds (`target/` went from 3.8 GB to 1.2 GB).
