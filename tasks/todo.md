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
- [x] **M1 Capture**: Swift tap sidecar, mic capture, mixer, permission prompts (mic + system audio), level meters. Tests on resampler and chunker.
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

### M1 (2026-10-17 to 2026-10-18)

- `parley-audiotap` Swift sidecar: Core Audio process tap + private aggregate device, mono f32 PCM on stdout, JSON status on stderr, clean teardown on stdin close or SIGTERM. Compiled by `build.rs` and bundled as a Tauri `externalBin`.
- Mic capture via cpal 0.18 on its own thread; both tracks resampled to 16 kHz (windowed-sinc low-pass + linear interpolation) and written as WAV under `~/Library/Application Support/com.eshaanjain.parley/recordings/<id>/`.
- Live level meters (`audio-levels` event every 100 ms), Start/Stop UI, mic-only fallback with a warning when system audio is unavailable.
- Tests: 17 Rust (DSP, track writer, sidecar protocol, settings) + 6 Vitest (meter and duration helpers).
- Bugs found in live testing and fixed:
  - Core Audio skips the tap's IO callback while nothing plays, so the system track came out 6 s long for a 22 s recording. The sidecar now fills gaps with silence from the host clock (second run: 20.1 s vs 21.8 s).
  - The remaining 1.7 s offset came from the sidecar starting after the mic; the system track is now padded by the mic's lead-in. Live check on 2026-10-18: tracks 22.35 s vs 22.40 s, and a played sound starts 50 ms later in the mic track than the system track (speaker-to-mic delay).
  - `start_recording` ran on the main thread and froze the UI behind the permission prompt; the commands are now async with a 60 s first-run timeout.
- Dev note: every unsigned rebuild is a new app to macOS privacy controls, so mic and system-audio prompts reappear after each build. A stable signing identity (M7) fixes this.
- Disk: dependency debug info is off in dev builds (`target/` went from 3.8 GB to 1.2 GB).
- Machine note: this Mac runs at 98-99% disk with up to 5 GB of swap, which twice stopped builds with "No space left on device". M2 needs room for whisper.cpp and a model (~150 MB to 1.5 GB).

### M2a local transcription (2026-10-20)

- `stt` module: `SpeechToText` trait (push 16 kHz audio, get partial/final events with stable segment ids) so cloud engines slot in later.
- Segmenter: adaptive energy VAD, 300 ms pre-roll, partial every 1.5 s, final after 700 ms pause or 20 s cap, drops sub-250 ms blips.
- whisper.cpp engine via whisper-rs 0.16 (Metal); hallucination filter for `[BLANK_AUDIO]`-style annotations; stale partials collapsed after a backlog. whisper.cpp builds optimized even in dev.
- Model manager: tiny/base/small English models with sizes and SHA-256 from the Hugging Face LFS pointers; streamed `.part` download, verified before rename, cancellable, progress events.
- Recording taps feed one transcriber thread per speaker (You = mic, Them = system audio); `transcript.json` saved per recording.
- UI: live transcript with partials in italics, in-app model download with progress, model picker in Settings.
- Tests: 40 Rust + 1 model-backed fixture test (spoken WAV from `say`, run in CI against tiny.en) + 12 Vitest.
- Live check: spoken sentence played through system audio appeared as a final "Them" line within the 13 s recording, matching the script word for word; `transcript.json` correct.
- Known issue (fixed in the echo-cancellation PR): on laptop speakers the mic heard the call, so "Them" speech was duplicated as "You".
- Local machine note: `/Library/Developer/CommandLineTools/usr/include/c++/v1` is a stale 11-file leftover that hides the SDK's libc++ headers, so C++ builds fail without `CXXFLAGS="-isystem $(xcrun --show-sdk-path)/usr/include/c++/v1"`. Removing that folder (sudo) or reinstalling the Command Line Tools fixes it for good.

### Echo cancellation (2026-10-20)

- New `parley-mic` Swift sidecar: AVAudioEngine with voice processing enabled (Apple's echo cancellation), other-audio ducking set to minimum so call audio isn't turned down. Falls back to plain cpal capture with a UI warning.
- Shared sidecar runner for both capture helpers; the transcriber tap is handed over only once a source is ready, so the fallback keeps it.
- Each sidecar's stderr and exit status go to `<track>.log` in the recording folder; the UI warns if a track stops mid-recording.
- Bug found through those logs: `parley-audiotap` crashed when voice processing was on ("failed to allocate 147573952589675104 bytes"). Input latency made buffer timestamps predate the start time and the unsigned subtraction wrapped to ~2^64. Fixed with a signed comparison.
- Live check: sentence played through laptop speakers is transcribed once, as "Them"; the mic is gated to -51 to -180 dB during playback; tracks 29.40 s vs 29.72 s; both sidecars stopped cleanly.
- Follow-up: whisper transcribes steady room noise (mic at -30 to -40 dB) into short nonsense lines. Filter on whisper's no-speech probability and tighten the VAD for the mic track.
