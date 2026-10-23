import { FileText, Search } from "lucide-react";
import { ReactNode, useEffect, useId, useMemo, useRef, useState } from "react";
import { timeOfDay, dayLabel } from "./lib/dates";
import { MeetingSummary, listMeetings, searchMeetings } from "./lib/library";

export interface Command {
  id: string;
  label: string;
  group: "Actions" | "Go to" | "Folders" | "Appearance";
  icon: ReactNode;
  shortcut?: string;
  run: () => void;
}

interface Props {
  commands: Command[];
  onOpenMeeting: (id: string) => void;
  onClose: () => void;
}

interface Row {
  id: string;
  group: string;
  label: string;
  icon: ReactNode;
  hint?: string;
  run: () => void;
}

/** Case-insensitive match on every word, so "new fol" finds "New folder". */
export function matches(label: string, query: string): boolean {
  const l = label.toLowerCase();
  return query
    .toLowerCase()
    .split(/\s+/)
    .filter(Boolean)
    .every((w) => l.includes(w));
}

/** ⌘K: jump anywhere, run any action, find any meeting. */
export function CommandPalette({ commands, onOpenMeeting, onClose }: Props) {
  const [query, setQuery] = useState("");
  const [meetings, setMeetings] = useState<MeetingSummary[]>([]);
  const [active, setActive] = useState(0);
  const input = useRef<HTMLInputElement>(null);
  const listId = useId();
  const returnFocus = useRef<Element | null>(document.activeElement);

  useEffect(() => {
    input.current?.focus();
    const back = returnFocus.current;
    return () => {
      if (back instanceof HTMLElement) back.focus();
    };
  }, []);

  // Recent meetings with no query; full-text matches once something is typed.
  useEffect(() => {
    const q = query.trim();
    const t = setTimeout(
      () => {
        (q.length >= 2 ? searchMeetings(q) : listMeetings(null, 5)).then((m) => setMeetings(m.slice(0, 6))).catch(() => setMeetings([]));
      },
      q ? 150 : 0,
    );
    return () => clearTimeout(t);
  }, [query]);

  const rows: Row[] = useMemo(() => {
    const q = query.trim();
    const cmds = commands.filter((c) => !q || matches(c.label, q)).map((c) => ({ ...c, hint: c.shortcut }));
    const meetingRows = meetings.map((m) => ({
      id: `meeting:${m.id}`,
      group: q.length >= 2 ? "Meetings" : "Recent meetings",
      label: m.title,
      icon: <FileText size={16} />,
      hint: `${dayLabel(m.startedAtMs)}, ${timeOfDay(m.startedAtMs)}`,
      run: () => onOpenMeeting(m.id),
    }));
    return [...cmds, ...meetingRows];
  }, [commands, meetings, query, onOpenMeeting]);

  useEffect(() => setActive(0), [query]);
  const clamped = Math.min(active, Math.max(rows.length - 1, 0));

  const choose = (row: Row | undefined) => {
    if (!row) return;
    onClose();
    row.run();
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown") setActive((i) => Math.min(i + 1, rows.length - 1));
    else if (e.key === "ArrowUp") setActive((i) => Math.max(i - 1, 0));
    else if (e.key === "Enter") choose(rows[clamped]);
    else if (e.key === "Escape") onClose();
    else if (e.key === "Tab") {
      // Focus stays in the palette; Tab moves through results like the arrows.
      setActive((i) => (e.shiftKey ? Math.max(i - 1, 0) : Math.min(i + 1, rows.length - 1)));
    } else return;
    e.preventDefault();
  };

  useEffect(() => {
    document.getElementById(`${listId}-${clamped}`)?.scrollIntoView({ block: "nearest" });
  }, [clamped, listId]);

  let lastGroup = "";
  return (
    <div className="palette-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="palette" role="dialog" aria-modal="true" aria-label="Command palette" onKeyDown={onKeyDown}>
        <div className="palette-input">
          <Search size={18} aria-hidden />
          <input
            ref={input}
            role="combobox"
            aria-expanded="true"
            aria-controls={listId}
            aria-activedescendant={rows.length ? `${listId}-${clamped}` : undefined}
            aria-autocomplete="list"
            placeholder="Search meetings or type a command"
            spellCheck={false}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
          <kbd aria-hidden>esc</kbd>
        </div>
        <div className="palette-list" role="listbox" id={listId} aria-label="Results">
          {rows.length === 0 && <p className="palette-empty">No commands or meetings match “{query.trim()}”.</p>}
          {rows.map((r, i) => {
            const header = r.group !== lastGroup ? r.group : null;
            lastGroup = r.group;
            return (
              <div key={r.id} role="presentation">
                {header && <div className="palette-group" role="presentation">{header}</div>}
                <div
                  id={`${listId}-${i}`}
                  role="option"
                  aria-selected={i === clamped}
                  className={`palette-row${i === clamped ? " active" : ""}`}
                  onMouseMove={() => setActive(i)}
                  onClick={() => choose(r)}
                >
                  <span className="palette-icon" aria-hidden>{r.icon}</span>
                  <span className="palette-label">{r.label}</span>
                  {r.hint && <span className="palette-hint">{r.hint}</span>}
                </div>
              </div>
            );
          })}
        </div>
        <div className="palette-foot" aria-hidden>
          <span><kbd>↑</kbd><kbd>↓</kbd> move</span>
          <span><kbd>↵</kbd> open</span>
          <span><kbd>esc</kbd> close</span>
        </div>
      </div>
    </div>
  );
}
