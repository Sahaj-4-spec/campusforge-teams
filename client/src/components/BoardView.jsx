import { useEffect, useMemo, useRef, useState } from "react";
import {
  DndContext,
  DragOverlay,
  PointerSensor,
  closestCorners,
  useDroppable,
  useSensor,
  useSensors
} from "@dnd-kit/core";
import {
  SortableContext,
  useSortable,
  verticalListSortingStrategy
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import {
  CalendarDays,
  CheckCircle2,
  GripVertical,
  MoreHorizontal,
  Pencil,
  Plus,
  Trash2,
  UserRound
} from "lucide-react";
import { presenceApi, subscribeWorkspaceSocket } from "../lib/api.js";

function formatDueDate(value) {
  if (!value) return "No due date";
  return new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric" }).format(
    new Date(value + "T00:00:00")
  );
}

function initials(name) {
  return (name || "U")
    .split(" ")
    .map((part) => part[0])
    .join("")
    .slice(0, 2)
    .toUpperCase();
}

function getChecklistProgress(task) {
  const items = Array.isArray(task.checklist) ? task.checklist : [];
  const total = Number(task.checklist_total ?? task.checklist_count ?? items.length);
  if (!total) return null;
  const completed = Number(
    task.checklist_completed_count ??
    task.checklist_done_count ??
    items.filter((item) => item.is_done || item.completed).length
  );
  return { completed, total };
}

function TaskCardFace({ task, users, assigneeIds, lists = [], editors = [], onOpen, onMove, dragHandleProps }) {
  const assignedUsers = assigneeIds
    .map((id) => users.find((user) => user.id === id))
    .filter(Boolean);
  const checklist = getChecklistProgress(task);
  const label = task.category || task.labels?.[0]?.name || task.label || task.priority || "Task";
  const labelTone = task.priority || "medium";

  return (
    <>
      <div className="task-card-topline">
        <span className={"priority-tag priority-" + labelTone}>{label}</span>
        <div className="task-card-controls">
          <details className="task-quick-menu">
            <summary aria-label={"Card actions for " + task.title} title="Card actions">
              <MoreHorizontal size={16} />
            </summary>
            <div className="task-quick-menu-panel">
              <button type="button" onClick={() => onOpen(task)}>Open card details</button>
              <label>
                <span>Move to list</span>
                <select
                  aria-label={"Move " + task.title + " to list"}
                  value={task.list_id}
                  onChange={(event) => {
                    const listId = event.target.value;
                    if (listId && listId !== task.list_id) onMove?.(task.id, listId, "list:" + listId);
                    event.currentTarget.closest("details")?.removeAttribute("open");
                  }}
                >
                  {lists.map((list) => <option key={list.id} value={list.id}>{list.name}</option>)}
                </select>
              </label>
            </div>
          </details>
          <button
            className="drag-handle"
            type="button"
            aria-label={"Drag " + task.title}
            data-drag-handle
            {...(dragHandleProps || {})}
          >
            <GripVertical size={15} />
          </button>
        </div>
      </div>
      <button className="task-title-button" type="button" onClick={() => onOpen(task)}>
        {task.title}
      </button>
      <div className="task-card-footer">
        <span className={"task-date" + (task.due_date && task.due_date < new Date().toISOString().slice(0, 10) ? " overdue" : "")}>
          <CalendarDays size={12} /> <time className="tabular-nums" dateTime={task.due_date || undefined}>{formatDueDate(task.due_date)}</time>
        </span>
        <span className="task-card-meta">
          {checklist && (
            <span className="task-checklist-progress" title={checklist.completed + " of " + checklist.total + " checklist items complete"}>
              <CheckCircle2 size={12} /> <span className="tabular-nums">{checklist.completed}/{checklist.total}</span>
            </span>
          )}
          <span className="task-assignees" aria-label="Task assignees">
            {assignedUsers.length > 0
              ? assignedUsers.slice(0, 2).map((user) => (
                  <span className="mini-avatar" key={user.id} title={user.full_name}>{initials(user.full_name)}</span>
                ))
              : <span className="unassigned-avatar" title="Unassigned"><UserRound size={11} /></span>}
          </span>
        </span>
      </div>
      {editors.length > 0 && <span className="task-card-editor-signal" aria-label={editors.map((user) => user.full_name).join(", ") + " editing"} />}
    </>
  );
}

