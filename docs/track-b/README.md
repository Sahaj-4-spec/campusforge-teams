# Track B — AI Project Assistant and Intelligent Collaboration

**Status:** Started — foundation and scope definition  
**Repository:** `Sahaj-4-spec/campusforge-teams`  
**Working branch:** `track-b/ai-assistant-foundation`  
**Base:** `main`  
**Track A relationship:** Parallel, isolated work. Track A's release and acceptance gates remain unchanged.

## 1. Purpose

Track A deliberately delivered the non-AI CampusForge Teams product. Track B begins the separately scoped AI capabilities that were deferred from Track A:

- A contextual AI project assistant for questions about a user's authorized workspace and project.
- Concise project, board, and task summaries.
- Meeting-note summarization and extraction of decisions and proposed action items.
- Task breakdown and subtask proposals.
- Explanations of blockers, overdue work, workload patterns, and project progress.

This file establishes the initial product and engineering boundary. The first read-only assistant slice is implemented on this branch, but no provider is configured and the feature remains disabled by default.

## 2. Initial product principles

1. **Assist, do not take over.** AI outputs are suggestions. Creating, editing, assigning, moving, or deleting CampusForge records requires an explicit user action and the existing API's authorization checks.
2. **Respect the current user's access.** AI context is assembled server-side from records the authenticated user is authorized to read. Workspace or board membership is checked before retrieval.
3. **Keep tenants separate.** Every conversation, context lookup, proposal, and audit record is scoped to the authorized user and workspace. Cross-workspace retrieval is prohibited.
4. **Show evidence.** Answers about CampusForge data should reference the records used (for example, task, board, or meeting-note links). The assistant must distinguish stored facts from generated interpretations.
5. **Keep credentials server-side.** Provider keys and credentials must never reach the browser, Vite variables, source control, or model-visible context.
6. **Make failure safe.** Timeouts, provider failures, malformed output, quota exhaustion, or missing permissions must not trigger partial or unreviewed mutations.
7. **Protect private data.** Minimize the context sent to a provider, avoid sending unrelated workspace content, and establish provider data-retention and training terms before production use.

## 3. Proposed MVP

### B0 — Scope and safety foundation

- Confirm the first user journeys and success criteria.
- Decide what data classes can be sent to an external model.
- Choose the initial AI provider and hosting/data-processing terms.
- Define retention, deletion, user disclosure, budget, and rate-limit policies.
- Threat-model prompt injection, cross-tenant access, tool misuse, and sensitive-data leakage.

### B1 — Provider abstraction and feature controls

**Implemented on this branch:**
- Added a provider-neutral OpenAI-compatible chat-completions adapter in `server/src/ai.rs`.
- Configured model, endpoint, key, timeout, request size, and output limits exclusively through backend environment variables.
- Added the `CAMPUSFORGE_AI_ENABLED` kill switch; it defaults to disabled and the UI reports an unavailable state when provider settings are missing.
- Added per-user (10/minute) and shared per-workspace (120/hour) fixed-window request limits using the existing PostgreSQL rate-limit table.
- Disabled HTTP redirects and require HTTPS provider endpoints, except loopback HTTP for local development.
- Normalized provider failures and reject empty or oversized responses.

**Still required:** provider selection, processing/retention review, cost/spend budget, and live provider verification.

### B2 — Authorized context retrieval

**Implemented on this branch:**
- Reuses the existing authenticated `WorkspaceUser` extractor.
- Loads only boards and tasks in the selected workspace that the user may access as a board member or workspace owner/admin.
- Supports an optional board/project scope and rejects inaccessible project IDs.
- Limits context to 20 boards and 35 tasks, truncates long descriptions, and labels records with evidence references.
- Treats project records and conversation history as untrusted content in the model prompt.
- Does not expose SQL, arbitrary query tools, file contents, chat messages, or cross-workspace retrieval to the model.

**Still required:** dedicated adversarial cross-workspace and per-board role integration tests.

### B3 — Read-only assistant and summaries

**Implemented on this branch:**
- Added a responsive assistant panel with project scoping and in-memory conversation history.
- Added Ask, Summary, Task Plan, and Risk Review modes.
- Added source-reference chips, token-use metadata, suggested prompts, and clear unavailable/disabled states.
- Added structured JSON task-plan previews with an explicit "suggestions only" notice.
- Rendered AI output as plain text; no generated HTML is injected into the page.
- All requests remain read-only; the API returns `actions_applied: false`.
- The task-plan JSON schema, task count, priority, estimate, field lengths, and source references are validated server-side; model-invented source IDs are removed.
- A first-use data-sharing acknowledgement is required in the UI and is checked again by the API on every provider request. The notice covers the prompt, recent conversation, and authorized project context.
- Changing the selected project clears the conversation; changing workspaces remounts the assistant so prior history and consent do not carry across tenants.

