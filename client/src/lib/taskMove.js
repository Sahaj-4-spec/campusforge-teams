/**
 * Calculate the next task order before the server confirms a Kanban move.
 * Positions are normalized to a one-based sequence, matching the API.
 */
export function buildOptimisticTaskMove(tasks, taskId, destinationListId, overId) {
  const movingTask = tasks.find((task) => task.id === taskId);
  if (!movingTask || !destinationListId) return null;

  const sourceListId = movingTask.list_id;
  const sourceRemaining = sourceListId === destinationListId
    ? []
    : tasks
      .filter((task) => task.list_id === sourceListId && task.id !== taskId)
      .sort((a, b) => a.position - b.position);

  const destinationTasks = tasks
    .filter((task) => task.list_id === destinationListId && task.id !== taskId)
    .sort((a, b) => a.position - b.position);

  const overIndex = overId?.startsWith("list:")
    ? -1
    : destinationTasks.findIndex((task) => task.id === overId);
  const insertAt = overIndex >= 0 ? overIndex : destinationTasks.length;
  destinationTasks.splice(insertAt, 0, movingTask);

  const updatedById = new Map();
  sourceRemaining.forEach((task, index) => {
    updatedById.set(task.id, { ...task, position: index + 1 });
  });
  destinationTasks.forEach((task, index) => {
    updatedById.set(task.id, {
      ...task,
      list_id: destinationListId,
      position: index + 1
    });
  });

  return {
    sourceListId,
    destinationListId,
    orderedTaskIds: destinationTasks.map((task) => task.id),
    tasks: tasks.map((task) => updatedById.get(task.id) || task)
  };
}
