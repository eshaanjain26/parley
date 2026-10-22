# Privacy policy

Last updated: 22 October 2026

Parley is a desktop app that runs on your Mac. There is no Parley server and no Parley account. The project collects no data about you.

This page lists everything Parley stores and every place it can send data, so you can choose providers knowingly.

## What stays on your Mac

| Data | Where | Protection |
|---|---|---|
| Transcripts, notes, enhanced notes, templates | `~/Library/Application Support/com.eshaanjain.parley/parley.db` | SQLCipher (AES-256) encryption. The key is random, generated on first launch and stored in your login Keychain. |
| Audio recordings | Same folder, `recordings/`, one subfolder per meeting | Deleted once the transcript is saved. Kept only if you turn on "Keep audio recordings". Not encrypted while they exist. |
| API keys | macOS Keychain | Never written to disk by Parley; the interface only learns whether a key is set. |
| Settings | `~/Library/Application Support/com.eshaanjain.parley/settings.json` | Plain JSON. Holds choices like provider and model names, never keys or content. |
| Speech models | Same folder, `models/` | Public model files. |
| Calendar link on a recording | Inside the encrypted database: the event's id and the names and email addresses of the people invited | Encrypted with everything else. Only for meetings you record. |

If you connect your calendar, Parley reads events from the macOS Calendar on this Mac (which includes Google, Outlook and iCloud accounts added to the Mac) to show upcoming meetings and to name recordings. It reads; it never changes events, and it doesn't contact any calendar server itself. Invite text is scanned on the Mac for a Zoom, Teams, Meet or Webex link and then discarded; Join opens that link in its own app or browser. You can turn calendar access off in Settings, or for good in System Settings > Privacy & Security > Calendars.

Meeting detection reads the list of apps that are using the microphone from macOS. It doesn't record or listen to anything; recording starts only when you start it.

## When data leaves your Mac

Only in these cases:

| When | Sent to | What is sent |
|---|---|---|
| You download a speech model | Hugging Face (`huggingface.co`) | A request for the model file. |
| Parley checks for updates (on launch, can be turned off; or when you click Check for updates) | GitHub (`github.com`) | A request for the latest release information and, if there is one, the update file. Like any web request, it includes your IP address. |
| You choose Deepgram for transcription | Deepgram (`api.deepgram.com`) | Your meeting audio, streamed while recording, with your Deepgram key. |
| You choose AssemblyAI for transcription | AssemblyAI (`streaming.assemblyai.com`) | Your meeting audio, streamed while recording, with your AssemblyAI key. |
| You use Enhance or Ask with an OpenAI-compatible provider | The base URL you set (default `api.openai.com`) | Your notes, transcript excerpts and template or question, with your key. |
| You use Enhance or Ask with Anthropic | Anthropic (`api.anthropic.com`) | Your notes, transcript excerpts and template or question, with your key. |
| You use Enhance or Ask with Ollama | The Ollama URL you set (default `127.0.0.1`, this Mac) | Your notes, transcript excerpts and template or question. Leaves your Mac only if you point the URL at another machine. |

Each provider handles that data under its own terms and privacy policy. With the defaults (whisper.cpp and Ollama on this Mac), meeting content never leaves your computer.

Parley has no analytics, crash reporting, advertising or tracking of any kind.

## Deleting your data

- Delete a meeting in the app to remove its database rows, search entry and recording folder.
- To remove everything, quit Parley, delete `~/Library/Application Support/com.eshaanjain.parley`, `~/Library/Caches/com.eshaanjain.parley` and `~/Library/WebKit/com.eshaanjain.parley`, and remove the "com.eshaanjain.parley" items from Keychain Access.

## Changes

Changes to this policy are recorded in this file's history on GitHub.

## Contact

Questions about privacy: open an issue at https://github.com/eshaanjain26/parley/issues.
