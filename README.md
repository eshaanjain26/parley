<p align="center"><img src="assets/brand/parley-icon-1024.png" width="96" alt="Parley icon"></p>

# Parley

Open-source AI meeting notes for macOS. Parley records your mic and the call audio on your Mac, writes a live transcript, and turns your rough notes into clean ones.

By default, everything runs on your Mac: whisper.cpp for transcription and Ollama for summaries. If you want cloud speech or AI, add your own Deepgram, AssemblyAI, OpenAI, or Anthropic key. Keys are stored in the macOS Keychain.

> Status: early development. Recording, transcription, and AI notes are being built milestone by milestone; see [`tasks/todo.md`](tasks/todo.md).

## Requirements

- macOS 14.4 or later on Apple Silicon
- Node 22+, Rust (stable), cmake

## Develop

```bash
npm install
npm run tauri dev
```

### Signing dev builds

Unsigned builds get a new identity every time, so macOS asks again for microphone, system audio and Keychain access after each rebuild. Create a local code-signing certificate once (Keychain Access > Certificate Assistant > Create a Certificate, type "Code Signing", then set it to Always Trust for code signing) and build with it:

```bash
APPLE_SIGNING_IDENTITY="Parley Dev" npm run tauri build -- --debug --bundles app
```

## Test

```bash
cd src-tauri && cargo test
```

## Build

```bash
npm run tauri build
```

The unsigned app lands in `src-tauri/target/release/bundle/macos/Parley.app`.

## License

MIT. See [LICENSE](LICENSE).
