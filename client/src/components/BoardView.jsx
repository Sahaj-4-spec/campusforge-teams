import { useMemo } from "react";
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
import { CalendarDays, GripVertical, Pencil, Plus, Trash2, UserRound } from "lucide-react";

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

function SortableTask({ task, users, assigneeIds, onOpen }) {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id: task.id
  });
  const assignedUsers = assigneeIds
    .map((id) => users.find((user) => user.id === id))
    .filter(Boolean);

  return (
    <article
      ref={setNodeRef}
      style={{ transform: CSS.Transform.toString(transform), transition }}
      className={"task-card" + (isDragging ? " task-card-dragging" : "")}
    >
      <div className="task-card-topline">
        <span className={"priority-tag priority-" + task.priority}>{task.priority}</span>
        <button
          className="drag-handle"
          type="button"
          aria-label={"Drag " + task.title}
          {...attributes}
          {...listeners}
        >
          <GripVertical size={16} />
        </button>
      </div>
      <button className="task-title-button" type="button" onClick={() => onOpen(task)}>
        {task.title}
      </button>
      {task.description && <p className="task-description">{task.description}</p>}
      <div className="task-card-footer">
        <span className={"task-date" + (task.due_date && task.due_date < new Date().toISOString().slice(0, 10) ? " overdue" : "")}>
          <CalendarDays size={13} /> {formatDueDate(task.due_date)}
        </span>
        <div className="task-assignees" aria-label="Task assignees">
          {assignedUsers.length > 0
            ? assignedUsers.map((user) => (
                <span className="mini-avatar" key={user.id} title={user.full_name}>{initials(user.full_name)}</span>
              ))
            : <span className="unassigned-avatar" title="Unassigned"><UserRound size={12} /></span>}
        </div>
      </div>
    </article>
  );
}

function BoardColumn({ list, tasks, users, assigneesByTask, onOpenTask, onAddTask, onRenameList, onDeleteList }) {
  const { setNodeRef, isOver } = useDroppable({
    id: "list:" + list.id,
    data: { type: "column", listId: list.id }
  });

  return (
    <section className={"board-column" + (isOver ? " board-column-over" : "")}>
      <div className="column-heading">
        <div className="column-title-wrap">
          <span className={"column-dot dot-" + list.name.toLowerCase().replaceAll(" ", "-")} />
          <h3>{list.name}</h3>
          <span className="column-count">{tasks.length}</span>
        </div>
        <div className="column-actions">
          <button className="column-add" type="button" onClick={() => onRenameList(list)} aria-label={"Rename " + list.name} title="Rename column">
            <Pencil size={14} />
          </button>
          <button className="column-add column-delete" type="button" onClick={() => onDeleteList(list)} aria-label={"Delete " + list.name} title="Delete empty column">
            <Trash2 size={14} />
          </button>
          <button className="column-add" type="button" onClick={() => onAddTask(list.id)} aria-label={"Add a task to " + list.name} title="Add task">
            <Plus size={16} />
          </button>
        </div>
      </div>
      <div ref={setNodeRef} className="column-dropzone" data-list-id={list.id}>
        <SortableContext items={tasks.map((task) => task.id)} strategy={verticalListSortingStrategy}>
          {tasks.length > 0 ? (
            tasks.map((task) => (
              <SortableTask
                key={task.id}
                task={task}
                users={users}
                assigneeIds={assigneesByTask[task.id] || []}
                onOpen={onOpenTask}
              />
            ))
          ) : (
            <div className="column-empty">Drop a task here or add a new one.</div>
          )}
        </SortableContext>
      </div>
      <button className="column-add-task" type="button" onClick={() => onAddTask(list.id)}>
        <Plus size={15} /> Add task
      </button>
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
  onMoveTask,
  onCreateList,
  onRenameList,
  onDeleteList
}) {
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 7 } })
  );
  const assigneesByTask = useMemo(() => {
    return taskAssignees.reduce((result, assignment) => {
      result[assignment.task_id] = result[assignment.task_id] || [];
      result[assignment.task_id].push(assignment.user_id);
      return result;
    }, {});
  }, [taskAssignees]);

  function handleDragEnd(event) {
    const activeId = String(event.active.id);
    const overId = event.over ? String(event.over.id) : "";
    if (!overId || activeId === overId) return;

    let destinationListId = "";
    if (overId.startsWith("list:")) {
      destinationListId = overId.slice(5);
    } else {
      const overTask = tasks.find((task) => task.id === overId);
      destinationListId = overTask ? overTask.list_id : "";
    }

    if (destinationListId) onMoveTask(activeId, destinationListId, overId);
  }

  return (
    <section className="board-page">
      <div className="board-page-heading">
        <div>
          <p className="eyebrow">PROJECT BOARD</p>
          <h1>{board.name}</h1>
          <p className="board-description">{board.description}</p>
        </div>
        <div className="board-members">
          {users.slice(0, 4).map((user) => (
            <span className="mini-avatar" key={user.id} title={user.full_name}>{initials(user.full_name)}</span>
          ))}
          <span className="board-members-label">{users.length} members</span>
        </div>
      </div>

      <div className="board-toolbar">
        <div className="board-toolbar-note"><span className="live-dot" /> Workspace board <span className="toolbar-divider">·</span> Drag cards to update their list</div>
        <div className="board-toolbar-actions">
          <button className="secondary-button" type="button" onClick={onCreateList}>
            <Plus size={16} /> Add column
          </button>
          <button className="primary-button" type="button" onClick={() => onAddTask(lists[0]?.id)}>
            <Plus size={17} /> New task
          </button>
        </div>
      </div>

      <DndContext sensors={sensors} collisionDetection={closestCorners} onDragEnd={handleDragEnd}>
        <div className="kanban-columns">
          {lists.map((list) => {
            const columnTasks = tasks
              .filter((task) => task.list_id === list.id)
              .sort((a, b) => a.position - b.position);
            return (
              <BoardColumn
                key={list.id}
                list={list}
                tasks={columnTasks}
                users={users}
                assigneesByTask={assigneesByTask}
                onOpenTask={onOpenTask}
                onAddTask={onAddTask}
                onRenameList={onRenameList}
                onDeleteList={onDeleteList}
              />
            );
          })}
        </div>
        <DragOverlay dropAnimation={null}>
          {null}
        </DragOverlay>
      </DndContext>
    </section>
  );
}
