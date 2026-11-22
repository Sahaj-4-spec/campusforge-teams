import { useEffect, useState } from "react";
import { FolderPlus, X } from "lucide-react";

export default function ProjectDialog({ open, onClose, onSave }) {
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    if (open) {
      setName("");
      setDescription("");
      setError("");
      setSaving(false);
    }
  }, [open]);

  if (!open) return null;

  async function submit(event) {
    event.preventDefault();
    if (!name.trim()) return;
    setSaving(true);
    setError("");
    try {
      await onSave({ name: name.trim(), description: description.trim() || null });
    } catch (saveError) {
      setError(saveError.message || "Could not create project");
      setSaving(false);
    }
  }

  return (
    <div className="dialog-backdrop" role="presentation" onMouseDown={(event) => {
      if (event.target === event.currentTarget) onClose();
    }}>
      <section className="task-dialog project-dialog" role="dialog" aria-modal="true" aria-labelledby="project-dialog-title">
        <div className="dialog-header">
          <div>
            <p className="eyebrow">NEW PROJECT</p>
            <h2 id="project-dialog-title">Create a project</h2>
          </div>
          <button className="icon-button" type="button" onClick={onClose} aria-label="Close dialog"><X size={19} /></button>
        </div>
        <form className="task-form" onSubmit={submit}>
          <label className="field-group">
            <span>Project name <b aria-hidden="true">*</b></span>
            <input autoFocus value={name} onChange={(event) => setName(event.target.value)} placeholder="e.g. Campus event planning" maxLength={80} required />
          </label>
          <label className="field-group">
            <span>Description</span>
            <textarea value={description} onChange={(event) => setDescription(event.target.value)} placeholder="What is this project about?" rows={4} maxLength={1000} />
          </label>
          {error && <p className="auth-error" role="alert">{error}</p>}
          <div className="dialog-actions">
            <span className="muted-copy"><FolderPlus size={14} /> Four starter columns will be added</span>
            <div className="dialog-actions-right">
              <button className="secondary-button" type="button" onClick={onClose} disabled={saving}>Cancel</button>
              <button className="primary-button" type="submit" disabled={saving || !name.trim()}>{saving ? "Creating..." : "Create project"}</button>
            </div>
          </div>
        </form>
      </section>
    </div>
  );
}
