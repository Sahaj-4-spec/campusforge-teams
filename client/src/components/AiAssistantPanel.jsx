import { useEffect, useMemo, useRef, useState } from "react";
import {
  AlertCircle,
  ArrowUpRight,
  BookOpen,
  BrainCircuit,
  CheckCircle2,
  CalendarDays,
  CircleHelp,
  Clock3,
  FileText,
  Files,
  Lightbulb,
  LoaderCircle,
  LockKeyhole,
  MessageSquare,
  MessagesSquare,
  Send,
  ShieldCheck,
  Sparkles,
  Target,
  X
} from "lucide-react";
import { aiApi, taskApi } from "../lib/api.js";

const MODES = [
  { id: "ask", label: "Ask", icon: MessageSquare },
  { id: "summary", label: "Summary", icon: FileText },
  { id: "task_breakdown", label: "Task plan", icon: Target },
  { id: "risk_review", label: "Risk review", icon: ShieldCheck },
  { id: "meeting_summary", label: "Meeting notes", icon: CalendarDays },
  { id: "chat_summary", label: "Team chat", icon: MessagesSquare },
  { id: "file_summary", label: "Document", icon: Files }
];

const SUMMARIZABLE_FILE_TYPES = new Set([
  "text/plain", "text/markdown", "text/csv", "text/tab-separated-values",
  "text/xml", "application/xml", "application/json", "application/yaml",
  "text/yaml", "application/toml", "text/javascript", "application/javascript",
  "text/typescript", "text/css", "text/x-python", "text/x-rust"
]);

function canSummarizeFile(file) {
  return file
    && file.scan_status === "clean"
    && SUMMARIZABLE_FILE_TYPES.has(String(file.media_type || "").toLowerCase())
    && Number(file.size_bytes) > 0
    && Number(file.size_bytes) <= 128 * 1024;
}

const STARTERS = [
  { label: "What's blocking progress?", prompt: "Review the current project records and identify the most important blockers. Cite the tasks that support each finding." },
  { label: "Summarize this project", prompt: "Give me a concise project status summary, including urgent work, overdue tasks, and the next useful steps." },
  { label: "Break down a deliverable", prompt: "Propose a practical task breakdown for the next major deliverable based on the current project context. Do not create any tasks." }
];

function parseProposal(content) {
  try {
    const value = JSON.parse(content);
    if (!value || !Array.isArray(value.tasks)) return null;
    return value;
  } catch {
    return null;
  }
}

function ProposalTaskCard({ task, index, onCreateTask }) {
  const [saving, setSaving] = useState(false);
  const [created, setCreated] = useState(false);
  const [error, setError] = useState("");

  async function createTask() {
    if (saving || created || !onCreateTask) return;
    setSaving(true);
    setError("");
    try {
      await onCreateTask(task);
      setCreated(true);
    } catch (requestError) {
      setError(requestError.message || "Could not create this task.");
    } finally {
      setSaving(false);
    }
  }

  return (
    <article className="ai-proposal-card" key={task.title + "-" + index}>
      <div className="ai-proposal-card-top">
        <span className="ai-proposal-number">{index + 1}</span>
        <strong>{task.title}</strong>
        <span className={"ai-priority ai-priority-" + (["low", "medium", "high"].includes(String(task.priority).toLowerCase()) ? String(task.priority).toLowerCase() : "medium")}>{task.priority}</span>
      </div>
      {task.description && <p>{task.description}</p>}
      <div className="ai-proposal-meta">
        {Number.isFinite(Number(task.estimated_minutes)) && <span><Clock3 size={12} /> {task.estimated_minutes} min estimate</span>}
        {task.reason && <span>{task.reason}</span>}
      </div>
      {Array.isArray(task.source_refs) && task.source_refs.length > 0 && (
        <div className="ai-source-references">{task.source_refs.slice(0, 6).map((reference) => <span className="ai-source-chip" key={reference}>{reference}</span>)}</div>
      )}
      <div className="ai-proposal-actions">
        <button className="ai-proposal-create" type="button" onClick={createTask} disabled={saving || created || !onCreateTask}>
          {created ? <><CheckCircle2 size={13} /> Created</> : saving ? <><LoaderCircle size={13} className="ai-spin" /> Creating…</> : <>Create task <ArrowUpRight size={13} /></>}
        </button>
        {error && <span className="ai-proposal-error" role="alert">{error}</span>}
      </div>
    </article>
  );
}

