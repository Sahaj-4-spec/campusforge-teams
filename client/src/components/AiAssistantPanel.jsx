import { useEffect, useMemo, useRef, useState } from "react";
import {
  AlertCircle,
  ArrowUpRight,
  BookOpen,
  BrainCircuit,
  CheckCircle2,
  CircleHelp,
  Clock3,
  FileText,
  Lightbulb,
  LoaderCircle,
  LockKeyhole,
  MessageSquare,
  Send,
  ShieldCheck,
  Sparkles,
  Target,
  X
} from "lucide-react";
import { aiApi } from "../lib/api.js";

const MODES = [
  { id: "ask", label: "Ask", icon: MessageSquare },
  { id: "summary", label: "Summary", icon: FileText },
  { id: "task_breakdown", label: "Task plan", icon: Target },
  { id: "risk_review", label: "Risk review", icon: ShieldCheck }
];

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

function AssistantAnswer({ message }) {
  const proposal = message.mode === "task_breakdown" ? parseProposal(message.content) : null;
  const proposalTasks = proposal && Array.isArray(proposal.tasks)
    ? proposal.tasks.filter((task) => task && typeof task === "object" && typeof task.title === "string").slice(0, 8)
    : [];
  const allowedReferences = new Set((message.sources || []).map((source) => source.reference));

  return (
    <div className="ai-answer-body">
      {proposal ? (
        <>
          {proposal.summary && <p className="ai-proposal-summary">{proposal.summary}</p>}
          <div className="ai-proposal-list">
            {proposalTasks.map((task, index) => (
              <article className="ai-proposal-card" key={task.title + "-" + index}>
                <div className="ai-proposal-card-top">
                  <span className="ai-proposal-number">{index + 1}</span>
                  <strong>{task.title}</strong>
                  <span className={"ai-priority ai-priority-" + (["low", "medium", "high"].includes(String(task.priority).toLowerCase()) ? String(task.priority).toLowerCase() : "medium")}>{["low", "medium", "high"].includes(String(task.priority).toLowerCase()) ? String(task.priority).toLowerCase() : "medium"}</span>
                </div>
                {task.description && <p>{task.description}</p>}
                <div className="ai-proposal-meta">
                  {Number.isFinite(Number(task.estimated_minutes)) && <span><Clock3 size={12} /> {task.estimated_minutes} min estimate</span>}
                  {task.reason && <span>{task.reason}</span>}
                </div>
                {Array.isArray(task.source_refs) && task.source_refs.some((reference) => allowedReferences.has(reference)) && (
                  <div className="ai-source-references">{task.source_refs.filter((reference) => allowedReferences.has(reference)).slice(0, 6).map((reference) => <span className="ai-source-chip" key={reference}>{reference}</span>)}</div>
                )}
              </article>
            ))}
          </div>
          <p className="ai-proposal-disclaimer"><LockKeyhole size={13} /> Suggestions only. Nothing has been added to your project.</p>
        </>
      ) : (
        <p className="ai-answer-copy">{message.content}</p>
      )}
    </div>
  );
}

export default function AiAssistantPanel({ onClose, boards = [], activeBoardId = "" }) {
  const [status, setStatus] = useState(null);
  const [statusError, setStatusError] = useState("");
  const [privacyAcknowledged, setPrivacyAcknowledged] = useState(false);
  const [mode, setMode] = useState("ask");
  const [boardId, setBoardId] = useState(activeBoardId || "");
  const [messages, setMessages] = useState([]);
  const [prompt, setPrompt] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const endRef = useRef(null);
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
    endRef.current?.scrollIntoView({ behavior: "smooth", block: "end" });
  }, [messages, loading]);

  useEffect(() => {
    const previousOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    const onKeyDown = (event) => {
      if (event.key === "Escape" && !loading) onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => {
      document.body.style.overflow = previousOverflow;
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [onClose, loading]);

  async function submitPrompt(value = prompt) {
    const content = value.trim();
    if (!content || loading || !status?.enabled || !privacyAcknowledged) return;
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
        history
      });
      setMessages((current) => [...current, {
        role: "assistant",
        content: result.answer,
        mode: result.mode,
        sources: result.sources || [],
        model: result.model,
        usage: result.usage
      }]);
    } catch (requestError) {
      setError(requestError.message || "The assistant could not complete this request.");
    } finally {
      setLoading(false);
    }
  }

  const enabled = status?.enabled === true;

  return (
    <div className="ai-panel-backdrop" onMouseDown={(event) => { if (event.target === event.currentTarget && !loading) onClose(); }}>
      <section className="ai-panel" role="dialog" aria-modal="true" aria-labelledby="ai-panel-title">
        <header className="ai-panel-header">
          <div className="ai-panel-brand">
            <span className="ai-panel-brand-icon"><BrainCircuit size={20} /></span>
            <div>
              <p className="eyebrow">CAMPUSFORGE INTELLIGENCE</p>
              <h2 id="ai-panel-title">Project assistant</h2>
              <span className="ai-panel-subtitle"><span className={enabled ? "ai-status-dot active" : "ai-status-dot"} /> {enabled ? "Connected · Read-only mode" : "Not configured on this environment"}</span>
            </div>
          </div>
          <button className="ai-close-button" type="button" onClick={onClose} aria-label="Close AI assistant" disabled={loading || !privacyAcknowledged}><X size={19} /></button>
        </header>

        <div className="ai-panel-controls">
          <div className="ai-mode-tabs" role="group" aria-label="Assistant mode">
            {MODES.filter((item) => availableModes.has(item.id)).map(({ id, label, icon: Icon }) => (
              <button key={id} className={"ai-mode-tab" + (mode === id ? " active" : "")} type="button" onClick={() => setMode(id)} aria-pressed={mode === id}>
                <Icon size={14} /><span>{label}</span>
              </button>
            ))}
          </div>
          <label className="ai-scope-select">
            <span>Context</span>
            <select aria-label="Choose project context" value={boardId} onChange={(event) => setBoardId(event.target.value)}>
              <option value="">All accessible projects</option>
              {boards.map((board) => <option key={board.id} value={board.id}>{board.name}</option>)}
            </select>
          </label>
        </div>

        <div className="ai-panel-content">
          {statusError && <div className="ai-inline-error" role="alert"><AlertCircle size={15} /> {statusError}</div>}

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
              <p>Ask about accessible project records, request a concise summary, or draft a task plan for your team.</p>
              <div className="ai-starter-list">
                {STARTERS.map((starter) => (
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
                    {message.role === "assistant" ? <AssistantAnswer message={message} /> : <p className="ai-user-copy">{message.content}</p>}
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
              <span>I understand that my prompt and authorized board/task context will be sent to the AI provider configured for this workspace.</span>
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
              placeholder={!enabled ? "AI is unavailable until server configuration is complete" : !privacyAcknowledged ? "Acknowledge the data-sharing notice to begin…" : "Ask about tasks, priorities, or project risks…"}
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
