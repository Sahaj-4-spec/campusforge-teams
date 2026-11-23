import { useEffect, useRef, useState } from "react";
import { AtSign, CalendarDays, CornerDownRight, MessageCircle, Reply, Trash2, X } from "lucide-react";
import TaskAdvancedPanel from "./TaskAdvancedPanel.jsx";
import { sendWorkspaceSocketMessage } from "../lib/api.js";

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
  onAddComment,
  allTasks = [],
  onAdvancedChanged,
  refreshKey = 0
}) {
  const [form, setForm] = useState({
    title: "",
    description: "",
    priority: "medium",
    due_date: "",
    list_id: defaultListId || "",
    assignee_id: "",
    estimated_minutes: ""
  });
  const [commentDraft, setCommentDraft] = useState("");
  const [replyToCommentId, setReplyToCommentId] = useState("");
  const [mentionUserId, setMentionUserId] = useState("");
  const dialogWasOpen = useRef(false);
  const lastTaskIdentity = useRef(null);

  useEffect(() => {
    if (!open) {
      dialogWasOpen.current = false;
      lastTaskIdentity.current = null;
      return;
    }

    const identity = task?.id || "__new_task__";
    if (!dialogWasOpen.current || lastTaskIdentity.current !== identity) {
      if (task) {
        setForm({
          title: task.title || "",
          description: task.description || "",
          priority: task.priority || "medium",
          due_date: task.due_date || "",
          list_id: task.list_id || defaultListId || "",
          assignee_id: assigneeIds[0] || "",
          estimated_minutes: task.estimated_minutes ? String(task.estimated_minutes) : ""
        });
      } else {
        setForm({
          title: "",
          description: "",
          priority: "medium",
          due_date: "",
          list_id: defaultListId || (lists[0] && lists[0].id) || "",
          assignee_id: "",
          estimated_minutes: ""
        });
      }
      setCommentDraft("");
      setReplyToCommentId("");
      setMentionUserId("");
      lastTaskIdentity.current = identity;
    }
    dialogWasOpen.current = true;
  }, [open, task?.id]);

  useEffect(() => {
    if (!open || !task?.id || !task?.board_id) return undefined;
    const publishEditing = (editing) => sendWorkspaceSocketMessage({
      type: "card_editing",
      board_id: task.board_id,
      task_id: task.id,
      editing
    });
    publishEditing(true);
    const heartbeat = window.setInterval(() => publishEditing(true), 7000);
    return () => {
      window.clearInterval(heartbeat);
      publishEditing(false);
    };
  }, [open, task?.id, task?.board_id]);

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
      assignee_id: form.assignee_id || null,
      estimated_minutes: form.estimated_minutes ? Number(form.estimated_minutes) : null
    });
  }

  function submitComment(event) {
    event.preventDefault();
    if (!task || !commentDraft.trim()) return;
    onAddComment(task.id, commentDraft.trim(), replyToCommentId || null);
    setCommentDraft("");
    setReplyToCommentId("");
    setMentionUserId("");
  }

  function insertCommentMention(userId) {
    if (!userId) return;
    setCommentDraft((current) => (current ? current + " " : "") + "@{" + userId + "} ");
    setMentionUserId("");
  }

  function displayCommentBody(body) {
    let remaining = body || "";
    let rendered = "";
    while (remaining) {
      const start = remaining.indexOf("@{");
      if (start < 0) return rendered + remaining;
      const end = remaining.indexOf("}", start + 2);
      if (end < 0) return rendered + remaining;
      const userId = remaining.slice(start + 2, end).trim();
      const user = users.find((row) => row.id === userId);
      rendered += remaining.slice(0, start) + "@" + (user?.full_name || "workspace member");
      remaining = remaining.slice(end + 1);
    }
    return rendered;
  }

  const taskComments = task
    ? comments.filter((comment) => comment.task_id === task.id)
    : [];
  const rootComments = taskComments.filter((comment) => !comment.parent_comment_id);
  const replyTarget = taskComments.find((comment) => comment.id === replyToCommentId);

  function renderComment(comment, isReply = false) {
    const author = users.find((user) => user.id === comment.author_id);
    const replies = isReply ? [] : taskComments.filter((row) => row.parent_comment_id === comment.id);
    return (
      <article className={"comment-item" + (isReply ? " comment-reply" : "")} key={comment.id}>
        {isReply && <CornerDownRight className="comment-reply-mark" size={14} />}
        <div className="avatar avatar-small">{(author?.full_name || "M").slice(0, 1)}</div>
        <div className="comment-body">
          <div className="comment-meta">
            <strong>{author?.full_name || "Workspace member"}</strong>
            <time>{formatCommentDate(comment.created_at)}</time>
          </div>
          <p>{displayCommentBody(comment.body)}</p>
          {!isReply && (
            <button className="comment-reply-button" type="button" onClick={() => setReplyToCommentId(comment.id)}>
              <Reply size={12} /> Reply
            </button>
          )}
          {replies.length > 0 && <div className="comment-replies">{replies.map((reply) => renderComment(reply, true))}</div>}
        </div>
      </article>
    );
  }

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
            <label className="field-group">
              <span>Estimated time (minutes)</span>
              <input type="number" name="estimated_minutes" min="1" max="100000" value={form.estimated_minutes} onChange={updateField} placeholder="e.g. 90" />
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

        {task && <TaskAdvancedPanel task={task} lists={lists} tasks={allTasks} onChanged={onAdvancedChanged} refreshKey={refreshKey} />}

        {task && (
          <section className="comments-section" aria-label="Task comments">
            <div className="comments-heading">
              <h3><MessageCircle size={16} /> Discussion</h3>
              <span>{taskComments.length}</span>
            </div>
            {rootComments.length > 0 ? (
              <div className="comment-list">
                {rootComments.map((comment) => renderComment(comment))}
              </div>
            ) : (
              <p className="comment-empty">No comments yet. Start the discussion with your team.</p>
            )}
            <form className="comment-form comment-thread-composer" onSubmit={submitComment}>
              <div className="avatar avatar-small">{(currentUser?.full_name || "Y").slice(0, 1)}</div>
              <div className="comment-compose-main">
                {replyTarget && (
                  <div className="comment-replying-to">
                    Replying to {users.find((user) => user.id === replyTarget.author_id)?.full_name || "a teammate"}
                    <button type="button" onClick={() => setReplyToCommentId("")} aria-label="Cancel reply"><X size={12} /></button>
                  </div>
                )}
                <input
                  id="comment-draft"
                  value={commentDraft}
                  onChange={(event) => setCommentDraft(event.target.value)}
                  placeholder={replyTarget ? "Write a reply..." : "Write a comment..."}
                  maxLength={2000}
                  aria-label={replyTarget ? "Write a threaded reply" : "Write a comment"}
                />
                <div className="comment-compose-actions">
                  <label className="comment-mention-picker">
                    <AtSign size={13} />
                    <select value={mentionUserId} onChange={(event) => insertCommentMention(event.target.value)} aria-label="Mention a teammate">
                      <option value="">Mention teammate</option>
                      {users.filter((user) => user.id !== currentUser?.id).map((user) => (
                        <option key={user.id} value={user.id}>{user.full_name}</option>
                      ))}
                    </select>
                  </label>
                  <button className="comment-submit" type="submit" disabled={!commentDraft.trim()}>Send</button>
                </div>
              </div>
            </form>
          </section>
        )}
        <div className="dialog-footnote"><CalendarDays size={13} /> Changes are saved to your shared workspace.</div>
      </section>
    </div>
  );
}