**Not yet implemented:** meeting-note or file summarization, persistent conversations, streaming, and user preferences.

### B4 — Human-approved action proposals

- Generate structured task breakdowns, subtasks, and meeting action-item proposals.
- Show a preview with the proposed title, description, assignee, due date, and source evidence.
- Require an explicit user confirmation before calling existing mutation APIs.
- Re-check authorization and resource state at confirmation time; never rely on permissions captured when the proposal was generated.
- Record who approved an action and what proposal was applied.

### B5 — Evaluation and controlled release

- Test factual grounding, source attribution, refusal behavior, prompt-injection resistance, and output validation.
- Test cross-workspace and cross-board access with separate users and roles.
- Measure usefulness, latency, failure rate, token consumption, and cost.
- Provide a kill switch and a documented rollback path.
- Release only after privacy, security, cost, accessibility, and user acceptance gates pass.

## 4. Implemented technical shape

The existing application remains the system of record:

- **Frontend:** React/Vite assistant panel in `client/src/components/AiAssistantPanel.jsx`.
- **Backend:** Rust/Axum routes and an OpenAI-compatible `/chat/completions` adapter in `server/src/ai.rs`. The interface is configured by endpoint/model/key rather than a hard-coded vendor, but the initial wire protocol is OpenAI-compatible.
- **Authorization:** Existing `WorkspaceUser` authentication and workspace membership; board context is further restricted to board members or workspace owners/admins. All retrieval is server-side.
- **Context:** PostgreSQL board/task records only. Chat, files, meeting notes and unrelated workspace data are not sent to the model.
- **Conversation:** Browser-panel memory only. Messages are not persisted in PostgreSQL, and closing the panel clears the current conversation.
- **Writes:** None. Task breakdowns are validated display previews; the API always returns `actions_applied: false`.

### API contract

- `GET /api/ai/status` — authenticated workspace status and supported modes.
- `POST /api/ai/assistant` — authenticated read-only request.
- `GET /api/ai/insights` — authenticated, provider-independent Project Pulse dashboard data. Accepts an optional `board_id` query parameter and applies the same board-member or workspace owner/admin scope as assistant context reads.

Example request:

```json
{
  "prompt": "What is at risk this week?",
  "mode": "risk_review",
  "board_id": null,
  "acknowledged_data_sharing": true,
  "history": []
}
```

Supported modes are `ask`, `summary`, `task_breakdown`, and `risk_review`. Prompt length is capped at 2,000 characters; up to eight user/assistant history messages are accepted, with each message capped at 1,800 characters. The request must include `acknowledged_data_sharing: true`; the API rejects requests without it. An optional `board_id` scopes retrieval to one authorized board. Without it, retrieval is limited to the caller's accessible boards within the selected workspace.

The response contains `answer`, `mode`, `model`, `generated_at`, cited `sources`, prompt/completion token counts when supplied by the provider, and `actions_applied: false`. Only source references actually cited in the answer are returned in the `sources` list.

### Project Pulse

Project Pulse is available independently of the external AI provider and does not send workspace data to an AI vendor. It returns:
- A transparent 0–100 health heuristic and the underlying overdue, blocked, due-soon, unassigned-priority, stale, and unestimated counts.
- A top-five focus queue with evidence signals and suggested human next steps.
- Assigned workload estimates, with work split evenly for tasks that have multiple assignees and missing estimates shown separately.
- Per-board health comparisons for the authorized boards in scope.

Project Pulse reads are limited to 30 requests per user per minute and 300 per workspace per hour. The API reads up to 100 authorized boards and 5,000 tasks per response. When either cap is reached, `sample_capped` is true and the dashboard warns that the figures represent a bounded sample. Completion is inferred from Done/Complete/Completed/Closed/Resolved list names; blockers are inferred from Blocked/Stuck/On Hold list names. These are heuristics, not predictive analytics or AI-generated judgments. Users can narrow the dashboard to a single board for a more focused view.

### Provider configuration

Configure these values only in the Rust API environment:

- `CAMPUSFORGE_AI_ENABLED=true`
- `CAMPUSFORGE_AI_API_BASE_URL=https://your-provider.example/v1` (the server appends `/chat/completions`)
- `CAMPUSFORGE_AI_API_KEY=...`
- `CAMPUSFORGE_AI_MODEL=...`

