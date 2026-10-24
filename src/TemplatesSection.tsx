import { Copy, Eye, EyeOff, LayoutTemplate, Pencil, Plus, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { Template, deleteTemplate, listTemplates, newTemplate, saveTemplate } from "./lib/enhance";
import { templateIcon } from "./templateIcons";

/** The "## Section" headings of a template, for the gallery preview. */
export const sectionsOf = (instructions: string) =>
  instructions
    .split("\n")
    .filter((l) => l.startsWith("## "))
    .map((l) => l.slice(3).trim());

/** Built-in templates are read-only; custom ones can be created, edited and deleted. */
export function TemplatesSection() {
  const [templates, setTemplates] = useState<Template[]>([]);
  const [editing, setEditing] = useState<Template | null>(null);
  const [status, setStatus] = useState("");
  const [preview, setPreview] = useState<string | null>(null);

  const refresh = () => listTemplates().then(setTemplates).catch((e) => setStatus(`Couldn't load templates: ${e}`));

  useEffect(() => {
    refresh();
  }, []);

  const save = async () => {
    if (!editing) return;
    try {
      await saveTemplate(editing);
      setEditing(null);
      setStatus("Template saved");
      refresh();
    } catch (e) {
      setStatus(`Couldn't save: ${e}`);
    }
  };

  const remove = async (t: Template) => {
    try {
      await deleteTemplate(t.id);
      setStatus(`Deleted "${t.name}"`);
      refresh();
    } catch (e) {
      setStatus(`Couldn't delete: ${e}`);
    }
  };

  const builtIns = templates.filter((t) => t.builtIn);
  const custom = templates.filter((t) => !t.builtIn);
  const copy = (t: Template) => setEditing({ ...t, id: "", name: `${t.name} (copy)`, builtIn: false, description: "" });

  return (
    <div className="templates">
      <h2 className="section-title"><LayoutTemplate size={17} aria-hidden />AI note templates</h2>
      <p className="card-text">
        Pick a template on any meeting before writing AI notes. Parley suggests one from the meeting's title, like Interview for "Interview:
        Ana Ruiz" or 1:1 for "Weekly 1:1".
      </p>

      <ul className="template-grid">
        {builtIns.map((t) => {
          const open = preview === t.id;
          return (
            <li key={t.id} className="template-card">
              <div className="template-card-head">
                <span className="template-icon" aria-hidden>{templateIcon(t.id, 18)}</span>
                <strong>{t.name}</strong>
              </div>
              <p className="template-desc">{t.description}</p>
              {open && (
                <ol className="template-sections" aria-label={`${t.name} sections`}>
                  {sectionsOf(t.instructions).map((h) => (
                    <li key={h}>{h}</li>
                  ))}
                </ol>
              )}
              <div className="template-actions">
                <button className="ghost small-ghost" aria-expanded={open} onClick={() => setPreview(open ? null : t.id)}>
                  {open ? <EyeOff size={14} aria-hidden /> : <Eye size={14} aria-hidden />}
                  {open ? "Hide sections" : "Sections"}
                </button>
                <button className="ghost small-ghost" onClick={() => copy(t)} aria-label={`Copy ${t.name} and edit it`}>
                  <Copy size={14} aria-hidden />
                  Copy and edit
                </button>
              </div>
            </li>
          );
        })}
      </ul>

      <h3 className="subsection">Your templates</h3>
      {custom.length === 0 && !editing && <p className="hint">None yet. Start from scratch or copy a built-in one above.</p>}
      <ul className="template-list">
        {custom.map((t) => (
          <li key={t.id}>
            <span className="template-icon small" aria-hidden>{templateIcon(t.id, 15)}</span>
            <span>{t.name}</span>
            <button className="ghost small-ghost" onClick={() => setEditing(t)} aria-label={`Edit ${t.name}`}>
              <Pencil size={14} aria-hidden />
              Edit
            </button>
            <button className="ghost small-ghost" onClick={() => remove(t)} aria-label={`Delete ${t.name}`}>
              <Trash2 size={14} aria-hidden />
              Delete
            </button>
          </li>
        ))}
      </ul>
      {editing ? (
        <div className="template-editor">
          <label>
            Name
            <input value={editing.name} onChange={(e) => setEditing({ ...editing, name: e.target.value })} />
          </label>
          <label>
            What the notes should contain
            <textarea
              rows={10}
              value={editing.instructions}
              onChange={(e) => setEditing({ ...editing, instructions: e.target.value })}
            />
          </label>
          <p className="hint">Use "## Heading" lines for sections. Leave a short note under each on what belongs there.</p>
          <div className="actions">
            <button className="primary" onClick={save} disabled={!editing.name.trim() || !editing.instructions.trim()}>
              Save template
            </button>
            <button onClick={() => setEditing(null)}>Cancel</button>
          </div>
        </div>
      ) : (
        <button onClick={() => setEditing(newTemplate())}>
          <Plus size={15} aria-hidden />
          New template
        </button>
      )}
      <p className="status" role="status">{status}</p>
    </div>
  );
}
