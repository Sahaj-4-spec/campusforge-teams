const TOKEN_KEY = "campusforge-auth-token";
const API_BASE = (import.meta.env.VITE_API_BASE_URL || "").replace(/\/$/, "");

export function getStoredToken() {
  return window.localStorage.getItem(TOKEN_KEY);
}

export function storeToken(token) {
  window.localStorage.setItem(TOKEN_KEY, token);
}

export function clearStoredToken() {
  window.localStorage.removeItem(TOKEN_KEY);
}

async function request(path, options = {}) {
  const headers = new Headers(options.headers || {});
  headers.set("Accept", "application/json");

  if (options.body !== undefined) {
    headers.set("Content-Type", "application/json");
  }

  const token = getStoredToken();
  if (token) headers.set("Authorization", `Bearer ${token}`);

  let response;
  try {
    response = await fetch(`${API_BASE}${path}`, {
      ...options,
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
    const error = new Error(payload?.error || `Request failed (${response.status})`);
    error.status = response.status;
    throw error;
  }

  return payload;
}

export const authApi = {
  register: (values) => request("/api/auth/register", { method: "POST", body: values }),
  login: (values) => request("/api/auth/login", { method: "POST", body: values })
};

export const workspaceApi = {
  get: () => request("/api/workspace")
};

export const boardApi = {
  create: (values) => request("/api/boards", { method: "POST", body: values })
};

export const taskApi = {
  create: (values) => request("/api/tasks", { method: "POST", body: values }),
  update: (taskId, values) => request(`/api/tasks/${encodeURIComponent(taskId)}`, { method: "PATCH", body: values }),
  remove: (taskId) => request(`/api/tasks/${encodeURIComponent(taskId)}`, { method: "DELETE" }),
  addComment: (taskId, body) => request(`/api/tasks/${encodeURIComponent(taskId)}/comments`, {
    method: "POST",
    body: { body }
  }),
  move: (taskId, destinationListId, orderedTaskIds) => request(
    `/api/tasks/${encodeURIComponent(taskId)}/move`,
    { method: "PATCH", body: { destination_list_id: destinationListId, ordered_task_ids: orderedTaskIds } }
  )
};