Hosted endpoints must use HTTPS. HTTP loopback is accepted only when both `CAMPUSFORGE_ENV=development` and `CAMPUSFORGE_AI_ALLOW_LOOPBACK_PROVIDER=true` are set. Production must leave that opt-in disabled. Provider DNS is resolved, checked against private/metadata destinations, pinned to the validated addresses, and sent without ambient proxy settings. Redirects are disabled. Keep the feature off until provider retention/training terms, disclosure, spend ceilings and privacy review are approved.

### Existing safety controls

- Fixed-window rate limits: 10 requests/user/minute and 120 requests/workspace/hour, shared through the existing PostgreSQL rate-limit table.
- Provider call timeout: 45 seconds; output limit: 1,200 tokens and 12,000 characters.
- Prompt size, history length/roles, context record counts and description lengths are bounded.
- Common private-key, API-key, bearer-token, JWT-like, database-URL and email patterns are redacted recursively from provider-bound text and sanitized responses. This is defense in depth, not a guarantee that every sensitive value can be detected.
- Project records and conversation history are identified as untrusted content; no tools, arbitrary SQL or autonomous writes are exposed.

No AI-specific persistence migration has been created. Conversation/audit tables remain deferred until retention, deletion, indexing and migration ordering are approved.

## 5. Security and privacy requirements

- Enforce workspace and resource authorization before every context read and again before every proposed mutation.
- Treat retrieved user content as untrusted input; it cannot override system policy or grant tools/permissions.
- Start with read-only model capabilities. Any future tool/action interface must use allow-listed server functions, strict input schemas, and user confirmation.
- Keep provider API keys, database credentials, and session tokens outside model context and client responses. A controlled internal system instruction is necessarily included in model context; model responses must never disclose that instruction or implementation details.
- Apply pattern-based redaction to prompts, conversation history, serialized workspace text and provider responses before returning them. This detects common credential, database-URL, JWT-like token and email patterns but cannot guarantee detection of every sensitive value; data minimization and provider privacy review remain required.
- Define conversation retention, export, and deletion behavior before persisting sensitive prompts or completions.
- Review each provider's data processing, storage region, retention, and model-training terms before using real user content.
- Apply per-user/workspace request limits, token budgets, spend ceilings, and an administrator kill switch.
- Log security-relevant approvals and failures without copying unnecessary private prompt content into logs.

## 6. Acceptance gates

Track B is not ready for production until all applicable checks pass:

- Authenticated user cannot retrieve another workspace's or unauthorized board's context.
- Prompt injection in comments, task descriptions, files, chat, or meeting notes cannot change server policy or invoke unapproved actions.
- No mutation occurs until the user confirms a validated proposal.
- Authorization and resource state are re-checked at apply time.
- Provider secrets are absent from browser bundles, logs, and Git history.
- Provider timeouts, invalid JSON, refusals, retries, and quota exhaustion fail safely.
- Conversation export/deletion and retention are tested.
- Provider data-handling terms and user-facing AI disclosure are approved.
- Security, privacy, cost, accessibility, and multi-user acceptance are signed off.

## 7. Explicit non-goals for the initial MVP

- Autonomous task/project mutations or background agents acting without confirmation.
- Cross-workspace answers or organization-wide data aggregation.
- Training or fine-tuning models on CampusForge user content.
- Uploading complete workspaces or arbitrary files to a provider by default.
- Video meeting transcription, call recording, or external calendar synchronization.
- A commitment to a particular provider, model, or paid AI plan before comparative evaluation.

## 8. Current execution status

| Work item | Status |
|---|---|
| Track B repository workspace | Started on `track-b/ai-assistant-foundation` |
| Initial scope and guardrails | Documented in this file |
| Rust read-only assistant API | Implemented on branch; CI verification pending |
| Provider-neutral OpenAI-compatible adapter | Implemented; server feature flag defaults off |
| Workspace/project-scoped context | Implemented for boards and tasks; adversarial integration tests pending |
| Assistant UI and conversation state | Implemented; current-session memory only |
| Ask, summary, task-plan, risk-review modes | Implemented |
| Source references and token usage display | Implemented |
| Request limits and provider URL checks | Implemented; spend-budget policy still needed |
| Meeting-note/file summarization | Not started |
| Persistent conversation/audit schema | Not started; requires retention and migration-order decision |
| Human-reviewed action application | Proposal preview only; apply workflow not implemented |
| Provider selection, terms, and credentials | Not decided/configured |
| Production enablement | Disabled / not approved |

The `CAMPUSFORGE_AI_*` variables are server-only. Keep `CAMPUSFORGE_AI_ENABLED=false` until provider data-handling terms, retention, spend ceilings, and production privacy/security review are approved. Track B is separate from Track A; do not merge either track to `main` or trigger deployment without explicit release authorization.
