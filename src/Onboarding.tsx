import { Keyboard, Lock, MenuSquare, MonitorSpeaker, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { Logo } from "./Logo";
import { ModelDownload } from "./Recorder";
import { Settings, updateSettings } from "./lib/settings";
import { ModelStatus, listModels } from "./lib/transcript";

interface Props {
  settings: Settings;
  hotkey: string;
  onDone: (settings: Settings) => void;
}

const STEPS = ["Welcome", "Transcription", "AI notes", "Shortcuts"] as const;

/** First-run setup. Every step can be skipped; everything here also lives in Settings. */
export function Onboarding({ settings, hotkey, onDone }: Props) {
  const [step, setStep] = useState(0);
  const [draft, setDraft] = useState<Settings>(settings);
  const [models, setModels] = useState<ModelStatus[]>([]);
  const [error, setError] = useState<string | null>(null);

  const refreshModels = () => listModels().then(setModels).catch(() => setModels([]));
  useEffect(() => {
    refreshModels();
  }, []);

  const finish = async () => {
    try {
      onDone(await updateSettings({ ...draft, onboarded: true }));
    } catch (e) {
      setError(String(e));
    }
  };

  const model = models.find((m) => m.id === draft.whisperModel);
  const last = step === STEPS.length - 1;

  return (
    <div className="onboarding">
      <div className="onboarding-card">
        <ol className="onboarding-steps" aria-label="Setup steps">
          {STEPS.map((s, i) => (
            <li key={s} className={i === step ? "current" : i < step ? "done" : ""}>{s}</li>
          ))}
        </ol>

        {step === 0 && (
          <section>
            <Logo size={44} />
            <h1 className="display">Welcome to Parley</h1>
            <p>Parley takes meeting notes on your Mac. It records your microphone and the call audio, writes a live transcript, and turns your rough notes into clean ones.</p>
            <ul className="onboarding-points">
              <li><MonitorSpeaker size={18} aria-hidden />Audio comes from your Mac, so no bot joins your calls.</li>
              <li><Lock size={18} aria-hidden />Transcripts and notes are encrypted on this Mac, with the key in your Keychain.</li>
              <li><Trash2 size={18} aria-hidden />Audio is deleted after transcription unless you choose to keep it.</li>
            </ul>
          </section>
        )}

        {step === 1 && (
          <section>
            <h1 className="display">Transcription on this Mac</h1>
            <p>Parley transcribes with a speech model that runs locally, so audio never leaves your Mac. You can switch to Deepgram or AssemblyAI later in Settings.</p>
            {model && !model.installed ? (
              <ModelDownload model={model} onDone={refreshModels} />
            ) : (
              <p className="ready">{model ? `${model.label} is ready.` : "Checking the speech model…"}</p>
            )}
            <p className="hint">The first recording asks for microphone and system-audio access. Both are needed to hear you and the call.</p>
          </section>
        )}

        {step === 2 && (
          <section>
            <h1 className="display">AI notes</h1>
            <p>After a meeting, Parley turns your rough notes and the transcript into clean, structured notes. Choose where that runs:</p>
            <div className="choices">
              {[
                { value: "ollama", title: "On this Mac with Ollama", body: "Free and private. Needs Ollama installed with a model pulled." },
                { value: "anthropic", title: "Claude, with your Anthropic key", body: "Best quality. Notes and transcript are sent to Anthropic." },
                { value: "open-ai-compatible", title: "An OpenAI-compatible API", body: "Use your own key and model. Notes and transcript are sent to that provider." },
              ].map((c) => (
                <label key={c.value} className={`choice${draft.llmProvider === c.value ? " selected" : ""}`}>
                  <input
                    type="radio"
                    name="llm"
                    checked={draft.llmProvider === c.value}
                    onChange={() => setDraft({ ...draft, llmProvider: c.value as Settings["llmProvider"] })}
                  />
                  <span>
                    <strong>{c.title}</strong>
                    <span className="hint">{c.body}</span>
                  </span>
                </label>
              ))}
            </div>
            {draft.llmProvider !== "ollama" && <p className="hint">Add your API key in Settings after setup. It's stored in the macOS Keychain.</p>}
          </section>
        )}

        {step === 3 && (
          <section>
            <h1 className="display">Always one step away</h1>
            <ul className="onboarding-points">
              <li>
                <Keyboard size={18} aria-hidden />
                <span>Press <kbd>{hotkey}</kbd> from any app to start or stop recording.</span>
              </li>
              <li><MenuSquare size={18} aria-hidden />Parley lives in the menu bar. Closing the window keeps it running there.</li>
            </ul>
            <label className="check">
              <input type="checkbox" checked={draft.detectMeetings} onChange={(e) => setDraft({ ...draft, detectMeetings: e.target.checked })} />
              <span>
                Notify me when a meeting starts
                <span className="hint">Parley checks which apps use the microphone (Zoom, Teams, Meet and others). It doesn't listen until you record.</span>
              </span>
            </label>
          </section>
        )}

        {error && <p className="error" role="alert">{error}</p>}
        <div className="onboarding-nav">
          <button onClick={finish} className="link">Skip setup</button>
          <span className="spacer" />
          {step > 0 && <button onClick={() => setStep(step - 1)}>Back</button>}
          <button className="primary" onClick={last ? finish : () => setStep(step + 1)}>
            {last ? "Start using Parley" : "Continue"}
          </button>
        </div>
      </div>
    </div>
  );
}
