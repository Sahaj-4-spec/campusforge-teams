const WORKSPACE_KEY = "campusforge-active-workspace";
const TAB_WORKSPACE_KEY = "campusforge-tab-active-workspace";
const API_BASE = (import.meta.env.VITE_API_BASE_URL || "").replace(/\/$/, "");

// Remove JWTs saved by older CampusForge builds before using cookie-based sessions.
try {
  window.localStorage.removeItem("campusforge-auth-token");
} catch {
  // Storage may be unavailable in strict browser privacy modes.
}

export function getStoredWorkspaceId() {
  return window.sessionStorage.getItem(TAB_WORKSPACE_KEY) || window.localStorage.getItem(WORKSPACE_KEY);
}

export function setStoredWorkspaceId(workspaceId) {
  if (workspaceId) {
    window.sessionStorage.setItem(TAB_WORKSPACE_KEY, workspaceId);
    window.localStorage.setItem(WORKSPACE_KEY, workspaceId);
  } else {
    window.sessionStorage.removeItem(TAB_WORKSPACE_KEY);
    window.localStorage.removeItem(WORKSPACE_KEY);
  }
}

export function clearStoredWorkspaceId() {
  window.sessionStorage.removeItem(TAB_WORKSPACE_KEY);
  window.localStorage.removeItem(WORKSPACE_KEY);
}

