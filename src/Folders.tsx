import { Check, ChevronDown, Folder as FolderIcon, FolderInput, FolderMinus, FolderPlus, X } from "lucide-react";
import { ReactNode, createContext, useCallback, useContext, useEffect, useRef, useState } from "react";
import { Menu } from "./Menu";
import { onStoreReady } from "./lib/library";
import { Folder, checkFolderName, createFolder, listFolders, setMeetingFolder } from "./lib/folders";

interface FoldersValue {
  folders: Folder[];
  refresh: () => void;
  /** Creates a folder after checking the name; throws a message to show on failure. */
  create: (name: string) => Promise<Folder>;
  byId: (id: string | null | undefined) => Folder | undefined;
}

const Ctx = createContext<FoldersValue | null>(null);

export function useFolders(): FoldersValue {
  const v = useContext(Ctx);
  if (!v) throw new Error("useFolders outside FoldersProvider");
  return v;
}

export function FoldersProvider({ children }: { children: ReactNode }) {
  const [folders, setFolders] = useState<Folder[]>([]);
  const refresh = useCallback(() => {
    listFolders().then(setFolders).catch(() => {});
  }, []);

  useEffect(() => {
    refresh();
    const sub = onStoreReady(refresh);
    return () => {
      sub.then((fn) => fn());
    };
  }, [refresh]);

  const create = async (name: string) => {
    const checked = checkFolderName(name, folders);
    if ("error" in checked) throw checked.error;
    const folder = await createFolder(checked.name);
    refresh();
    return folder;
  };

  return (
    <Ctx.Provider value={{ folders, refresh, create, byId: (id) => (id ? folders.find((f) => f.id === id) : undefined) }}>
      {children}
    </Ctx.Provider>
  );
}

/** A one-line form for naming a folder (new or renamed). Enter saves, Escape cancels. */
export function FolderNameForm({
  initial = "",
  submitLabel,
  onSubmit,
  onCancel,
}: {
  initial?: string;
  submitLabel: string;
  onSubmit: (name: string) => Promise<void>;
  onCancel: () => void;
}) {
  const [name, setName] = useState(initial);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const input = useRef<HTMLInputElement>(null);

  useEffect(() => {
    input.current?.focus();
    input.current?.select();
  }, []);

  const submit = async () => {
    setBusy(true);
    setError(null);
    try {
      await onSubmit(name);
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  };

  return (
    <form
      className="folder-form"
      onSubmit={(e) => {
        e.preventDefault();
        void submit();
      }}
    >
      <div className="folder-form-row">
        <FolderIcon size={16} aria-hidden />
        <input
          ref={input}
          aria-label="Folder name"
          placeholder="Folder name"
          value={name}
          maxLength={80}
          onChange={(e) => setName(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") {
              e.preventDefault();
              onCancel();
            }
          }}
          aria-invalid={error ? true : undefined}
          aria-describedby={error ? "folder-form-error" : undefined}
        />
        <button className="icon-button small" type="button" aria-label="Cancel" onClick={onCancel}>
          <X size={14} aria-hidden />
        </button>
      </div>
      {error && (
        <p className="error" id="folder-form-error" role="alert">
          {error}
        </p>
      )}
      <button className="primary small-button" type="submit" disabled={busy || !name.trim()}>
        {submitLabel}
      </button>
    </form>
  );
}

/** The folder chip on a meeting: shows where it's filed and moves it. */
export function FolderChip({ meetingId, folderId, onChange }: { meetingId: string; folderId: string | null; onChange: (id: string | null) => void }) {
  const { folders, byId, refresh, create } = useFolders();
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const current = byId(folderId);

  const move = async (id: string | null) => {
    setError(null);
    try {
      await setMeetingFolder(meetingId, id);
      onChange(id);
      refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  if (creating) {
    return (
      <div className="folder-chip-form">
        <FolderNameForm
          submitLabel="Create and move"
          onCancel={() => setCreating(false)}
          onSubmit={async (name) => {
            const folder = await create(name);
            await move(folder.id);
            setCreating(false);
          }}
        />
      </div>
    );
  }

  return (
    <>
      <Menu
        label={current ? `Folder: ${current.name}. Move to another folder` : "Add to a folder"}
        heading="Move to folder"
        trigger={
          <>
            {current ? <FolderIcon size={14} aria-hidden /> : <FolderInput size={14} aria-hidden />}
            {current ? current.name : "Add to folder"}
            <ChevronDown size={13} aria-hidden />
          </>
        }
        items={[
          ...folders.map((f) => ({
            label: f.name,
            icon: <FolderIcon size={16} />,
            trailing: f.id === folderId ? <Check size={16} aria-label="Current folder" /> : undefined,
            onSelect: () => void move(f.id),
          })),
          ...(folders.length ? (["separator"] as const) : []),
          { label: "New folder…", icon: <FolderPlus size={16} />, onSelect: () => setCreating(true) },
          ...(current ? [{ label: "Remove from folder", icon: <FolderMinus size={16} />, onSelect: () => void move(null) }] : []),
        ]}
      />
      {error && <span className="error" role="alert">{error}</span>}
    </>
  );
}
