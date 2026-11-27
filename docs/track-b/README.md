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

This file establishes the initial product and engineering boundary. It does not claim that an AI provider is configured or that AI features are implemented.

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

- Add a Rust-side provider interface so the application is not coupled to one vendor.
- Keep model name, endpoint, key, timeout, and token limits in backend-only configuration.
- Add a server-side feature flag and per-workspace/user rate and spend controls.
- Normalize provider errors and validate model responses before returning them to the client.
- Do not enable provider calls in production until the provider and data-processing review is complete.

### B2 — Authorized context retrieval

- Build a context service that reuses CampusForge authentication, workspace membership, and resource authorization.
- Retrieve only the minimum records required for the user's selected workspace/project.
- Treat task descriptions, comments, uploaded text, chat, and meeting notes as untrusted data—not instructions.
- Add source references and timestamps to context items so generated answers can cite their evidence.
- Do not permit arbitrary SQL, unrestricted database access, or model-directed resource discovery.

### B3 — Read-only assistant and summaries

- Add an assistant surface in the React client, initially behind a feature flag.
- Support questions about the current authorized project and workspace.
- Add project/board summaries and meeting-note summaries.
- Apply output schemas, maximum response sizes, timeouts, and safe rendering for generated Markdown.
- Keep this phase read-only: no AI-originated writes.

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

## 4. Proposed technical shape

The existing application remains the system of record:

- **Frontend:** React/Vite assistant panel and proposal-review UI.
- **Backend:** Rust/Axum routes and a provider-neutral AI service boundary.
- **Authorization:** Existing CampusForge identity and workspace/resource authorization; all context assembly is server-side.
- **Data:** PostgreSQL/SQLx for approved conversation metadata and audit events, subject to a retention decision.
- **Streaming:** Select a transport only after the UX and proxy/runtime constraints are tested; do not introduce a new realtime dependency by default.

Potential modules (names are illustrative until implementation is approved):

```text
server/src/ai/
  mod.rs
  provider.rs       # provider-neutral request/response contract
  context.rs        # authorized, minimal workspace context
  prompts.rs        # versioned instructions and task templates
  output.rs         # structured output validation
  policy.rs         # feature flags, limits, and allowed capabilities
  audit.rs          # proposal/approval audit hooks
client/src/features/ai/
  AssistantPanel.jsx
  ProposalReview.jsx
```

Possible persistence entities to evaluate—not approved migrations:

- `ai_conversations`: user/workspace ownership, title, status, timestamps.
- `ai_messages`: role, content, provider/model metadata, source references, timestamps.
- `ai_action_proposals`: validated proposal payload, source record references, status, expiry, approver, and applied resource ID.
- `ai_usage_events`: request counts, token usage, latency, and cost metadata without storing provider secrets.

Do not create these tables until the data-retention model, indexing, deletion behavior, and workspace/user authorization rules are agreed.

## 5. Security and privacy requirements

- Enforce workspace and resource authorization before every context read and again before every proposed mutation.
- Treat retrieved user content as untrusted input; it cannot override system policy or grant tools/permissions.
- Start with read-only model capabilities. Any future tool/action interface must use allow-listed server functions, strict input schemas, and user confirmation.
- Never expose provider API keys, database credentials, session tokens, or internal system prompts in model context or client responses.
- Apply redaction and data minimization before external provider calls.
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
| Provider selection and data terms | Not decided |
| Provider adapter implementation | Not started |
| Authorized context service | Not started |
| Assistant UI | Not started |
| AI persistence migrations | Not started |
| Evaluation/security suite | Not started |
| Production enablement | Disabled / not approved |

Track B is being developed separately from Track A. Keep the Track A PR and production acceptance record intact. Do not merge either track to `main` or trigger deployment without explicit release authorization.