async function request(path, options = {}) {
  const headers = new Headers(options.headers || {});
  headers.set("Accept", "application/json");
  if (options.body !== undefined) headers.set("Content-Type", "application/json");
  if (path !== "/api/workspace" && !headers.has("X-Workspace-ID")) {
    const workspaceId = getStoredWorkspaceId();
    if (workspaceId) headers.set("X-Workspace-ID", workspaceId);
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
  headers.set("Accept", options.accept || "application/json");
  const workspaceId = getStoredWorkspaceId();
  if (workspaceId) headers.set("X-Workspace-ID", workspaceId);
  try {
    const { accept, ...fetchOptions } = options;
    const response = await fetch(API_BASE + path, {
      ...fetchOptions,
      credentials: "include",
      headers
    });
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
  login: (values) => request("/api/auth/login", { method: "POST", body: values }),
  logout: () => request("/api/auth/logout", { method: "POST" }),
  oauthStartUrl: (provider) => (API_BASE || window.location.origin) + "/api/auth/" + encodeURIComponent(provider) + "/start"
};

export const aiApi = {
  status: () => request("/api/ai/status"),
  ask: (values) => request("/api/ai/assistant", { method: "POST", body: values })
};

export const workspaceApi = {
  get: (workspaceId = getStoredWorkspaceId()) => request("/api/workspace", {
    headers: workspaceId ? { "X-Workspace-ID": workspaceId } : {}
  })
};

export const boardApi = {
  create: (values, workspaceId) => request("/api/boards", { method: "POST", body: values, headers: workspaceId ? { "X-Workspace-ID": workspaceId } : {} })
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
  updateMemberRole: (userId, role) => request("/api/invitations/members/" + encodeURIComponent(userId) + "/role", { method: "PATCH", body: { role } }),
  accept: (token) => request("/api/invitations/accept", { method: "POST", body: { token } })
};

export const taskApi = {
  create: (values) => request("/api/tasks", { method: "POST", body: values }),
  update: (taskId, values) => request("/api/tasks/" + encodeURIComponent(taskId), { method: "PATCH", body: values }),
  remove: (taskId) => request("/api/tasks/" + encodeURIComponent(taskId), { method: "DELETE" }),
  addComment: (taskId, body, parentCommentId = null) => request("/api/tasks/" + encodeURIComponent(taskId) + "/comments", {
    method: "POST", body: { body, parent_comment_id: parentCommentId }
  }),
  move: (taskId, destinationListId, orderedTaskIds) => request(
    "/api/tasks/" + encodeURIComponent(taskId) + "/move",
    { method: "PATCH", body: { destination_list_id: destinationListId, ordered_task_ids: orderedTaskIds } }
  ),
  search: (filters = {}) => {
    const params = new URLSearchParams();
    Object.entries(filters).forEach(([key, value]) => {
      if (value !== undefined && value !== null && value !== "") params.set(key, String(value));
    });
    return request("/api/tasks/search" + (params.size ? "?" + params.toString() : ""));
  }
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
  list: (from, to, filters = {}) => {
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
    Object.entries(filters).forEach(([key, value]) => {
      if (value !== undefined && value !== null && value !== "") params.set(key, String(value));
    });
    return request("/api/calendar?" + params.toString());
  },
  create: (values) => request("/api/calendar", { method: "POST", body: values }),
  update: (eventId, values) => request("/api/calendar/events/" + encodeURIComponent(eventId), { method: "PATCH", body: values }),
  attendees: (eventId, userIds) => request("/api/calendar/events/" + encodeURIComponent(eventId) + "/attendees", { method: "PUT", body: { user_ids: userIds } }),
  respond: (eventId, response) => request("/api/calendar/events/" + encodeURIComponent(eventId) + "/respond", { method: "POST", body: { response } }),
  remove: (eventId) => request("/api/calendar/events/" + encodeURIComponent(eventId), { method: "DELETE" })
};

export const notificationApi = {
  list: (before, limit = 50) => {
    const params = new URLSearchParams({ limit: String(limit) });
    if (before) params.set("before", before);
    return request("/api/notifications?" + params.toString());
  },
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
  usage: () => request("/api/files/usage"),
  setQuota: (quotaBytes) => request("/api/files/quota", { method: "PUT", body: { quota_bytes: quotaBytes } }),
  audit: () => request("/api/files/audit"),
  preview: async (fileId) => {
    const response = await authorizedFetch("/api/files/" + encodeURIComponent(fileId) + "/preview", { accept: "*/*" });
    return URL.createObjectURL(await response.blob());
  },
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

export const lockerApi = {
  getVault: () => request("/api/locker/vault"),
  createVault: (values) => request("/api/locker/vault", { method: "POST", body: values }),
  deleteVault: () => request("/api/locker/vault", { method: "DELETE" }),
  listItems: () => request("/api/locker/items"),
  createItem: (values) => request("/api/locker/items", { method: "POST", body: values }),
  updateItem: (itemId, values) => request("/api/locker/items/" + encodeURIComponent(itemId), { method: "PUT", body: values }),
  removeItem: (itemId) => request("/api/locker/items/" + encodeURIComponent(itemId), { method: "DELETE" })
};

export const chatApi = {
  list: (boardId, before, limit = 50) => {
    const params = new URLSearchParams({ limit: String(limit) });
    if (boardId) params.set("board_id", boardId);
    if (before) params.set("before", before);
    return request("/api/chat/messages?" + params.toString());
  },
  send: (values) => request("/api/chat/messages", { method: "POST", body: values }),
  directConversations: () => request("/api/chat/direct"),
  openDirect: (otherUserId) => request("/api/chat/direct", { method: "POST", body: { other_user_id: otherUserId } }),
  directMessages: (conversationId, before, limit = 50) => {
    const params = new URLSearchParams({ limit: String(limit) });
    if (before) params.set("before", before);
    return request("/api/chat/direct/" + encodeURIComponent(conversationId) + "/messages?" + params.toString());
  },
  sendDirect: (conversationId, body) => request("/api/chat/direct/" + encodeURIComponent(conversationId) + "/messages", { method: "POST", body: { body } }),
  edit: (messageId, body) => request("/api/chat/messages/" + encodeURIComponent(messageId), { method: "PATCH", body: { body } }),
  remove: (messageId) => request("/api/chat/messages/" + encodeURIComponent(messageId), { method: "DELETE" }),
  pin: (messageId, pinned) => request("/api/chat/messages/" + encodeURIComponent(messageId) + "/pin", { method: "PATCH", body: { pinned } }),
  pinned: (boardId) => request("/api/chat/pinned" + (boardId ? "?board_id=" + encodeURIComponent(boardId) : ""))
};

export const meetingNotesApi = {
  list: () => request("/api/meeting-notes"),
  create: (values) => request("/api/meeting-notes", { method: "POST", body: values }),
  update: (noteId, values) => request("/api/meeting-notes/" + encodeURIComponent(noteId), { method: "PATCH", body: values }),
  remove: (noteId) => request("/api/meeting-notes/" + encodeURIComponent(noteId), { method: "DELETE" }),
  actionItems: (noteId) => request("/api/meeting-notes/" + encodeURIComponent(noteId) + "/action-items"),
  createActionItem: (noteId, values) => request("/api/meeting-notes/" + encodeURIComponent(noteId) + "/action-items", { method: "POST", body: values }),
  updateActionItem: (itemId, values) => request("/api/meeting-action-items/" + encodeURIComponent(itemId), { method: "PATCH", body: values }),
  removeActionItem: (itemId) => request("/api/meeting-action-items/" + encodeURIComponent(itemId), { method: "DELETE" })
};

export const presenceApi = {
  list: (boardId) => request("/api/presence" + (boardId ? "?board_id=" + encodeURIComponent(boardId) : ""))
};

export const analyticsApi = {
  get: () => request("/api/analytics"),
  history: (from, to) => {
    const params = new URLSearchParams({ from, to });
    return request("/api/analytics/history?" + params.toString());
  },
  exportCsv: async (from, to) => {
    const params = new URLSearchParams({ from, to });
    const response = await authorizedFetch("/api/analytics/export.csv?" + params.toString(), { accept: "text/csv" });
    const blob = await response.blob();
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = "campusforge-analytics.csv";
    document.body.appendChild(anchor);
    anchor.click();
    anchor.remove();
    URL.revokeObjectURL(url);
  }
};

let activeWorkspaceSocket = null;
const workspaceSocketListeners = new Set();

export function subscribeWorkspaceSocket(listener) {
  workspaceSocketListeners.add(listener);
  return () => workspaceSocketListeners.delete(listener);
}

export function sendWorkspaceSocketMessage(payload) {
  if (!activeWorkspaceSocket || activeWorkspaceSocket.readyState !== WebSocket.OPEN) return false;
  activeWorkspaceSocket.send(JSON.stringify(payload));
  return true;
}

let desiredWorkspaceBoardId = null;

export function setWorkspaceSocketBoard(boardId) {
  desiredWorkspaceBoardId = boardId || null;
  sendWorkspaceSocketMessage({ type: "board_view", board_id: desiredWorkspaceBoardId });
}

export function createWorkspaceSocket() {
  const origin = API_BASE || window.location.origin;
  const websocketOrigin = origin.replace(/^https:/i, "wss:").replace(/^http:/i, "ws:");
  const socket = new WebSocket(websocketOrigin + "/ws");
  activeWorkspaceSocket = socket;
  socket.addEventListener("open", () => {
    socket.send(JSON.stringify({
      type: "authenticate",
      workspace_id: getStoredWorkspaceId()
    }));
  });
  socket.addEventListener("message", (message) => {
    let payload;
    try { payload = JSON.parse(message.data); } catch { return; }
    if (payload.type === "ready") {
      socket.send(JSON.stringify({ type: "board_view", board_id: desiredWorkspaceBoardId }));
    }
    workspaceSocketListeners.forEach((listener) => {
      try { listener(payload); } catch {}
    });
  });
  socket.addEventListener("close", () => {
    if (activeWorkspaceSocket === socket) activeWorkspaceSocket = null;
    workspaceSocketListeners.forEach((listener) => {
      try { listener({ type: "connection_state", connected: false }); } catch {}
    });
  });
  return socket;
}
