import test from "node:test";
import assert from "node:assert/strict";
import { searchCommandItems } from "./commandSearch.js";

test("ranks exact and prefix matches ahead of partial matches", () => {
  const items = [
    { id: "partial", label: "Team project dashboard" },
    { id: "prefix", label: "Projects" },
    { id: "exact", label: "Project" }
  ];
  assert.deepEqual(
    searchCommandItems(items, "project").map((item) => item.id),
    ["exact", "prefix", "partial"]
  );
});

test("matches descriptions and keywords without case sensitivity", () => {
  const items = [
    { id: "calendar", label: "Calendar", description: "Plan milestones", keywords: ["deadlines"] },
    { id: "locker", label: "Save Locker", description: "Private storage" }
  ];
  assert.deepEqual(searchCommandItems(items, "DEADLINE").map((item) => item.id), ["calendar"]);
});

test("returns stable defaults and respects the result limit", () => {
  const items = Array.from({ length: 15 }, (_, index) => ({ id: String(index), label: "Task " + index }));
  assert.deepEqual(searchCommandItems(items, "").map((item) => item.id), items.slice(0, 10).map((item) => item.id));
  assert.equal(searchCommandItems(items, "task", 4).length, 4);
});
