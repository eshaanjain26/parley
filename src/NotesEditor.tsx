import { EditorContent, useEditor } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import { Placeholder } from "@tiptap/extensions";
import { ReactNode, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { NotesUpdate, createAutosaver, saveNotes } from "./lib/meetings";

interface Props {
  meetingId: string;
  initialTitle: string;
  initialJson: string;
  /** Receives a function that saves any pending edit immediately. */
  onFlushReady?: (flush: () => Promise<void>) => void;
  /** Called after each successful save (e.g. to refresh the meeting list's title). */
  onSaved?: () => void;
  /** Rendered between the title and the notes (meta chips, tabs). */
  toolbar?: ReactNode;
  /** Keeps the editor mounted (so autosave keeps working) while another tab is shown. */
  hideBody?: boolean;
}

type SaveState = "saved" | "saving" | "error";

/** Title plus rich-text notes, autosaved to the encrypted store 800 ms after the last edit. */
export function NotesEditor({ meetingId, initialTitle, initialJson, onFlushReady, onSaved, toolbar, hideBody }: Props) {
  const [title, setTitle] = useState(initialTitle);
  const [saveState, setSaveState] = useState<SaveState>("saved");
  const titleRef = useRef(initialTitle);
  const titleBox = useRef<HTMLTextAreaElement>(null);
  // Long titles wrap; grow the field to fit instead of scrolling inside it.
  useLayoutEffect(() => {
    const el = titleBox.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${el.scrollHeight}px`;
  }, [title]);
  const onSavedRef = useRef(onSaved);
  onSavedRef.current = onSaved;

  const saver = useMemo(
    () =>
      createAutosaver(
        async (update: NotesUpdate) => {
          setSaveState("saving");
          await saveNotes(meetingId, update);
          setSaveState("saved");
          onSavedRef.current?.();
        },
        800,
        () => setSaveState("error"),
      ),
    [meetingId],
  );

  const editor = useEditor({
    extensions: [
      StarterKit,
      Placeholder.configure({ placeholder: "Write rough notes here. Parley keeps the full transcript alongside." }),
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
        <textarea
          ref={titleBox}
          className="notes-title display"
          aria-label="Meeting title"
          rows={1}
          value={title}
          onChange={(e) => {
            setTitle(e.target.value.replace(/\n/g, " "));
            titleRef.current = e.target.value.replace(/\n/g, " ");
            schedule();
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              editor?.commands.focus("start");
            }
          }}
        />
        <span className={`save-state save-${saveState}`} role="status">
          {saveState === "saving" ? "Saving…" : saveState === "error" ? "Couldn't save" : "Saved"}
        </span>
      </div>
      {toolbar}
      <EditorContent editor={editor} className="notes-body" hidden={hideBody} />
    </div>
  );
}
