import { describe, expect, it } from "vitest";
import { llmKeyAccount, missingKeyMessage, speechKeyAccount } from "./settings";

describe("key accounts", () => {
  it("maps speech providers to Keychain accounts; on-device needs none", () => {
    expect(speechKeyAccount("local-whisper")).toBeNull();
    expect(speechKeyAccount("deepgram")).toBe("deepgram");
    expect(speechKeyAccount("assembly-ai")).toBe("assemblyai");
  });

  it("maps AI providers to Keychain accounts; Ollama needs none", () => {
    expect(llmKeyAccount("ollama")).toBeNull();
    expect(llmKeyAccount("open-ai-compatible")).toBe("openai");
    expect(llmKeyAccount("anthropic")).toBe("anthropic");
  });
});

describe("missingKeyMessage", () => {
  it("names the provider whose key is missing", () => {
    expect(missingKeyMessage("api-key-missing:anthropic")).toBe("Add your Anthropic API key in Settings first.");
    expect(missingKeyMessage("api-key-missing:assemblyai")).toBe("Add your AssemblyAI API key in Settings first.");
  });

  it("ignores other errors", () => {
    expect(missingKeyMessage("Couldn't reach Ollama")).toBeNull();
  });
});
