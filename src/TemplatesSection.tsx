import { useEffect, useState } from "react";
import { Template, deleteTemplate, listTemplates, newTemplate, saveTemplate } from "./lib/enhance";

/** Built-in templates are read-only; custom ones can be created, edited and deleted. */
export function TemplatesSection() {
  const [templates, setTemplates] = useState<Template[]>([]);
  const [editing, setEditing] = useState<Template | null>(null);
  const [status, setStatus] = useState("");

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

  return (
    <div className="templates">
      <h2>Note templates</h2>
      <ul className="template-list">
        {templates.map((t) => (
          <li key={t.id}>
            <span>{t.name}</span>
            {t.builtIn ? (
              <button onClick={() => setEditing({ ...t, id: "", name: `${t.name} (copy)`, builtIn: false })}>Copy</button>
            ) : (
              <>
                <button onClick={() => setEditing(t)}>Edit</button>
                <button onClick={() => remove(t)}>Delete</button>
              </>
            )}
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
              rows={8}
              value={editing.instructions}
              onChange={(e) => setEditing({ ...editing, instructions: e.target.value })}
            />
          </label>
          <div className="actions">
            <button className="primary" onClick={save} disabled={!editing.name.trim() || !editing.instructions.trim()}>
              Save template
            </button>
            <button onClick={() => setEditing(null)}>Cancel</button>
          </div>
        </div>
      ) : (
        <button onClick={() => setEditing(newTemplate())}>New template</button>
      )}
      <p className="status" role="status">{status}</p>
    </div>
  );
}
