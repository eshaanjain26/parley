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
- [x] **M2 Live transcript**: whisper.cpp local streaming with partials, then Deepgram and AssemblyAI behind the same trait. Model download manager. Tests with fixture WAVs.
- [x] **M3 Notes**: TipTap editor beside the live transcript; meeting record saved to encrypted SQLite.
- [x] **M4 AI notes**: "Enhance" merges your notes + transcript through a template; streaming output; 4 built-in templates (general, 1:1, sales call, standup) plus custom. Ollama default, BYO cloud keys.
- [x] **M5 Library**: meeting list, full-text search, ask-across-meetings chat, Markdown/PDF export, copy to clipboard.
- [x] **M6 Detection + polish**: meeting detection notification, menu-bar item, global hotkey, onboarding, branding (icon, colors, name).
- [ ] **M7 Release**: Developer ID signing, notarization, DMG, signed auto-updater on GitHub Releases, landing README, privacy policy.

## M7 plan (2026-10-21)

Decisions: no Apple Developer membership yet, so signing and notarization switch on from GitHub secrets when they exist and releases ship ad-hoc signed until then. The main repo goes public (MIT) and hosts releases plus the update feed.

- [x] **Updater in the app.** tauri-plugin-updater + tauri-plugin-process. Feed: `releases/latest/download/latest.json` on this repo, verified with a minisign public key in `tauri.conf.json`. Quiet check on launch (Settings toggle, default on) and a "Check for updates" button; a banner offers "Restart to update" once downloaded. A local feed is tested by building with a `--config` overlay (test key, http endpoint); no test hooks ship in the app.
- [ ] **Updater signing key.** You generate it (`npx tauri signer generate`) so you own the password; I read only the `.pub` file. You add the private key and password as GitHub secrets with `gh secret set` (I give you the commands; I don't handle the key).
- [x] **Release workflow.** On a `v*` tag: matrix build on macos-14 for `aarch64-apple-darwin` and `x86_64-apple-darwin` (build.rs already compiles sidecars per arch), tauri-action creates a draft release with both DMGs, the updater `.app.tar.gz` + `.sig`, and a merged `latest.json`. Fails if the tag doesn't match the app version.
- [x] **Signing switch.** With `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` set: Developer ID signing, hardened runtime, notarization and stapling. Without them: ad-hoc signing (`-`). Documented trade-off: ad-hoc builds lose mic/system-audio permission on every update.
- [x] **Version script.** `npm run version -- 0.2.0` updates package.json, Cargo.toml and tauri.conf.json together; unit-tested.
- [ ] **Docs.** Landing README (what it does, screenshot, install incl. right-click > Open for unsigned builds, privacy summary, build from source), `PRIVACY.md` (what stays on the Mac, what goes to each optional provider, no telemetry), `SECURITY.md`, `CHANGELOG.md`.
- [ ] **Pre-public audit.** Scan full git history for secrets and personal data, confirm no third-party product assets, check LICENSE and third-party notices (whisper.cpp MIT, model download licenses).
- [ ] **Verify.** Local release DMG build; updater end to end on this Mac: install 0.1.0, serve a signed 0.1.1 from a local feed, update and relaunch. CI green on the PR.
- [ ] **Go public + first release.** Only after you confirm: flip the repo to public, tag `v0.1.0`, publish the draft release.

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

### M2b cloud transcription (2026-10-21)

- `StreamingEngine`: one I/O thread owns the WebSocket (tungstenite over rustls), sends 100 ms PCM16 chunks, polls replies with a 20 ms read timeout, and on finish sends the provider's close message and drains finals for up to 8 s. Connects during `start_recording`, so a bad key fails up front ("Deepgram rejected the API key (401)").
- Deepgram (`nova-3`, interim results, smart formatting) and AssemblyAI v3 (`format_turns`, final only once the formatted turn arrives) are each just a URL, an auth header, a close message and a JSON parser behind `StreamParser`.
- API keys are stored in the macOS Keychain (`security-framework` generic passwords, service `com.eshaanjain.parley`); the UI only sees whether a key exists.
- Settings: masked key field with save/remove for the selected cloud provider; the on-device model picker hides for cloud engines. Starting without a key explains what to do.
- Tests: 47 Rust (incl. end-to-end streaming against a local WebSocket server: chunking, auth header, final on finish, provider error, connection refused; parser tests for both providers) + 14 Vitest.
- Live check: with Deepgram selected and no key, Start shows "Add your Deepgram API key in Settings…"; the Settings key field renders as designed. Not checked: a real Deepgram or AssemblyAI session (needs your API key).

### M3 notes + encrypted storage (2026-10-21)

- `store` module: SQLCipher via rusqlite `bundled-sqlcipher` (CommonCrypto on macOS, no OpenSSL). Raw 256-bit key generated with `getrandom` on first launch and kept in the Keychain (`database-key`); Parley refuses to mint a new key if a database already exists. WAL mode, foreign keys, `user_version` migrations.
- Schema v1: `meetings` (title, start, duration, notes JSON + plain text, updated) and `transcript_lines` (speaker, segment, times, text).
- Recording now creates the meeting row on start and saves the transcript to the database on stop. `transcript.json` (plaintext) is gone, and WAVs are deleted after transcription unless Settings > "Keep audio recordings" is on (default off). Capture logs stay; they hold no content.
- TipTap 3.32 editor: title + rich-text notes beside the transcript, autosaved 800 ms after the last edit; saves never overlap and flush on unmount.
- Tests: 53 Rust (incl. encryption at rest: no SQLite header and note text absent from db + WAL bytes; wrong key rejected; transcript replace) + 19 Vitest (incl. autosaver timing).
- Verified on this Mac: first launch created the Keychain key and an encrypted `parley.db` (random header bytes).
- In-app check (after re-granting permissions for the new build): typed notes autosaved ("Saved"), the played sentence came back from the database as one "Them" line with no echo duplicate, both recordings kept only their .log files (WAVs deleted), and the db + WAL bytes contain neither the notes ("Priya", "Budget review") nor the transcript ("revenue numbers").

### M4 AI-enhanced notes (2026-10-21)

- Templates: General meeting, 1:1, Sales call, Standup, plus custom templates (create, copy a built-in, edit, delete) stored in the encrypted database. Built-ins are read-only.
- Prompt: the user's notes are the outline and the transcript supplies detail; nothing invented; empty sections omitted. Transcript lines carry time and speaker; long transcripts keep the first 40% and last 60% of the budget with an omission marker. Budgets: 24k chars for Ollama, 200k OpenAI-compatible, 600k Anthropic.
- Providers (all streaming): Ollama `/api/chat` (default, local), OpenAI-compatible `/chat/completions`, Anthropic Messages API (raw HTTPS since Rust has no official SDK; `claude-opus-5-5`, effort medium, 64k max_tokens, server-side refusal fallbacks via `fallbacks: "default"` + `server-side-fallback-2026-07-01`). Keys for OpenAI and Anthropic live in the Keychain.
- `enhance_meeting` streams `enhance-delta` events, saves Markdown to the meeting (migration v2, upgrades v1 in place), supports cancel. Output is rendered with marked and sanitized with DOMPurify.
- Fixes found during testing: the database key read moved off the main thread (a pending Keychain prompt left the window blank); Settings no longer unmounts an in-progress recording; header labels no longer wrap.
- Tests: 70 Rust (incl. end-to-end streaming for all three providers against a local fake HTTP server, request headers and body checked; v1 to v2 migration) + 21 Vitest.
- Live check with a local fake Ollama server: recorded a meeting, typed notes, clicked Enhance; text streamed into the Enhanced tab as rendered Markdown and was saved. The captured request held the edited title, the template, the flushed notes and the timestamped transcript line. Custom template create and delete worked in Settings.
- Not checked live: real Ollama, OpenAI or Anthropic responses (no local model or keys on this Mac).
- Dev signing: a local "Parley Dev" identity keeps microphone and system-audio permission across rebuilds (mic ready in 560 ms after two signed rebuilds, no prompt). The Keychain still asked once per rebuild during testing; choose Always Allow.

### M5 library, search, ask, export (2026-10-21)

- Store migration v3: FTS5 index (bundled SQLCipher has FTS5) over title, notes, enhanced notes and transcript, rebuilt per meeting on every write and backfilled for existing meetings. User input becomes quoted prefix terms, so operators and punctuation can't break a query. bm25 ranking weights title over notes over enhanced over transcript. Snippets carry highlight markers.
- Library: searchable meeting list (newest first, "Recording now" row, Enhanced badge) beside the recorder or a meeting's detail view. Stopping a recording opens the saved meeting in the detail view, so only one editor ever autosaves a meeting. Two-step delete removes the database rows, search entry and recording folder.
- Ask: keywords minus stopwords, any-term search with a recency fallback, up to 6 meetings; per meeting the enhanced notes (or the user's notes) plus transcript lines that mention a keyword with one line of context; numbered citations; streaming answer with clickable sources.
- Export: TipTap JSON to Markdown (headings, nested lists, task lists, marks, links, quotes, code), saved through the native Save dialog (tauri-plugin-dialog 2.8.3). PDF: a print-only document rendered outside the app shell, then the macOS print sheet's Save as PDF.
- Fixes from live testing: views reload on a `store-ready` event once the database unlocks; ref guards stop double clicks from sending Ask or Enhance twice; detail header shows the title; transcript stacks under notes in narrow panes; search box has no autocorrect; the print document stays mounted because the print sheet renders its preview after the print call returns (it printed a blank page before).
- Tests: 85 Rust (FTS search, safe queries, index upkeep, delete, v3 migration, ask prompt and retrieval helpers, local dates, Markdown export) + 27 Vitest.
- Live check (signed build): list loads after unlock; "priya" finds the two meetings with highlighted snippets; detail view and narrow layout; Save dialog opens with the title as file name (cancelled, Documents is iCloud-synced); PDF preview shows title, date, notes, enhanced notes and transcript with no app UI; Ask over a local fake Ollama streamed an answer citing the two budget meetings, and the captured prompt held numbered meeting blocks with local dates and matching transcript lines; recording, stop, and two-step delete removed the meeting and its folder.
- Not checked live: a real LLM answer for Ask (no local model or key); a completed Markdown file save through the dialog.
- Keychain note: with the self-signed dev identity the Keychain still asks once per rebuild; mic and system-audio permission persist.

### M6 detection, menu bar, hotkey, onboarding (2026-10-21)

- Meeting detection: `parley-micwatch` (Swift sidecar) prints the bundle IDs of processes with live mic input once a second from the Core Audio process list; it never opens an input stream. `detect::Detector` reports a meeting app (Zoom, Teams, Slack, FaceTime, Webex, Discord, WhatsApp, major browsers) after 3 s of mic use, once per session, and stays quiet while Parley records. A detection shows a notification and an in-app banner with Start recording. Settings > "Notify me when a meeting starts".
- Menu bar: template tray icon with Start/Stop recording, Show Parley and Quit; title reads "REC" while recording. Closing the window hides it, and the Dock icon reopens it.
- Global hotkey ⌥⇧⌘R toggles recording from any app; Settings can turn it off (registered or unregistered live). Tray, hotkey and banner all emit one `toggle-recording` event the recorder handles.
- Onboarding on first launch: Welcome, Transcription (model download), AI notes (provider choice), Shortcuts (hotkey and detection toggle). Skippable; finishing saves `onboarded`.
- Tests: 91 Rust (detector settle, once-per-session, re-arm, suppression, line parsing, settings defaults) + 27 Vitest.
- Live check (signed build): with `onboarded` reset, the first launch opened onboarding at the Welcome step; it was finished in the app and `onboarded: true` persisted. A recording then started outside the window (menu bar or hotkey), with the "Recording now" row, header and live transcript. `parley-micwatch` run against the live session listed `com.eshaanjain.parley` and `com.apple.CoreSpeech`, which are not meeting apps, so no notification fired, as intended; it exited on stdin EOF.
- Not checked live: the REC title and menu items (full-screen capture was not available), a real meeting app holding the mic producing the notification and banner, and the Settings hotkey toggle. Detection logic is covered by unit tests.
- Keychain: relaunching the same signed build asked again once; choose Always Allow.
