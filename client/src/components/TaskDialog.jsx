import { useEffect, useState } from "react";
import { CalendarDays, MessageCircle, Trash2, X } from "lucide-react";

function formatCommentDate(value) {
  if (!value) return "";
  return new Intl.DateTimeFormat(undefined, {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit"
  }).format(new Date(value));
}

export default function TaskDialog({
  open,
  task,
  defaultListId,
  lists,
  users,
  assigneeIds,
  comments,
  currentUser,
  onClose,
  onSave,
  onDelete,
  onAddComment
}) {
  const [form, setForm] = useState({
    title: "",
    description: "",
    priority: "medium",
    due_date: "",
    list_id: defaultListId || "",
    assignee_id: ""
  });
  const [commentDraft, setCommentDraft] = useState("");

  useEffect(() => {
    if (task) {
      setForm({
        title: task.title || "",
        description: task.description || "",
        priority: task.priority || "medium",
        due_date: task.due_date || "",
        list_id: task.list_id || defaultListId || "",
        assignee_id: assigneeIds[0] || ""
      });
    } else {
      setForm({
        title: "",
        description: "",
        priority: "medium",
        due_date: "",
        list_id: defaultListId || (lists[0] && lists[0].id) || "",
        assignee_id: ""
      });
    }
    setCommentDraft("");
  }, [task, open, defaultListId, lists, assigneeIds]);

  useEffect(() => {
    if (!open) return undefined;
    function handleKeyDown(event) {
      if (event.key === "Escape") onClose();
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [open, onClose]);

  if (!open) return null;

  function updateField(event) {
    const { name, value } = event.target;
    setForm((current) => ({ ...current, [name]: value }));
  }

  function submitForm(event) {
    event.preventDefault();
    if (!form.title.trim()) return;
    onSave({
      title: form.title.trim(),
      description: form.description.trim(),
      priority: form.priority,
      due_date: form.due_date || null,
      list_id: form.list_id,
      assignee_id: form.assignee_id || null
    });
  }

  function submitComment(event) {
    event.preventDefault();
    if (!task || !commentDraft.trim()) return;
    onAddComment(task.id, commentDraft.trim());
    setCommentDraft("");
  }

  const taskComments = task
    ? comments.filter((comment) => comment.task_id === task.id)
    : [];

  return (
    <div
      className="dialog-backdrop"
      role="presentation"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <section className="task-dialog" role="dialog" aria-modal="true" aria-labelledby="task-dialog-title">
        <div className="dialog-header">
          <div>
            <p className="eyebrow">{task ? "TASK DETAILS" : "NEW TASK"}</p>
            <h2 id="task-dialog-title">{task ? "Update task" : "Create a task"}</h2>
          </div>
          <button className="icon-button" type="button" onClick={onClose} aria-label="Close dialog">
            <X size={19} />
          </button>
        </div>

        <form className="task-form" onSubmit={submitForm}>
          <label className="field-group">
            <span>Task title <b aria-hidden="true">*</b></span>
            <input
              autoFocus
              name="title"
              value={form.title}
              onChange={updateField}
              placeholder="e.g. Prepare the project handoff"
              maxLength={180}
              required
            />
          </label>

          <label className="field-group">
            <span>Description</span>
            <textarea
              name="description"
              value={form.description}
              onChange={updateField}
              placeholder="Add context, acceptance criteria, or useful links..."
              rows={4}
              maxLength={4000}
            />
          </label>

          <div className="field-grid">
            <label className="field-group">
              <span>List</span>
              <select name="list_id" value={form.list_id} onChange={updateField}>
                {lists.map((list) => (
                  <option key={list.id} value={list.id}>{list.name}</option>
                ))}
              </select>
            </label>
            <label className="field-group">
              <span>Priority</span>
              <select name="priority" value={form.priority} onChange={updateField}>
                <option value="low">Low</option>
                <option value="medium">Medium</option>
                <option value="high">High</option>
              </select>
            </label>
            <label className="field-group">
              <span>Due date</span>
              <input type="date" name="due_date" value={form.due_date} onChange={updateField} />
            </label>
            <label className="field-group">
              <span>Assignee</span>
              <select name="assignee_id" value={form.assignee_id} onChange={updateField}>
                <option value="">Unassigned</option>
                {users.map((user) => (
                  <option key={user.id} value={user.id}>{user.full_name}</option>
                ))}
              </select>
            </label>
          </div>

          <div className="dialog-actions">
            {task && (
              <button className="danger-button" type="button" onClick={() => onDelete(task.id)}>
                <Trash2 size={15} /> Delete task
              </button>
            )}
            <div className="dialog-actions-right">
              <button className="secondary-button" type="button" onClick={onClose}>Cancel</button>
              <button className="primary-button" type="submit">{task ? "Save changes" : "Create task"}</button>
            </div>
          </div>
        </form>

        {task && (
          <section className="comments-section" aria-label="Task comments">
            <div className="comments-heading">
              <h3><MessageCircle size={16} /> Discussion</h3>
              <span>{taskComments.length}</span>
            </div>
            {taskComments.length > 0 ? (
              <div className="comment-list">
                {taskComments.map((comment) => {
                  const author = users.find((user) => user.id === comment.author_id);
                  return (
                    <article className="comment-item" key={comment.id}>
                      <div className="avatar avatar-small">{(author?.full_name || "M").slice(0, 1)}</div>
                      <div className="comment-body">
                        <div className="comment-meta">
                          <strong>{author?.full_name || "Workspace member"}</strong>
                          <time>{formatCommentDate(comment.created_at)}</time>
                        </div>
                        <p>{comment.body}</p>
                      </div>
                    </article>
                  );
                })}
              </div>
            ) : (
              <p className="comment-empty">No comments yet. Start the discussion with your team.</p>
            )}
            <form className="comment-form" onSubmit={submitComment}>
              <div className="avatar avatar-small">{(currentUser?.full_name || "Y").slice(0, 1)}</div>
              <label className="sr-only" htmlFor="comment-draft">Write a comment</label>
              <input
                id="comment-draft"
                value={commentDraft}
                onChange={(event) => setCommentDraft(event.target.value)}
                placeholder="Write a comment..."
                maxLength={2000}
              />
              <button className="comment-submit" type="submit" disabled={!commentDraft.trim()}>Send</button>
            </form>
          </section>
        )}
        <div className="dialog-footnote"><CalendarDays size={13} /> Demo changes are saved in this browser only.</div>
      </section>
    </div>
  );
}
