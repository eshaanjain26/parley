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
  openaiBaseUrl: string;
  openaiModel: string;
  anthropicModel: string;
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

/** Keychain accounts for API keys; mirrors KEY_ACCOUNTS in src-tauri/src/lib.rs. */
export type KeyAccount = "deepgram" | "assemblyai" | "openai" | "anthropic";
export type ApiKeyStatus = Partial<Record<KeyAccount, boolean>>;

export const KEY_LABELS: Record<KeyAccount, string> = {
  deepgram: "Deepgram",
  assemblyai: "AssemblyAI",
  openai: "OpenAI-compatible",
  anthropic: "Anthropic",
};

/** Commands fail with this prefix (plus the account) when a needed API key is missing. */
export const KEY_MISSING = "api-key-missing:";

export const apiKeyStatus = () => invoke<ApiKeyStatus>("api_key_status");
export const setApiKey = (account: KeyAccount, key: string) => invoke<void>("set_api_key", { account, key });
export const deleteApiKey = (account: KeyAccount) => invoke<void>("delete_api_key", { account });

export function speechKeyAccount(provider: SpeechProvider): KeyAccount | null {
  if (provider === "deepgram") return "deepgram";
  if (provider === "assembly-ai") return "assemblyai";
  return null;
}

export function llmKeyAccount(provider: LlmProvider): KeyAccount | null {
  if (provider === "open-ai-compatible") return "openai";
  if (provider === "anthropic") return "anthropic";
  return null;
}

/** Turns a KEY_MISSING error into a sentence for the user, or returns null for other errors. */
export function missingKeyMessage(error: string): string | null {
  if (!error.startsWith(KEY_MISSING)) return null;
  const account = error.slice(KEY_MISSING.length) as KeyAccount;
  const label = KEY_LABELS[account] ?? account;
  return `Add your ${label} API key in Settings first.`;
}