function AssistantAnswer({ message, onCreateTask }) {
  const proposal = message.mode === "task_breakdown" ? parseProposal(message.content) : null;
  const proposalTasks = proposal && Array.isArray(proposal.tasks)
    ? proposal.tasks.filter((task) => task && typeof task === "object" && typeof task.title === "string").slice(0, 8)
    : [];

  return (
    <div className="ai-answer-body">
      {proposal ? (
        <>
          {proposal.summary && <p className="ai-proposal-summary">{proposal.summary}</p>}
          <div className="ai-proposal-list">
            {proposalTasks.map((task, index) => (
              <ProposalTaskCard task={task} index={index} onCreateTask={onCreateTask} key={task.title + "-" + index} />
            ))}
          </div>
          <p className="ai-proposal-disclaimer"><LockKeyhole size={13} /> Suggestions only. Each task is created only after you press its Create task button. CampusForge rechecks your current workspace membership and list access.</p>
        </>
      ) : (
        <p className="ai-answer-copy">{message.content}</p>
      )}
    </div>
  );
}

export default function AiAssistantPanel({ onClose, boards = [], lists = [], activeBoardId = "", onTaskCreated }) {
  const [status, setStatus] = useState(null);
  const [statusError, setStatusError] = useState("");
  const [privacyAcknowledged, setPrivacyAcknowledged] = useState(false);
  const [mode, setMode] = useState("ask");
  const [boardId, setBoardId] = useState(activeBoardId || "");
  const [availableFiles, setAvailableFiles] = useState([]);
  const [selectedFileId, setSelectedFileId] = useState("");
  const [filesLoading, setFilesLoading] = useState(false);
  const [filesError, setFilesError] = useState("");
  const initialTargetBoardId = activeBoardId || boards[0]?.id;
  const [proposalListId, setProposalListId] = useState(
    lists.find((list) => list.board_id === initialTargetBoardId && /^(to do|todo)$/i.test(list.name))?.id
      || lists.find((list) => list.board_id === initialTargetBoardId)?.id
      || lists[0]?.id
      || ""
  );
  const [messages, setMessages] = useState([]);
  const [prompt, setPrompt] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const endRef = useRef(null);
  const panelRef = useRef(null);
  const conversationGeneration = useRef(0);
  const onCloseRef = useRef(onClose);
  const loadingRef = useRef(loading);
  onCloseRef.current = onClose;
  loadingRef.current = loading;
  const availableModes = useMemo(() => new Set(status?.modes || MODES.map((item) => item.id)), [status]);

  useEffect(() => {
    let cancelled = false;
    aiApi.status()
      .then((value) => { if (!cancelled) setStatus(value); })
      .catch((requestError) => {
        if (!cancelled) setStatusError(requestError.message || "Could not check AI availability.");
      });
    return () => { cancelled = true; };
  }, []);

  useEffect(() => {
    const targetBoardId = boardId || activeBoardId || boards[0]?.id;
    const nextList = lists.find((list) => list.board_id === targetBoardId && /^(to do|todo)$/i.test(list.name))
      || lists.find((list) => list.board_id === targetBoardId)
      || lists[0];
    setProposalListId(nextList?.id || "");
  }, [boardId, activeBoardId, lists, boards]);

  useEffect(() => {
    if (mode !== "file_summary") return undefined;
    let cancelled = false;
    setFilesLoading(true);
    setFilesError("");
    aiApi.files(boardId || null)
      .then((rows) => {
        if (!cancelled) {
          const usable = Array.isArray(rows) ? rows.filter(canSummarizeFile) : [];
          setAvailableFiles(usable);
          setSelectedFileId((current) => usable.some((file) => file.id === current) ? current : (usable[0]?.id || ""));
        }
      })
      .catch((requestError) => {
        if (!cancelled) setFilesError(requestError.message || "Could not load your accessible files.");
      })
      .finally(() => { if (!cancelled) setFilesLoading(false); });
    return () => { cancelled = true; };
  }, [mode, boardId]);

  useEffect(() => {
    endRef.current?.scrollIntoView({ behavior: "smooth", block: "end" });
  }, [messages, loading]);

  useEffect(() => {
    const previousOverflow = document.body.style.overflow;
    const previousFocus = document.activeElement;
    const panel = panelRef.current;
    document.body.style.overflow = "hidden";

    const getFocusable = () => panel?.querySelectorAll(
      'button:not([disabled]), input:not([disabled]), textarea:not([disabled]), select:not([disabled]), a[href], [tabindex]:not([tabindex="-1"])'
    ) || [];
    const initialTargets = getFocusable();
    if (initialTargets.length) initialTargets[0].focus();
    else panel?.focus();

    const onKeyDown = (event) => {
      if (event.key === "Escape" && !loadingRef.current) {
        event.preventDefault();
        onCloseRef.current?.();
        return;
      }
      if (event.key !== "Tab") return;
      const items = Array.from(getFocusable());
      if (!items.length) {
        event.preventDefault();
        panel?.focus();
        return;
      }
      const first = items[0];
      const last = items[items.length - 1];
      if (event.shiftKey && (document.activeElement === first || !panel?.contains(document.activeElement))) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && (document.activeElement === last || !panel?.contains(document.activeElement))) {
        event.preventDefault();
        first.focus();
      }
    };

    window.addEventListener("keydown", onKeyDown, true);
    return () => {
      document.body.style.overflow = previousOverflow;
      window.removeEventListener("keydown", onKeyDown, true);
      if (previousFocus instanceof HTMLElement && previousFocus.isConnected) previousFocus.focus();
    };
  }, []);

  function resetScopedConversation() {
    conversationGeneration.current += 1;
    setMessages([]);
    setPrompt("");
    setError("");
    setPrivacyAcknowledged(false);
    setLoading(false);
  }

  async function submitPrompt(value = prompt) {
    const content = value.trim();
    if (!content || loading || !status?.enabled || !privacyAcknowledged) return;
    if (mode === "file_summary" && !selectedFileId) {
      setError("Select a clean UTF-8 text file before asking for a document summary.");
      return;
    }
    const requestGeneration = conversationGeneration.current;
    const history = messages.slice(-8).map(({ role, content: messageContent }) => ({
      role,
      content: messageContent.slice(0, 1800)
    }));
    setMessages((current) => [...current, { role: "user", content, mode }]);
    setPrompt("");
    setError("");
    setLoading(true);
    try {
      const result = await aiApi.ask({
        prompt: content,
        mode,
        board_id: boardId || null,
        acknowledged_data_sharing: privacyAcknowledged,
        file_id: mode === "file_summary" ? selectedFileId : null,
        history
      });
      if (requestGeneration !== conversationGeneration.current) return;
      setMessages((current) => [...current, {
        role: "assistant",
        content: result.answer,
        mode: result.mode,
        sources: result.sources || [],
        model: result.model,
        usage: result.usage
      }]);
    } catch (requestError) {
      if (requestGeneration === conversationGeneration.current) {
        setError(requestError.message || "The assistant could not complete this request.");
      }
    } finally {
      if (requestGeneration === conversationGeneration.current) setLoading(false);
    }
  }

  async function createProposedTask(task) {
    if (!proposalListId) {
      throw new Error("Choose a project column before creating proposed tasks.");
    }
    await taskApi.create({
      title: String(task.title).trim(),
      description: String(task.description || "").trim(),
      priority: ["low", "medium", "high"].includes(String(task.priority).toLowerCase()) ? String(task.priority).toLowerCase() : "medium",
      due_date: null,
      list_id: proposalListId,
      assignee_id: null,
      estimated_minutes: Number(task.estimated_minutes)
    });
    await onTaskCreated?.();
  }

  const enabled = status?.enabled === true;

  return (
    <div className="ai-panel-backdrop" onMouseDown={(event) => { if (event.target === event.currentTarget && !loading) onClose(); }}>
      <section className="ai-panel" role="dialog" aria-modal="true" aria-labelledby="ai-panel-title" ref={panelRef} tabIndex={-1}>
        <header className="ai-panel-header">
          <div className="ai-panel-brand">
            <span className="ai-panel-brand-icon"><BrainCircuit size={20} /></span>
            <div>
              <p className="eyebrow">CAMPUSFORGE INTELLIGENCE</p>
              <h2 id="ai-panel-title">Project assistant</h2>
              <span className="ai-panel-subtitle"><span className={enabled ? "ai-status-dot active" : "ai-status-dot"} /> {enabled ? "Connected · Read-only mode" : "Not configured on this environment"}</span>
            </div>
          </div>
          <button className="ai-close-button" type="button" onClick={onClose} aria-label="Close AI assistant" disabled={loading}><X size={19} /></button>
        </header>

        <div className="ai-panel-controls">
          <div className="ai-mode-tabs" role="group" aria-label="Assistant mode">
            {MODES.filter((item) => availableModes.has(item.id)).map(({ id, label, icon: Icon }) => (
              <button key={id} className={"ai-mode-tab" + (mode === id ? " active" : "")} type="button" onClick={() => {
                if (mode === id) return;
                resetScopedConversation();
                setMode(id);
                setSelectedFileId("");
              }} aria-pressed={mode === id}>
                <Icon size={14} /><span>{label}</span>
              </button>
            ))}
          </div>
          <label className="ai-scope-select">
            <span>Context</span>
            <select
              aria-label="Choose project context"
              value={boardId}
              onChange={(event) => {
                const nextBoardId = event.target.value;
                if (nextBoardId === boardId) return;
                resetScopedConversation();
                setSelectedFileId("");
                setBoardId(nextBoardId);
              }}
            >
              <option value="">All accessible projects</option>
              {boards.map((board) => <option key={board.id} value={board.id}>{board.name}</option>)}
            </select>
          </label>
          {mode === "file_summary" && enabled && (
            <label className="ai-scope-select ai-file-target">
              <span>Document</span>
              <select
                value={selectedFileId}
                onChange={(event) => {
                  const nextFileId = event.target.value;
                  if (nextFileId === selectedFileId) return;
                  resetScopedConversation();
                  setSelectedFileId(nextFileId);
                }}
                aria-label="Choose a clean text document to summarize"
                disabled={filesLoading || availableFiles.length === 0}
              >
                <option value="">{filesLoading ? "Loading clean text files…" : "Choose a clean text file"}</option>
                {availableFiles.map((file) => (
                  <option key={file.id} value={file.id}>
                    {file.original_name} · {(Number(file.size_bytes) / 1024).toFixed(0)} KB
                  </option>
                ))}
              </select>
            </label>
          )}
          {mode === "task_breakdown" && enabled && (
            <label className="ai-scope-select ai-proposal-target">
              <span>Create in</span>
              <select value={proposalListId} onChange={(event) => setProposalListId(event.target.value)} aria-label="Choose where approved tasks should be created">
                <option value="">Choose a project column</option>
                {lists
                  .filter((list) => !boardId || list.board_id === boardId)
                  .map((list) => {
                    const boardName = boards.find((board) => board.id === list.board_id)?.name || "Project";
                    return <option key={list.id} value={list.id}>{boardName} · {list.name}</option>;
                  })}
              </select>
            </label>
          )}
        </div>

        <div className="ai-panel-content">
          {statusError && <div className="ai-inline-error" role="alert"><AlertCircle size={15} /> {statusError}</div>}
          {mode === "file_summary" && filesError && <div className="ai-inline-error" role="alert"><AlertCircle size={15} /> {filesError}</div>}
          {mode === "file_summary" && !filesLoading && !filesError && availableFiles.length === 0 && (
            <div className="ai-setup-note"><LockKeyhole size={14} /> No eligible file is available. Only clean, UTF-8 text files up to 128 KB can be summarized; PDFs and images are not supported.</div>
          )}

          {!enabled ? (
            <div className="ai-setup-state">
              <div className="ai-setup-icon"><Sparkles size={24} /></div>
              <h3>Give your team a smarter starting point</h3>
              <p>The assistant is safely disabled until an administrator configures a compatible AI endpoint and enables it on the API server.</p>
              <div className="ai-setup-checklist">
                <span><CheckCircle2 size={15} /> Provider credentials stay on the server</span>
                <span><CheckCircle2 size={15} /> Context is scoped to your access</span>
                <span><CheckCircle2 size={15} /> Suggestions never change project data</span>
              </div>
              <div className="ai-setup-note"><LockKeyhole size={14} /> No prompt or project data is sent to an AI provider while this feature is disabled.</div>
              <p className="ai-setup-help">On the Rust API server, configure <code>CAMPUSFORGE_AI_ENABLED=true</code>, <code>CAMPUSFORGE_AI_API_BASE_URL</code>, <code>CAMPUSFORGE_AI_API_KEY</code>, and <code>CAMPUSFORGE_AI_MODEL</code>. Use an HTTPS OpenAI-compatible chat-completions endpoint in hosted environments.</p>
            </div>
          ) : messages.length === 0 ? (
            <div className="ai-welcome-state">
              <span className="ai-welcome-icon"><Lightbulb size={21} /></span>
              <h3>What should we work through?</h3>
              <p>{mode === "meeting_summary" ? "Review decisions and action items from authorized meeting notes." : mode === "chat_summary" ? "Review recent decisions and action items from authorized shared-channel messages." : mode === "file_summary" ? "Summarize one selected clean text document. The file is sent only after you acknowledge the data-sharing notice." : "Ask about accessible project records, request a concise summary, or draft a task plan for your team."}</p>
              <div className="ai-starter-list">
                {(mode === "file_summary"
                  ? [{ label: "Summarize selected document", prompt: "Summarize the selected document. Identify its purpose, key facts, important dates, decisions, risks, and explicit action items. Cite the source [F1]." }]
                  : mode === "meeting_summary"
                    ? [{ label: "Summarize latest meetings", prompt: "Summarize the authorized meeting notes. Separate explicit decisions, action items, unresolved questions, owners, and dates. Cite meeting notes." }]
                    : mode === "chat_summary"
                      ? [{ label: "Summarize team decisions", prompt: "Summarize decisions and explicit action items from the authorized shared-channel messages. Separate proposals from approved decisions and cite message references." }]
                      : STARTERS).map((starter) => (
                  <button type="button" className="ai-starter-button" key={starter.label} onClick={() => submitPrompt(starter.prompt)} disabled={loading}>
                    <span>{starter.label}</span><ArrowUpRight size={15} />
                  </button>
                ))}
              </div>
              <div className="ai-trust-note"><ShieldCheck size={14} /> Read-only context · Source references · Human-controlled actions</div>
            </div>
          ) : (
            <div className="ai-conversation" aria-live="polite">
              {messages.map((message, index) => (
                <article className={"ai-message ai-message-" + message.role} key={index}>
                  <div className="ai-message-avatar">{message.role === "assistant" ? <Sparkles size={15} /> : <span>You</span>}</div>
                  <div className="ai-message-main">
                    <div className="ai-message-label">{message.role === "assistant" ? "CampusForge assistant" : "You"}</div>
                    {message.role === "assistant" ? <AssistantAnswer message={message} onCreateTask={message.mode === "task_breakdown" ? createProposedTask : undefined} /> : <p className="ai-user-copy">{message.content}</p>}
                    {message.role === "assistant" && message.sources?.length > 0 && (
                      <div className="ai-citations">
                        <span className="ai-citations-label"><BookOpen size={12} /> Sources</span>
                        {message.sources.map((source) => (
                          <span className="ai-source-chip" key={source.reference} title={source.label}>
                            <strong>{source.reference}</strong> {source.label}
                          </span>
                        ))}
                      </div>
                    )}
                    {message.role === "assistant" && message.usage && (
                      <p className="ai-message-meta">
                        {message.model} · {message.usage.prompt_tokens ?? "—"} input / {message.usage.completion_tokens ?? "—"} output tokens
                      </p>
                    )}
                  </div>
                </article>
              ))}
              {loading && <div className="ai-thinking"><LoaderCircle size={16} /> Reviewing authorized project context…</div>}
              <div ref={endRef} />
            </div>
          )}
          {error && <div className="ai-inline-error" role="alert"><AlertCircle size={15} /> {error}</div>}
        </div>

        <footer className="ai-panel-footer">
          {enabled && (
            <label className="ai-data-consent">
              <input type="checkbox" checked={privacyAcknowledged} onChange={(event) => setPrivacyAcknowledged(event.target.checked)} />
              <span>I understand that my prompt, recent conversation, and the records selected by the chosen mode (projects/tasks, meeting notes, shared-channel messages, or the selected clean text file) will be sent to the configured AI provider. Changing projects starts a fresh conversation and requires a new acknowledgement.</span>
            </label>
          )}
          <div className="ai-composer-wrap">
            <textarea
              value={prompt}
              onChange={(event) => setPrompt(event.target.value.slice(0, 2000))}
              onKeyDown={(event) => {
                if (event.key === "Enter" && !event.shiftKey) {
                  event.preventDefault();
                  submitPrompt();
                }
              }}
              placeholder={!enabled ? "AI is unavailable until server configuration is complete" : !privacyAcknowledged ? "Acknowledge the data-sharing notice to begin…" : mode === "meeting_summary" ? "Summarize the latest meeting notes and decisions…" : mode === "chat_summary" ? "Summarize recent shared-channel decisions and action items…" : mode === "file_summary" ? "Ask a question about the selected document…" : "Ask about tasks, priorities, or project risks…"}
              rows={2}
              maxLength={2000}
              disabled={!enabled || !privacyAcknowledged || loading}
              aria-label="Ask the CampusForge assistant"
            />
            <button className="ai-send-button" type="button" onClick={() => submitPrompt()} disabled={!enabled || !privacyAcknowledged || loading || !prompt.trim()} aria-label="Send prompt">
              {loading ? <LoaderCircle size={17} className="ai-spin" /> : <Send size={17} />}
            </button>
          </div>
          <div className="ai-footer-meta">
            <span><LockKeyhole size={12} /> Session-only conversation</span>
            <span>{prompt.length}/2000</span>
          </div>
          <p className="ai-disclaimer"><CircleHelp size={12} /> AI can make mistakes. Verify conclusions against cited records and avoid entering secrets or sensitive personal data.</p>
        </footer>
      </section>
    </div>
  );
}
