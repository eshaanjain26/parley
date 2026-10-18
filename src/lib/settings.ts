import { invoke } from "@tauri-apps/api/core";

// Mirrors src-tauri/src/settings.rs. The Rust test `wire_format_matches_typescript_types` guards the shape.
export type SpeechProvider = "local-whisper" | "deepgram" | "assembly-ai";
export type LlmProvider = "ollama" | "open-ai-compatible" | "anthropic";

export interface Settings {
  speechProvider: SpeechProvider;
  whisperModel: string;
  language: string;
  llmProvider: LlmProvider;
  ollamaUrl: string;
  ollamaModel: string;
}

export interface AppInfo {
  name: string;
  version: string;
}

export const SPEECH_PROVIDERS: { value: SpeechProvider; label: string }[] = [
  { value: "local-whisper", label: "On this Mac (whisper.cpp)" },
  { value: "deepgram", label: "Deepgram (your API key)" },
  { value: "assembly-ai", label: "AssemblyAI (your API key)" },
];

export const LLM_PROVIDERS: { value: LlmProvider; label: string }[] = [
  { value: "ollama", label: "On this Mac (Ollama)" },
  { value: "open-ai-compatible", label: "OpenAI-compatible (your API key)" },
  { value: "anthropic", label: "Anthropic (your API key)" },
];

export const getAppInfo = () => invoke<AppInfo>("app_info");
export const getSettings = () => invoke<Settings>("get_settings");
export const updateSettings = (settings: Settings) =>
  invoke<Settings>("update_settings", { settings });
