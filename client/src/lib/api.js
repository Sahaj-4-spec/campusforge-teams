const API_BASE = (import.meta.env.VITE_API_BASE_URL || "").replace(/\/$/, "");

async function request(path, options = {}) {
  const headers = new Headers(options.headers || {});
  headers.set("Accept", "application/json");

  if (options.body !== undefined) {
    headers.set("Content-Type", "application/json");
  }


  let response;
  try {
    response = await fetch(API_BASE + path, {
      ...options,
      credentials: "include",
      headers,
      body: options.body === undefined ? undefined : JSON.stringify(options.body)
    });
  } catch {
    throw new Error("Could not reach the CampusForge API. Check your connection and API URL.");
  }

  const raw = await response.text();
  let payload = null;
  if (raw) {
    try {
      payload = JSON.parse(raw);
    } catch {
      payload = { error: raw };
    }
  }

  if (!response.ok) {
    const error = new Error(payload?.error || "Request failed (" + response.status + ")");
    error.status = response.status;
    throw error;
  }

  return payload;
}

export const authApi = {
  register: (values) => request("/api/auth/register", { method: "POST", body: values }),
  login: (values) => request("/api/auth/login", { method: "POST", body: values }),
  logout: () => request("/api/auth/logout", { method: "POST" }),
  oauthStartUrl: (provider) => (API_BASE || window.location.origin) + "/api/auth/oauth/" + encodeURIComponent(provider) + "/start"
};

export const workspaceApi = {
  get: () => request("/api/workspace")
};

export const boardApi = {
  create: (values) => request("/api/boards", { method: "POST", body: values })
};

export const listApi = {
  create: (boardId, name) => request("/api/boards/" + encodeURIComponent(boardId) + "/lists", {
    method: "POST",
    body: { name }
  }),
  update: (listId, name) => request("/api/lists/" + encodeURIComponent(listId), {
    method: "PATCH",
    body: { name }
  }),
  remove: (listId) => request("/api/lists/" + encodeURIComponent(listId), { method: "DELETE" })
};

export const invitationApi = {
  list: () => request("/api/invitations/"),
  create: (values) => request("/api/invitations/", { method: "POST", body: values }),
  revoke: (invitationId) => request("/api/invitations/" + encodeURIComponent(invitationId), { method: "DELETE" }),
  accept: (token) => request("/api/invitations/accept", { method: "POST", body: { token } })
};

export const taskApi = {
  create: (values) => request("/api/tasks", { method: "POST", body: values }),
  update: (taskId, values) => request("/api/tasks/" + encodeURIComponent(taskId), { method: "PATCH", body: values }),
  remove: (taskId) => request("/api/tasks/" + encodeURIComponent(taskId), { method: "DELETE" }),
  addComment: (taskId, body) => request("/api/tasks/" + encodeURIComponent(taskId) + "/comments", {
    method: "POST",
    body: { body }
  }),
  move: (taskId, destinationListId, orderedTaskIds) => request(
    "/api/tasks/" + encodeURIComponent(taskId) + "/move",
    { method: "PATCH", body: { destination_list_id: destinationListId, ordered_task_ids: orderedTaskIds } }
  )
};

export function createWorkspaceSocket() {
  const origin = API_BASE || window.location.origin;
  const websocketOrigin = origin
    .replace(/^https:/i, "wss:")
    .replace(/^http:/i, "ws:");
  return new WebSocket(websocketOrigin + "/ws");
}
