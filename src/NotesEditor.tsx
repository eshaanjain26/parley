import { EditorContent, useEditor } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import { Placeholder } from "@tiptap/extensions";
import { useEffect, useMemo, useRef, useState } from "react";
import { NotesUpdate, createAutosaver, saveNotes } from "./lib/meetings";

interface Props {
  meetingId: string;
  initialTitle: string;
  initialJson: string;
  /** Receives a function that saves any pending edit immediately. */
  onFlushReady?: (flush: () => Promise<void>) => void;
}

type SaveState = "saved" | "saving" | "error";

/** Title plus rich-text notes, autosaved to the encrypted store 800 ms after the last edit. */
export function NotesEditor({ meetingId, initialTitle, initialJson, onFlushReady }: Props) {
  const [title, setTitle] = useState(initialTitle);
  const [saveState, setSaveState] = useState<SaveState>("saved");
  const titleRef = useRef(initialTitle);

  const saver = useMemo(
    () =>
      createAutosaver(
        async (update: NotesUpdate) => {
          setSaveState("saving");
          await saveNotes(meetingId, update);
          setSaveState("saved");
        },
        800,
        () => setSaveState("error"),
      ),
    [meetingId],
  );

  const editor = useEditor({
    extensions: [
      StarterKit,
      Placeholder.configure({ placeholder: "Jot down what matters. Parley keeps the full transcript alongside." }),
    ],
    content: initialJson ? JSON.parse(initialJson) : "",
    onUpdate: () => schedule(),
  });

  const schedule = () => {
    if (!editor) return;
    saver.schedule({
      title: titleRef.current.trim() || initialTitle,
      notesJson: JSON.stringify(editor.getJSON()),
      notesText: editor.getText({ blockSeparator: "\n" }),
    });
  };

  useEffect(() => {
    onFlushReady?.(saver.flush);
  }, [saver, onFlushReady]);

  // Leaving the meeting (or the app view) must not drop the last few keystrokes.
  useEffect(
    () => () => {
      void saver.flush();
    },
    [saver],
  );

  return (
    <div className="notes">
      <div className="notes-head">
        <input
          className="notes-title"
          aria-label="Meeting title"
          value={title}
          onChange={(e) => {
            setTitle(e.target.value);
            titleRef.current = e.target.value;
            schedule();
          }}
        />
        <span className={`save-state save-${saveState}`} role="status">
          {saveState === "saving" ? "Saving…" : saveState === "error" ? "Couldn't save" : "Saved"}
        </span>
      </div>
      <EditorContent editor={editor} className="notes-body" />
    </div>
  );
}
