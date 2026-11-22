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
  if (options.body !== undefined) headers.set("Content-Type", "application/json");
  const token = getStoredToken();
  if (token) headers.set("Authorization", "Bearer " + token);

  let response;
  try {
    response = await fetch(API_BASE + path, {
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
    try { payload = JSON.parse(raw); }
    catch { payload = { error: raw }; }
  }
  if (!response.ok) {
    const error = new Error(payload?.error || "Request failed (" + response.status + ")");
    error.status = response.status;
    throw error;
  }
  return payload;
}

async function authorizedFetch(path, options = {}) {
  const headers = new Headers(options.headers || {});
  const token = getStoredToken();
  headers.set("Accept", options.accept || "application/json");
  if (token) headers.set("Authorization", "Bearer " + token);
  try {
    const response = await fetch(API_BASE + path, { ...options, headers });
    if (!response.ok) {
      let message = "Request failed (" + response.status + ")";
      try {
        const payload = await response.json();
        message = payload.error || message;
      } catch {}
      const error = new Error(message);
      error.status = response.status;
      throw error;
    }
    return response;
  } catch (error) {
    if (error instanceof TypeError) {
      throw new Error("Could not reach the CampusForge API. Check your connection and API URL.");
    }
    throw error;
  }
}

export const authApi = {
  register: (values) => request("/api/auth/register", { method: "POST", body: values }),
  login: (values) => request("/api/auth/login", { method: "POST", body: values }),
  oauthStartUrl: (provider) => (API_BASE || window.location.origin) + "/api/auth/oauth/" + encodeURIComponent(provider) + "/start"
};

export const workspaceApi = {
  get: () => request("/api/workspace")
};

export const boardApi = {
  create: (values) => request("/api/boards", { method: "POST", body: values })
};

export const listApi = {
  create: (boardId, name) => request("/api/boards/" + encodeURIComponent(boardId) + "/lists", { method: "POST", body: { name } }),
  update: (listId, name) => request("/api/lists/" + encodeURIComponent(listId), { method: "PATCH", body: { name } }),
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
  addComment: (taskId, body) => request("/api/tasks/" + encodeURIComponent(taskId) + "/comments", { method: "POST", body: { body } }),
  move: (taskId, destinationListId, orderedTaskIds) => request(
    "/api/tasks/" + encodeURIComponent(taskId) + "/move",
    { method: "PATCH", body: { destination_list_id: destinationListId, ordered_task_ids: orderedTaskIds } }
  )
};

export const advancedTaskApi = {
  details: (taskId) => request("/api/tasks/" + encodeURIComponent(taskId) + "/advanced"),
  createSubtask: (taskId, values) => request("/api/tasks/" + encodeURIComponent(taskId) + "/subtasks", { method: "POST", body: values }),
  updateSubtask: (subtaskId, values) => request("/api/subtasks/" + encodeURIComponent(subtaskId), { method: "PATCH", body: values }),
  removeSubtask: (subtaskId) => request("/api/subtasks/" + encodeURIComponent(subtaskId), { method: "DELETE" }),
  checklist: (taskId) => request("/api/tasks/" + encodeURIComponent(taskId) + "/checklist"),
  addChecklistItem: (taskId, title) => request("/api/tasks/" + encodeURIComponent(taskId) + "/checklist", { method: "POST", body: { title } }),
  updateChecklistItem: (itemId, values) => request("/api/checklist-items/" + encodeURIComponent(itemId), { method: "PATCH", body: values }),
  removeChecklistItem: (itemId) => request("/api/checklist-items/" + encodeURIComponent(itemId), { method: "DELETE" }),
  labels: () => request("/api/labels"),
  createLabel: (values) => request("/api/labels", { method: "POST", body: values }),
  addLabel: (taskId, labelId) => request("/api/tasks/" + encodeURIComponent(taskId) + "/labels/" + encodeURIComponent(labelId), { method: "POST" }),
  removeLabel: (taskId, labelId) => request("/api/tasks/" + encodeURIComponent(taskId) + "/labels/" + encodeURIComponent(labelId), { method: "DELETE" }),
  addDependency: (taskId, dependsOnTaskId) => request("/api/tasks/" + encodeURIComponent(taskId) + "/dependencies", { method: "POST", body: { depends_on_task_id: dependsOnTaskId } }),
  removeDependency: (taskId, dependsOnTaskId) => request("/api/tasks/" + encodeURIComponent(taskId) + "/dependencies/" + encodeURIComponent(dependsOnTaskId), { method: "DELETE" }),
  updateEstimate: (taskId, estimatedMinutes) => request("/api/tasks/" + encodeURIComponent(taskId) + "/estimate", { method: "PATCH", body: { estimated_minutes: estimatedMinutes } }),
  updateRecurrence: (taskId, values) => request("/api/tasks/" + encodeURIComponent(taskId) + "/recurrence", { method: "PUT", body: values })
};

export const calendarApi = {
  list: (from, to) => {
    const [fromYear, fromMonth, fromDay] = from.split("-").map(Number);
    const [toYear, toMonth, toDay] = to.split("-").map(Number);
    const localStart = new Date(fromYear, fromMonth - 1, fromDay);
    const localEnd = new Date(toYear, toMonth - 1, toDay);
    localEnd.setDate(localEnd.getDate() + 1);
    const params = new URLSearchParams({
      from,
      to,
      start_at: localStart.toISOString(),
      end_at: localEnd.toISOString()
    });
    return request("/api/calendar?" + params.toString());
  },
  create: (values) => request("/api/calendar", { method: "POST", body: values }),
  update: (eventId, values) => request("/api/calendar/events/" + encodeURIComponent(eventId), { method: "PATCH", body: values }),
  remove: (eventId) => request("/api/calendar/events/" + encodeURIComponent(eventId), { method: "DELETE" })
};

export const notificationApi = {
  list: () => request("/api/notifications"),
  read: (notificationId) => request("/api/notifications/" + encodeURIComponent(notificationId) + "/read", { method: "PATCH" }),
  readAll: () => request("/api/notifications/read-all", { method: "POST", body: {} }),
  preferences: () => request("/api/notification-preferences"),
  updatePreference: (notificationType, inAppEnabled, emailEnabled) => request("/api/notification-preferences", {
    method: "PUT", body: {
      notification_type: notificationType,
      in_app_enabled: inAppEnabled,
      email_enabled: emailEnabled
    }
  })
};

export const filesApi = {
  list: () => request("/api/files"),
  upload: async (formData) => {
    const response = await authorizedFetch("/api/files", { method: "POST", body: formData });
    return response.json();
  },
  download: async (fileId, filename) => {
    const response = await authorizedFetch("/api/files/" + encodeURIComponent(fileId) + "/download", { accept: "*/*" });
    const blob = await response.blob();
    const url = window.URL.createObjectURL(blob);
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = filename || "campusforge-file";
    document.body.appendChild(anchor);
    anchor.click();
    anchor.remove();
    window.URL.revokeObjectURL(url);
  },
  remove: (fileId) => request("/api/files/" + encodeURIComponent(fileId), { method: "DELETE" })
};

export const chatApi = {
  list: (boardId) => request("/api/chat/messages" + (boardId ? "?board_id=" + encodeURIComponent(boardId) : "")),
  send: (values) => request("/api/chat/messages", { method: "POST", body: values })
};

export const meetingNotesApi = {
  list: () => request("/api/meeting-notes"),
  create: (values) => request("/api/meeting-notes", { method: "POST", body: values }),
  update: (noteId, values) => request("/api/meeting-notes/" + encodeURIComponent(noteId), { method: "PATCH", body: values }),
  remove: (noteId) => request("/api/meeting-notes/" + encodeURIComponent(noteId), { method: "DELETE" })
};

export const analyticsApi = {
  get: () => request("/api/analytics")
};

export function createWorkspaceSocket(token) {
  const origin = API_BASE || window.location.origin;
  const websocketOrigin = origin.replace(/^https:/i, "wss:").replace(/^http:/i, "ws:");
  const socket = new WebSocket(websocketOrigin + "/ws");
  socket.addEventListener("open", () => {
    socket.send(JSON.stringify({ type: "authenticate", token }));
  });
  return socket;
}
