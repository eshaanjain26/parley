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
  keepAudio: boolean;
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

export interface ApiKeyStatus {
  deepgram: boolean;
  assemblyAi: boolean;
}

/** start_recording fails with this prefix (plus "deepgram" or "assemblyai") when a cloud key is missing. */
export const KEY_MISSING = "api-key-missing:";

export const apiKeyStatus = () => invoke<ApiKeyStatus>("api_key_status");
export const setApiKey = (provider: SpeechProvider, key: string) => invoke<void>("set_api_key", { provider, key });
export const deleteApiKey = (provider: SpeechProvider) => invoke<void>("delete_api_key", { provider });

/** Whether `status` has a key for `provider`; on-device engines never need one. */
export function hasKeyFor(status: ApiKeyStatus | null, provider: SpeechProvider): boolean {
  if (provider === "local-whisper") return true;
  if (!status) return false;
  return provider === "deepgram" ? status.deepgram : status.assemblyAi;
}

