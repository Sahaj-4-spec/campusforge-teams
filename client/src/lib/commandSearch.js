export function searchCommandItems(items, query, limit = 10) {
  const normalizedQuery = String(query || "").trim().toLocaleLowerCase();
  if (!normalizedQuery) return items.slice(0, limit);

  function score(value) {
    const normalizedValue = String(value || "").trim().toLocaleLowerCase();
    if (!normalizedValue) return 0;
    if (normalizedValue === normalizedQuery) return 120;
    if (normalizedValue.startsWith(normalizedQuery)) return 90;
    const wordMatch = normalizedValue.split(/\s+/).some((word) => word.startsWith(normalizedQuery));
    if (wordMatch) return 72;
    const position = normalizedValue.indexOf(normalizedQuery);
    return position >= 0 ? Math.max(1, 50 - position) : 0;
  }

  return items
    .map((item, index) => {
      const fields = [item.label, item.description, ...(item.keywords || [])];
      return { item, index, score: Math.max(0, ...fields.map(score)) };
    })
    .filter((entry) => entry.score > 0)
    .sort((left, right) => right.score - left.score || left.index - right.index)
    .slice(0, limit)
    .map((entry) => entry.item);
}
