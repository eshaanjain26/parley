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
- [x] **M7 Release**: Developer ID signing, notarization, DMG, signed auto-updater on GitHub Releases, landing README, privacy policy.

## M7 plan (2026-10-21)

Decisions: no Apple Developer membership yet, so signing and notarization switch on from GitHub secrets when they exist and releases ship ad-hoc signed until then. The main repo goes public (MIT) and hosts releases plus the update feed.

- [x] **Updater in the app.** tauri-plugin-updater + tauri-plugin-process. Feed: `releases/latest/download/latest.json` on this repo, verified with a minisign public key in `tauri.conf.json`. Quiet check on launch (Settings toggle, default on) and a "Check for updates" button; a banner offers "Restart to update" once downloaded. A local feed is tested by building with a `--config` overlay (test key, http endpoint); no test hooks ship in the app.
- [x] **Updater signing key.** You generate it (`npx tauri signer generate`) so you own the password; I read only the `.pub` file. You add the private key and password as GitHub secrets with `gh secret set` (I give you the commands; I don't handle the key).
- [x] **Release workflow.** On a `v*` tag: matrix build on macos-14 for `aarch64-apple-darwin` and `x86_64-apple-darwin` (build.rs already compiles sidecars per arch), tauri-action creates a draft release with both DMGs, the updater `.app.tar.gz` + `.sig`, and a merged `latest.json`. Fails if the tag doesn't match the app version.
- [x] **Signing switch.** With `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` set: Developer ID signing, hardened runtime, notarization and stapling. Without them: ad-hoc signing (`-`). Documented trade-off: ad-hoc builds lose mic/system-audio permission on every update.
- [x] **Version script.** `npm run version -- 0.2.0` updates package.json, Cargo.toml and tauri.conf.json together; unit-tested.
- [x] **Docs.** Landing README (what it does, screenshot, install incl. right-click > Open for unsigned builds, privacy summary, build from source), `PRIVACY.md` (what stays on the Mac, what goes to each optional provider, no telemetry), `SECURITY.md`, `CHANGELOG.md`.
- [x] **Pre-public audit.** Scan full git history for secrets and personal data, confirm no third-party product assets, check LICENSE and third-party notices (whisper.cpp MIT, model download licenses).
- [x] **Verify.** Local release DMG build; updater end to end on this Mac: install 0.1.0, serve a signed 0.1.1 from a local feed, update and relaunch. CI green on the PR.
- [x] **Go public + first release.** Only after you confirm: flip the repo to public, tag `v0.1.0`, publish the draft release.

## UI refresh plan (2026-10-21)

**Brief.** Domain: personal productivity, meeting notes on a Mac. Audience: busy professionals, mostly non-technical, using it several times a day. Mood to earn: *calm*. Motion: subtle feedback only. Layout families: index list grouped by day (Home), editorial single column (meeting), split (recording), form sections (Settings). Reference anchor: the `warm-editorial` library system, adjusted for contrast. Clean-room: the user's reference screenshots inform layout patterns only; no third-party names, assets, fonts or copy.

**Tokens** (in `App.css` custom properties, no hardcoded values in components)
- Light: paper `#FAF7F2`, raised surface `#FFFDF9`, sidebar `#F2EDE4`, ink `#1C1A17`, muted `#6B635C` (5.0:1), hairline `#E4DDD1`, accent terracotta `#B04A2B` (5.1:1 both ways; the spec's `#C0512F` fails AA at 4.1:1).
- Dark (designed, not inverted): `#1B1916` / `#24211D` / ink `#EDE7DD` (14.3:1) / muted `#A79E94` (6.7:1) / accent `#E58A63` with dark label (6.8:1).
- Type: display in the system serif (`ui-serif`, New York on macOS; works offline under the strict CSP), UI in the system sans. Scale 12/13/15/20/28/40. Radius 8/12/16, pill for chips. Elevation: flat + one raised level for menus and the Ask bar.
- Icons: lucide (`lucide-react`, a version at least 2 weeks old), no emoji.

**Layout and UX** (the "better" part, beyond the references)
- [ ] Sidebar with icons: Home, Ask, Settings; a live **recording pill** (red dot, timer, Stop) visible from every view; update notice; "Encrypted on this Mac" privacy cue in place of upsell chrome.
- [ ] **Home**: serif date header, a primary "Start recording" card with the ⌥⇧⌘R hint, the meeting-detected prompt inline, then meetings **grouped by day** (Today, Yesterday, weekday dates) as a calm list: icon, title, time and duration, "AI notes" chip. Search at the top filters in place with highlighted snippets. Friendly empty state.
- [ ] **Meeting page** (replaces the split list+detail): back button, serif title, chip row (date, duration, template), segmented My notes / AI notes, template menu with lucide icons plus Regenerate, transcript in a collapsible side panel, Export / PDF / Delete in a "More" menu.
- [ ] **Recording page**: big timer and meters, notes left, live transcript right; stopping lands on the meeting page.
- [ ] **Persistent "Ask your meetings" bar** at the bottom of Home and meeting pages with 1 suggestion chip; submitting opens Ask with the question running.
- [ ] Keyboard: ⌘N start/stop recording, ⌘K search, ⌘J Ask, Esc closes menus; full focus rings; menus are keyboard-operable (arrow keys, roving focus).
- [ ] Settings and onboarding restyled into sectioned cards with icons; no behavior change.
- [ ] Motion: 8px fade-up for list groups (staggered under 300ms), hover tint, press scale 0.98, menu scale-in 0.96 to 1; all removed under `prefers-reduced-motion`.

**Verify**
- [ ] Vitest for the day-grouping and relative-date helpers; existing 33 frontend + 93 Rust tests stay green.
- [ ] Contrast script on every token pair, light and dark.
- [ ] Live check in the signed dev build: Home, search, meeting page, menus, recording, Ask bar, Settings, onboarding; light and dark screenshots; keyboard-only pass.
- [ ] Pre-flight aesthetic checklist; PR with before/after screenshots.

## Folders, calendar and meeting apps plan (2026-10-22)

Builds on the UI refresh (PR #17). Three PRs in this order, each verified in the app before the next.

### 1. Folders (PR A)
- Store migration v4: `folders` (id, name, created) and a nullable `meetings.folder_id`. One folder per meeting, which matches how people file calls (by client, team or project); deleting a folder moves its meetings back to "No folder" and never deletes a meeting.
- Sidebar "Folders" section with counts; create inline, rename, delete from a menu.
- Folder page: the Home list filtered to the folder, with search inside it.
- Meeting page: a folder chip in the meta row; its menu moves the meeting or creates a folder on the spot.
- Ask scoped to a folder: `ask_meetings` takes an optional folder id, so the Ask bar on a folder page answers from that folder only ("Ask about Acme").
- Tests: migration from v3, folder CRUD, delete keeps meetings, folder-scoped search and Ask retrieval.

### 2. Calendar (PR B)
- Source: macOS Calendar through EventKit. It already syncs the Google, Outlook/Exchange and iCloud calendars added to the Mac, so one local integration covers all of them with no cloud sign-in and nothing new leaving the Mac. Direct Google/Microsoft OAuth is out of scope: it needs a hosted OAuth client and Google's restricted-scope verification, and duplicates what macOS Calendar already provides.
- New Swift helper `parley-calendar` (same pattern as the audio helpers): requests full calendar access, lists events for a window as JSON, and reports calendar changes. Needs `NSCalendarsFullAccessUsageDescription` in Info.plist and the `com.apple.security.personal-information.calendars` entitlement for the hardened runtime.
- Home "Coming up": today and tomorrow, with time, title, attendees count and a provider icon. Past days stay the meeting list.
- Starting a recording during or just before an event uses the event's title and saves its attendees and event id (migration v5) on the meeting. Recordings from a recurring event go into the folder the last one was filed in, so a weekly 1:1 files itself.
- Optional reminder notification 1 minute before an event with a call link. (Moved to PR C: it depends on call-link detection.)
- Settings > Calendar: connect (permission), choose which calendars count, reminders on or off.
- Tests: event JSON parsing, event-to-recording matching (overlap window), recurring-series folder memory.

### 3. Zoom, Teams, Google Meet and Webex (PR C)
- Local integration (recommended): read the call link from each event's URL, location and notes (zoom.us/j, teams.microsoft.com/l/meetup-join, meet.google.com, webex.com), and show a **Join** button that opens the call in its app or browser and starts Parley's recording in one click.
- Auto-start option: when a calendar event with a call link is in progress and its app takes the microphone (the existing detection), start recording automatically instead of only prompting. Auto-stop when the call app releases the microphone for 30 s, with a "Keep recording" escape. Off by default.
- Out of scope, and why: the vendors' cloud APIs (Zoom, Microsoft Graph, Google Meet) need hosted OAuth apps, admin consent in most companies and a server, and their recording features rely on a bot joining the call. That contradicts Parley's design (local, no bot). Reconsider only for pulling the vendors' own cloud transcripts later.
- Tests: link extraction across real-world invite formats (Zoom personal and meeting IDs, Teams safelinks, Meet codes, Webex), auto-start and auto-stop state machine.

### Docs
- PRIVACY.md: calendar data is read on the Mac and stored only as title, time, attendees and event id on recorded meetings; nothing is uploaded.

## Next plan: brand, accounts, templates, speakers (2026-10-22)

Decisions (from the user): file key in dev builds only; Mac Calendar accounts plus direct Google and Microsoft sign-in; local plus cloud speaker separation; "Ink and ember" brand. One PR per milestone, each checked in the app before merging.

### M8: No more Keychain prompts in dev builds (done, PR #22; verified: one migration prompt, then no prompts after rebuilds)
- Root cause: self-signed builds have no Team ID, so macOS scopes each Keychain approval to the binary's hash and every rebuild asks again. Developer ID releases don't have this.
- Debug builds keep the database key and API keys in `~/Library/Application Support/com.eshaanjain.parley/dev-secrets.json` (mode 0600). Release builds keep the Keychain. On first run, a debug build migrates the existing Keychain key once (one last prompt) so the encrypted database stays readable.
- Tests: file mode, round trip, migration path, release path untouched. Docs: README dev notes, PRIVACY.md (dev builds only).

### M9: Ink and ember brand (done, PR #23)
- Palette: deep ink navy surfaces and text, ember accent, warm neutrals; light, dark and "follow system"; every pair AA-checked. An accent choice in Settings (ember, ocean, forest, plum) re-points one token.
- Type: Inter for UI and a modern serif for titles (Fraunces or Instrument Serif), both SIL OFL, bundled locally (the CSP blocks remote fonts) and listed in THIRD-PARTY-NOTICES.
- Logo: a new monogram mark (speech-bubble P) as SVG, then app icon, tray template and favicon regenerated with `tauri icon`.
- Interaction: ⌘K command palette (go to, search meetings, start recording, new folder, switch theme); hover previews of AI notes on Home rows; empty-state illustrations; toast feedback for saves and moves; keyboard hints throughout.
- Pre-flight aesthetic check and contrast script; before and after screenshots.

### M10: More AI note templates
- New built-ins with icons: Customer discovery, Product demo, Customer check-in, Interview (hiring), Project kickoff, Retrospective, Weekly team meeting, Exec or board update, Brainstorm, Troubleshooting, Partner or negotiation, Lecture or learning session.
- Template gallery in Settings with a preview of each structure; "copy and edit" for any built-in.
- Suggested template per meeting from the event title and attendees (for example "1:1", "standup", "interview", external domains suggest a customer template); one click to accept.
- Tests: every template's prompt builds, suggestion rules.

### M11: Calendar accounts (Google and Microsoft sign-in)
- Settings > Calendar shows Accounts: the Mac's Calendar (as today) plus any number of Google and Microsoft accounts signed in from Parley. Each account can be turned off or removed; calendars are chosen per account.
- OAuth 2.0 with PKCE and a loopback redirect (the desktop-app flow); read-only scopes (Google `calendar.readonly`, Microsoft `Calendars.Read`); tokens stored like other secrets (file in dev, Keychain in release); refresh handled in the background.
- Events from all sources are merged and de-duplicated by iCalUID, so an account added both on the Mac and in Parley appears once.
- Needs from the user: a Google Cloud OAuth client (Desktop app) and a Microsoft Entra app registration (public client). For public release, Google verification of the sensitive `calendar.readonly` scope.
- Production users: each person signs in to their own accounts; no Parley server holds tokens.

### M12: Who said what
- Call audio is split into speakers on the Mac with sherpa-onnx speaker diarization (segmentation + embedding ONNX models, about 40 MB, downloaded and checksummed like the speech model) after recording, before audio is deleted. With Deepgram or AssemblyAI, their built-in diarization is used instead.
- Speakers become "Speaker 1, 2…"; Parley suggests names from calendar attendees and from the transcript ("thanks, Priya"), and the user can confirm or rename them per meeting. Your own microphone stays "You".
- AI notes and Ask use the names, so action items are assigned to the right person.
- Tests: segment-to-line alignment, renaming, prompt includes names; fixture audio with two synthetic voices.

Order: M8, M9, M10, M11, M12 (M11 can start as soon as the OAuth app IDs exist).

## Deferred past v1

Windows, integrations (Slack, Notion, CRM), sync, mobile.

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

### M7 release (2026-10-21)

- Updater (PR #10): tauri-plugin-updater 2.13.2 pinned; feed on this repo's latest release; `requireSignedVersion` on. End to end on this Mac with a throwaway key and a local feed: 0.1.0 found 0.1.1, verified and downloaded it, "Restart to update" replaced the bundle and relaunched; a feed announcing 0.1.2 with the 0.1.1 artifact was rejected as tampered. Real key generated by the owner (ID D9319754045A2C50); private key and password are repo secrets only.
- Release workflow: tag-driven, arm64 + x64 on macos-14, draft release with DMGs, update bundles, signatures and merged `latest.json`; ad-hoc signing until Developer ID secrets exist. Not run yet.
- Pre-public audit over all 31 commits: no keys, tokens, private keys, local paths or personal emails (git grep over every revision; no scanner installed). All Rust and npm licenses permissive. SQLCipher (BSD-3) and whisper.cpp/ggml (MIT) need their notices in binary distributions: `THIRD-PARTY-NOTICES.md` and `LICENSE` now ship in the app's Resources. Test fixture WAV is synthetic (macOS `say`). Icons are original. `tasks/todo.md` names the product this is a clean-room alternative to.
- Docs: landing README (features, privacy summary, install incl. Open Anyway for un-notarized builds, build and release steps), PRIVACY.md (every endpoint, checked against the code), SECURITY.md (private reporting), CHANGELOG.md.
- README screenshot (PR #12): the synthetic Q3 budget sync meeting, captured from the window at full density without the pointer.
- Went public: repo visibility set to public and private vulnerability reporting enabled (both confirmed through the API), after the changelog entry was dated (PR #13).
- First v0.1.0 run compiled both architectures, then failed at signing: a missing `APPLE_CERTIFICATE` secret reaches the job as an empty variable and Tauri tried to import it. Fixed in PR #14 by exporting the Apple variables only when the certificate secret is set. No release had been created, so the tag was moved to the fix.
- v0.1.0 published: DMGs and update bundles for Apple silicon and Intel. Checks on the published release: every binary has the right architecture (arm64 / x86_64, including the 3 Swift helpers); the app is ad-hoc signed with hardened runtime and the audio-input entitlement, passes `codesign --verify --deep --strict`, and Gatekeeper rejects it as expected without notarization (README explains Open Anyway); LICENSE and THIRD-PARTY-NOTICES.md are in Resources. `releases/latest/download/latest.json` answers without authentication with version 0.1.0; both update bundles download and verify against the project key (checked with minisign-verify, the updater's own library), and a tampered bundle fails.
- Not checked: launching the published build (it would share the dev build's data and Keychain item), and an update from 0.1.0 to a later release (needs 0.1.1).
- Feed URLs: tauri-action linked the feed to `api.github.com` asset URLs (the release was a draft when it wrote them), which count against GitHub's 60/hour unauthenticated API limit per IP. A `feed` job now rewrites them to `releases/download/<tag>/<asset>` after both builds (`scripts/feed-urls.mjs`, tested). The published v0.1.0 feed was rewritten the same way: only URLs changed (version, notes, date and signatures identical), and both bundles download from github.com and verify against the project key.
- Follow-up: Developer ID signing and notarization once there's an Apple Developer membership.

### Folders (PR A, 2026-10-22, not yet checked in the app)

- Migration v4: `folders` table and nullable `meetings.folder_id` with `ON DELETE SET NULL`, so deleting a folder unfiles its meetings. Names are trimmed and whitespace-collapsed, 1 to 60 characters, unique ignoring case (checked in the UI and again in the store).
- Store: list, create, rename, delete, file or unfile a meeting; `list_meetings` and `search` take an optional folder; Ask retrieval (search, then recent-meetings fallback) stays inside the folder.
- UI: Folders section in the sidebar with counts and inline create; folder page (Home scoped to the folder: start card records into it, search, day groups, Ask bar "Ask about <folder>", rename and delete with confirmation); folder chip on meeting and recording pages to move, create-and-move or remove; folder names on Home rows; Ask shows its folder scope with a chip to widen it.
- Tests: Rust 97 (5 new: scoping of list, search and counts; name rules; delete keeps meetings; v3 to v4 migration on an existing database; random ids). Vitest 42 (3 new for name checks).
- Not checked in the app: the disk ran out during the app build (356 MB free).

### Calendar (PR B, 2026-10-22, not yet checked in the app)

- `parley-calendar` Swift helper (EventKit, read-only): `status`, `request`, `calendars`, `events FROM TO`. Occurrence ids are `calendarItemIdentifier:startMs`; repeating events carry a `seriesId`. Invite text is capped at 4000 characters and never serialized to the interface or stored.
- Entitlement `com.apple.security.personal-information.calendars` (Tauri applies the app's entitlements to the helpers too, confirmed on the built bundle) and `NSCalendarsFullAccessUsageDescription`.
- Rust `calendar` module: parsing, calendar filter (empty = all), `current_event` (timed events in progress or starting within 10 minutes; nearest start wins for back-to-back meetings). Commands: status, connect (prompts and turns the setting on), calendars, upcoming events, current event, link meeting to event.
- Store v5: `event_id`, `series_id`, `attendees_json` on meetings. Linking a repeating event's new occurrence files it into the folder of its latest filed occurrence, unless the meeting is already filed.
- Recording: any start (button, ⌥⇧⌘R, menu bar, Coming up) looks up the current event (1.5 s cap), uses its title, links attendees and applies series filing.
- UI: Home "Coming up" (today and tomorrow, attendees, starts-in, Record button from 10 minutes before), a dismissible connect card, attendees chip on meetings, Settings > Calendar (connect, on/off, per-calendar checklist grouped by account).
- Build: `[lib] crate-type` is now `rlib` only. The template's staticlib and cdylib are for mobile targets and cost ~330 MB per build on a nearly full disk.
- Tests: Rust 104 (7 new), Vitest 45 (3 new).
- Not checked in the app (disk full): the macOS prompt, real calendar data, the Coming up layout.

### Meeting apps (PR C, 2026-10-22, not yet checked in the app)

- `links`: call-link extraction from an event's URL, then location, then notes. Unwraps Outlook safe links and Google redirects; accepts only https join links on Zoom (`/j/`, `/my/`, `/w/`, incl. zoomgov), Teams (`/l/meetup-join/`, teams.live.com `/meet/`), Meet (`meet.google.com/<code>`) and Webex hosts; rejects look-alike hosts, userinfo, ports, other schemes and non-join pages. Links are computed in Rust; a `callLink` in helper input is ignored.
- `join_call` re-validates the link and opens it with `/usr/bin/open`, then records the event and follows its provider's apps (browsers count for all four).
- `autorec`: reminders 75 s before events with a call link, once each; auto-start when a detected meeting app runs the current event's provider; `CallWatch` stops 30 s after the followed app leaves the mic, resets if it comes back, and ignores absence until the app has joined once.
- Watcher: with "Record calls automatically" on, a matching detection sends a "Recording ..." notification and an `auto-record` event instead of the prompt; the UI starts, then asks the backend to follow that app. A countdown toast offers Keep recording or Stop now. Stopping in any way clears the watch.
- Settings > Calendar: reminders (on by default), automatic recording (off by default, with a note to tell people on the call).
- Tests: Rust 115 (11 new), Vitest 45.
- Not checked in the app (disk full).

### Live check of the UI refresh, folders, calendar and meeting apps (2026-10-22, signed debug build from main)

- Checked in the app: Home (greeting, start card, connect card, day groups, Ask bar); calendar connect (access granted, Coming up shown, Settings lists the Mac's Google, subscribed, other and iCloud calendars through the helper); folder create from the sidebar, folder page, filing a meeting from its chip by keyboard (menu opens with Down, Return picks), sidebar count, folder-scoped search, folder-scoped Ask (answer cites only the folder's meeting, via the configured Anthropic key); recording from Home (recording page, sidebar pill, Home recording card, "Recording now" row, stop from the pill lands on the meeting page); More menu by keyboard, inline delete confirmation, delete.
- Found and fixed: search snippets from AI notes showed raw Markdown (`##`, `**`). Snippets are now plain text with highlights kept.
- Not checked: Join, reminders and automatic start/stop (no calendar events with call links on this Mac's calendars; adding test events would sync to the user's Google calendar), dark mode (needs a system appearance change).

### M9 brand (2026-10-24, checked in the app)

- Light and dark Home, Settings and meeting page; theme and accent apply instantly and survive relaunch; Ocean accent recolors logo, buttons and icons; command palette opens from the sidebar button, searches folders and meetings, keyboard selection opens the result.
- Fixed during the check: native checkboxes and selects stayed light in dark mode (added `color-scheme`); the selected segmented option was hard to see in dark (border and accent icon); serif titles ran words together because WebKit used Fraunces' largest optical size, so optical sizes are now set per size; added a visible "Search or jump to… ⌘K" button so the palette is discoverable.