function SortableTask({ task, users, assigneeIds, lists, editors = [], onOpen, onMove, movePending = false }) {
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
    isOver
  } = useSortable({
    id: task.id,
    disabled: movePending
  });

  return (
    <article
      ref={setNodeRef}
      style={{ transform: CSS.Transform.toString(transform), transition }}
      className={
        "task-card" +
        (isDragging ? " task-card-dragging" : "") +
        (isOver ? " task-card-drop-target" : "") +
        (editors.length ? " task-card-being-edited" : "")
      }
    >
      <TaskCardFace
        task={task}
        users={users}
        assigneeIds={assigneeIds}
        lists={lists}
        editors={editors}
        onOpen={onOpen}
        onMove={onMove}
        dragHandleProps={{ ...attributes, ...listeners }}
      />
    </article>
  );
}

function BoardColumn({
  list,
  lists,
  tasks,
  users,
  assigneesByTask,
  editingByTask,
  onOpenTask,
  onAddTask,
  onQuickAddTask,
  onMoveTask,
  onRenameList,
  onDeleteList,
  canManageLists,
  movePending
}) {
  const { setNodeRef, isOver } = useDroppable({
    id: "list:" + list.id,
    data: { type: "column", listId: list.id }
  });
  const [quickAddOpen, setQuickAddOpen] = useState(false);
  const [draftTitle, setDraftTitle] = useState("");
  const [savingQuickAdd, setSavingQuickAdd] = useState(false);
  const quickAddRef = useRef(null);

  useEffect(() => {
    if (quickAddOpen) quickAddRef.current?.focus();
  }, [quickAddOpen]);

  async function submitQuickAdd(event) {
    event.preventDefault();
    if (!draftTitle.trim() || savingQuickAdd) return;
    setSavingQuickAdd(true);
    try {
      await onQuickAddTask(list.id, draftTitle.trim());
      setDraftTitle("");
      setQuickAddOpen(false);
    } finally {
      setSavingQuickAdd(false);
    }
  }

  function cancelQuickAdd() {
    setDraftTitle("");
    setQuickAddOpen(false);
  }

  return (
    <section className={"board-column" + (isOver ? " board-column-over" : "")} aria-label={list.name + " list"}>
      <header className="column-heading">
        <div className="column-title-wrap">
          <span className={"column-dot dot-" + list.name.toLowerCase().replaceAll(" ", "-")} />
          <h3 title={list.name}>{list.name}</h3>
          <span className="column-count tabular-nums" aria-label={tasks.length + " cards"}>{tasks.length}</span>
        </div>
        <div className="column-actions">
          {canManageLists && (
            <>
              <button className="column-add" type="button" onClick={() => onRenameList(list)} aria-label={"Rename " + list.name} title="Rename list">
                <Pencil size={13} />
              </button>
              <button className="column-add column-delete" type="button" onClick={() => onDeleteList(list)} aria-label={"Delete " + list.name} title="Delete empty list">
                <Trash2 size={13} />
              </button>
            </>
          )}
          <button className="column-add" type="button" onClick={() => onAddTask(list.id)} aria-label={"Open card editor in " + list.name} title="Create detailed card">
            <Plus size={17} />
          </button>
        </div>
      </header>

      <div ref={setNodeRef} className={"column-dropzone" + (isOver ? " column-dropzone-over" : "")} data-list-id={list.id}>
        <SortableContext items={tasks.map((task) => task.id)} strategy={verticalListSortingStrategy}>
          {tasks.length > 0 ? (
            tasks.map((task) => (
              <SortableTask
                key={task.id}
                task={task}
                users={users}
                assigneeIds={assigneesByTask[task.id] || []}
                lists={lists}
                editors={editingByTask[task.id] || []}
                onOpen={onOpenTask}
                onMove={onMoveTask}
                movePending={movePending}
              />
            ))
          ) : (
            <button className="column-empty" type="button" onClick={() => setQuickAddOpen(true)}>
              <Plus size={15} />
              <strong>Add your first card</strong>
              <span>Keep it short. Add details any time.</span>
            </button>
          )}
        </SortableContext>
      </div>

      {quickAddOpen ? (
        <form className="column-quick-add" onSubmit={submitQuickAdd}>
          <label className="sr-only" htmlFor={"quick-add-" + list.id}>New card title</label>
          <input
            ref={quickAddRef}
            id={"quick-add-" + list.id}
            value={draftTitle}
            onChange={(event) => setDraftTitle(event.target.value)}
            onKeyDown={(event) => { if (event.key === "Escape") { event.preventDefault(); cancelQuickAdd(); } }}
            placeholder="Write a card title…"
            maxLength={180}
            disabled={savingQuickAdd}
          />
          <div className="column-quick-add-actions">
            <button className="primary-button" type="submit" disabled={!draftTitle.trim() || savingQuickAdd}>{savingQuickAdd ? "Adding…" : "Add card"}</button>
            <button className="secondary-button" type="button" onClick={cancelQuickAdd} disabled={savingQuickAdd}>Cancel</button>
            <span>Enter to add · Esc to cancel</span>
          </div>
        </form>
      ) : (
        <button className="column-add-task" type="button" onClick={() => setQuickAddOpen(true)}>
          <Plus size={15} /> Add a card
        </button>
      )}
    </section>
  );
}

