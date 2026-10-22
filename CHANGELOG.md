# Changelog

All notable changes to Parley. Versions follow [Semantic Versioning](https://semver.org).

## 0.1.0 (unreleased)

First public release.

- Records your microphone and the call audio (Core Audio process tap) with macOS echo cancellation.
- Live transcript on device with whisper.cpp (Metal), or streaming with your own Deepgram or AssemblyAI key.
- Notes editor beside the transcript, autosaved to a SQLCipher-encrypted database with its key in the Keychain. Audio is deleted after transcription by default.
- Enhance: rewrites your notes with transcript detail through templates (general, 1:1, sales call, standup, custom), using Ollama on device or your own OpenAI-compatible or Anthropic key.
- Meeting library with full-text search, Ask across meetings with cited sources, Markdown and PDF export.
- Menu-bar item, ⌥⇧⌘R global shortcut, meeting-app detection notifications and first-run setup.
- Signed in-app updates from GitHub Releases.
