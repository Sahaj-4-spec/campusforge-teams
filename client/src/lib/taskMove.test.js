import test from "node:test";
import assert from "node:assert/strict";
import { buildOptimisticTaskMove } from "./taskMove.js";

const tasks = [
  { id: "a", list_id: "todo", position: 1, title: "A" },
  { id: "b", list_id: "todo", position: 2, title: "B" },
  { id: "c", list_id: "todo", position: 3, title: "C" },
  { id: "d", list_id: "doing", position: 1, title: "D" },
  { id: "e", list_id: "doing", position: 2, title: "E" },
  { id: "f", list_id: "done", position: 1, title: "F" }
];

function listOrder(result, listId) {
  return result.tasks
    .filter((task) => task.list_id === listId)
    .sort((a, b) => a.position - b.position)
    .map((task) => task.id);
}

test("reorders cards in the same list immediately", () => {
  const result = buildOptimisticTaskMove(tasks, "a", "todo", "c");
  assert.deepEqual(result.orderedTaskIds, ["b", "a", "c"]);
  assert.deepEqual(listOrder(result, "todo"), ["b", "a", "c"]);
  assert.deepEqual(listOrder(result, "doing"), ["d", "e"]);
});

test("moves a card across lists and normalizes both lists", () => {
  const result = buildOptimisticTaskMove(tasks, "b", "doing", "e");
  assert.deepEqual(result.orderedTaskIds, ["d", "b", "e"]);
  assert.deepEqual(listOrder(result, "todo"), ["a", "c"]);
  assert.deepEqual(listOrder(result, "doing"), ["d", "b", "e"]);
  assert.equal(result.tasks.find((task) => task.id === "b").list_id, "doing");
});

test("dropping on a column appends the card", () => {
  const result = buildOptimisticTaskMove(tasks, "a", "doing", "list:doing");
  assert.deepEqual(result.orderedTaskIds, ["d", "e", "a"]);
  assert.deepEqual(listOrder(result, "todo"), ["b", "c"]);
  assert.deepEqual(listOrder(result, "doing"), ["d", "e", "a"]);
});

test("returns null when the dragged task is no longer present", () => {
  assert.equal(buildOptimisticTaskMove(tasks, "missing", "doing", "list:doing"), null);
});