export default function BoardView({
  board,
  lists,
  tasks,
  users,
  taskAssignees,
  onOpenTask,
  onAddTask,
  onQuickAddTask,
  onMoveTask,
  onCreateList,
  onRenameList,
  onDeleteList,
  canManageLists,
  currentUserId,
  realtimeConnected = true,
  demoMode = false
}) {
  const [presenceUsers, setPresenceUsers] = useState([]);
  const [editingByTask, setEditingByTask] = useState({});
  const [movePending, setMovePending] = useState(false);
  const [activeTaskId, setActiveTaskId] = useState("");
  const editingTimeouts = useRef(new Map());

  useEffect(() => {
    if (demoMode) {
      setPresenceUsers(users.slice(0, 3).map((user) => ({ user_id: user.id, full_name: user.full_name })));
      return undefined;
    }
    let cancelled = false;
    const loadPresence = () => {
      presenceApi.list(board.id)
        .then((rows) => { if (!cancelled) setPresenceUsers(rows || []); })
        .catch(() => {});
    };
    loadPresence();
    const refreshInterval = window.setInterval(loadPresence, 30000);
    const unsubscribe = subscribeWorkspaceSocket((payload) => {
      if (payload.type !== "workspace_event" || !payload.event || payload.event.board_id !== board.id) return;
      const event = payload.event;
      if (event.event_type === "presence_joined" || event.event_type === "presence_left") loadPresence();
      if (!event.task_id || !event.actor_user_id || event.actor_user_id === currentUserId) return;
      if (event.event_type === "card_editing_started") {
        const user = users.find((row) => row.id === event.actor_user_id);
        if (!user) return;
        const editorKey = event.task_id + ":" + user.id;
        const previous = editingTimeouts.current.get(editorKey);
        if (previous) window.clearTimeout(previous);
        setEditingByTask((current) => ({
          ...current,
          [event.task_id]: [...(current[event.task_id] || []).filter((row) => row.id !== user.id), user]
        }));
        editingTimeouts.current.set(editorKey, window.setTimeout(() => {
          setEditingByTask((current) => ({
            ...current,
            [event.task_id]: (current[event.task_id] || []).filter((row) => row.id !== user.id)
          }));
          editingTimeouts.current.delete(editorKey);
        }, 16000));
      } else if (event.event_type === "card_editing_stopped") {
        const editorKey = event.task_id + ":" + event.actor_user_id;
        const timeout = editingTimeouts.current.get(editorKey);
        if (timeout) window.clearTimeout(timeout);
        editingTimeouts.current.delete(editorKey);
        setEditingByTask((current) => ({
          ...current,
          [event.task_id]: (current[event.task_id] || []).filter((row) => row.id !== event.actor_user_id)
        }));
      }
    });
    return () => {
      cancelled = true;
      window.clearInterval(refreshInterval);
      unsubscribe();
      editingTimeouts.current.forEach((timeout) => window.clearTimeout(timeout));
      editingTimeouts.current.clear();
    };
  }, [board.id, currentUserId, users, demoMode]);

  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 7 } }));
  const assigneesByTask = useMemo(() => {
    return taskAssignees.reduce((result, assignment) => {
      result[assignment.task_id] = result[assignment.task_id] || [];
      result[assignment.task_id].push(assignment.user_id);
      return result;
    }, {});
  }, [taskAssignees]);

  const activeTask = tasks.find((task) => task.id === activeTaskId) || null;

  function handleDragEnd(event) {
    const activeId = String(event.active.id);
    const overId = event.over ? String(event.over.id) : "";
    setActiveTaskId("");
    if (!overId || activeId === overId || movePending) return;

    let destinationListId = "";
    if (overId.startsWith("list:")) {
      destinationListId = overId.slice(5);
    } else {
      const overTask = tasks.find((task) => task.id === overId);
      destinationListId = overTask ? overTask.list_id : "";
    }
    if (!destinationListId) return;

    setMovePending(true);
    Promise.resolve(onMoveTask(activeId, destinationListId, overId))
      .catch(() => {})
      .finally(() => setMovePending(false));
  }

  return (
    <section className="board-page">
      <div className="board-page-heading">
        <div>
          <p className="eyebrow">PROJECT BOARD</p>
          <h1>{board.name}</h1>
          <p className="board-description">{board.description}</p>
        </div>
        <div className="board-members" aria-label="Active board viewers">
          {presenceUsers.slice(0, 5).map((user) => (
            <span className="board-presence-avatar" key={user.user_id} title={user.full_name + " is viewing this board"}>
              {initials(user.full_name)}<i />
            </span>
          ))}
          <span className="board-members-label">{presenceUsers.length} viewing</span>
        </div>
      </div>

      <div className="board-toolbar">
        <div className="board-toolbar-note">
          <span className={"live-dot" + (realtimeConnected ? "" : " live-dot-offline")} />
          <span className={"board-connection-pill" + (realtimeConnected ? " is-live" : " is-reconnecting")}>
            {demoMode ? "Demo workspace" : realtimeConnected ? "Live" : "Reconnecting…"}
          </span>
          {movePending ? "Saving card position…" : "Drag cards to update their list"}
        </div>
        <div className="board-toolbar-actions">
          {canManageLists && <button className="secondary-button" type="button" onClick={onCreateList}><Plus size={16} /> Add list</button>}
          <button className="primary-button" type="button" onClick={() => onAddTask(lists[0]?.id)}><Plus size={17} /> New card</button>
        </div>
      </div>

      <DndContext
        sensors={sensors}
        collisionDetection={closestCorners}
        onDragStart={(event) => setActiveTaskId(String(event.active.id))}
        onDragCancel={() => setActiveTaskId("")}
        onDragEnd={handleDragEnd}
      >
        <div className="kanban-columns" aria-label="Kanban board columns">
          {lists.map((list) => {
            const columnTasks = tasks
              .filter((task) => task.list_id === list.id)
              .sort((a, b) => a.position - b.position);
            return (
              <BoardColumn
                key={list.id}
                list={list}
                lists={lists}
                tasks={columnTasks}
                users={users}
                assigneesByTask={assigneesByTask}
                editingByTask={editingByTask}
                onOpenTask={onOpenTask}
                onAddTask={onAddTask}
                onQuickAddTask={onQuickAddTask}
                onMoveTask={onMoveTask}
                onRenameList={onRenameList}
                onDeleteList={onDeleteList}
                canManageLists={canManageLists}
                movePending={movePending}
              />
            );
          })}
        </div>
        <DragOverlay dropAnimation={null}>
          {activeTask ? (
            <article className="task-card task-card-overlay">
              <TaskCardFace
                task={activeTask}
                users={users}
                assigneeIds={assigneesByTask[activeTask.id] || []}
                lists={lists}
                editors={editingByTask[activeTask.id] || []}
                onOpen={onOpenTask}
                onMove={onMoveTask}
              />
            </article>
          ) : null}
        </DragOverlay>
      </DndContext>
    </section>
  );
}
