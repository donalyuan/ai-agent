# Conversational Agent Runtime Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a reusable conversational Agent Runtime, connect the script Agent, and expose the workflow in the video-agent script detail UI after prototype approval.

**Architecture:** Add persisted conversations/messages beside existing `agent_runs`/`agent_steps`, then introduce a small runtime that routes one user turn to an `AgentAdapter`. The first adapter is `script`, which reads the bound script, asks the LLM for a structured scene patch, updates the scene, records steps, and stores the assistant reply. The frontend then binds the current script detail to this API through a script Agent chat panel.

**Tech Stack:** Rust, Axum, SQLx, PostgreSQL, `novex-model::LLMClient`, Next.js, TypeScript, Vitest, Playwright, OpenSpec.

**Current Constraint:** Backend tasks are implemented and verified. Frontend implementation is blocked until `docs/prototypes/video-agent/video-agent.pen` is updated and explicitly approved, because `script-agent-workspace` requires Pencil prototype confirmation before editing `apps/video-agent` UI.

---

### Task 1: Conversation Persistence

**Files:**
- Create: `backend/migrations/20260703030000_agent_conversations.sql`
- Create: `backend/src/agents/conversation.rs`
- Create: `backend/src/repositories/conversation_repository.rs`
- Modify: `backend/src/agents/mod.rs`
- Modify: `backend/src/repositories/mod.rs`
- Test: `backend/tests/database_migrations.rs`
- Test: `backend/tests/conversation_repository_contract.rs`

- [ ] Write failing migration/repository tests for conversations, messages, and run steps.
- [ ] Implement migration and repository methods.
- [ ] Run `cargo test -p novex-api --test database_migrations migrations_create_video_agent_core_schema` and `cargo test -p novex-api --test conversation_repository_contract`.

### Task 2: Runtime And Script Adapter

**Files:**
- Create: `backend/src/agents/conversational_runtime.rs`
- Modify: `backend/src/agents/llm.rs`
- Modify: `backend/src/repositories/script_repository.rs`
- Test: `backend/tests/conversational_script_agent.rs`

- [ ] Write failing service tests for sending a script dialogue turn that updates one scene.
- [ ] Add structured script scene patch prompt/parser.
- [ ] Add `update_scene` to `ScriptRepository`.
- [ ] Implement runtime turn handling and script adapter.
- [ ] Run `cargo test -p novex-api --test conversational_script_agent`.

### Task 3: HTTP API

**Files:**
- Modify: `backend/src/agents/models/request.rs`
- Modify: `backend/src/lib.rs`
- Test: `backend/tests/conversation_routes.rs`

- [ ] Write failing route tests for creating a conversation, sending a message, reading messages, and errors.
- [ ] Add request/response models and routes.
- [ ] Wire `AppState` repositories/runtime.
- [ ] Run `cargo test -p novex-api --test conversation_routes`.

### Task 4: Verification And Memory

**Files:**
- Modify: `MEMORY.md`
- Modify: `docs/memory/video-agent-workspace-flow.md`
- Modify: `openspec/changes/conversational-agent-runtime/tasks.md`

- [ ] Record the confirmed runtime boundary in memory.
- [ ] Mark OpenSpec tasks complete as implementation finishes.
- [ ] Run `cargo test -p novex-api conversation`, `cargo test -p novex-api --test script_routes`, and `openspec instructions apply --change "conversational-agent-runtime" --json`.

### Task 5: Frontend Prototype Gate

**Files:**
- Modify: `docs/prototypes/video-agent/video-agent.pen`
- Modify: `openspec/changes/conversational-agent-runtime/tasks.md`

- [ ] Update the Pencil prototype to add a script Agent conversation panel in the script detail workflow.
- [ ] Include empty state, sending state, failed send state, and successful scene-refresh state in the prototype.
- [ ] Get explicit user approval before editing `apps/video-agent` page files.

### Task 6: Frontend API Client

**Files:**
- Modify: `apps/video-agent/app/lib/api.ts`
- Test: `apps/video-agent/app/lib/api.test.ts`

- [ ] Write failing API client tests for `createAgentConversation`, `listAgentMessages`, and `sendAgentMessage`.
- [ ] Add conversation/message/run TypeScript types.
- [ ] Implement the three API helpers.
- [ ] Run `npm run test -- api.test.ts` inside `ai-agent-video-agent`.

### Task 7: Script Detail Chat Panel

**Files:**
- Modify: `apps/video-agent/app/page.tsx`
- Modify: `apps/video-agent/app/styles.css`
- Test: `apps/video-agent/app/page.test.tsx`

- [ ] Write failing component tests for the script Agent conversation panel.
- [ ] Add local state for conversation ID, messages, draft message, sending state, and panel errors.
- [ ] Create a script conversation on first send using selected project and selected script.
- [ ] Send user message, append returned user/assistant messages, and refresh script detail with `getScript`.
- [ ] Disable input when API is down, no script is selected, or a send is in progress.
- [ ] Keep errors scoped to the chat panel.
- [ ] Run `npm run test -- page.test.tsx` inside `ai-agent-video-agent`.

### Task 8: Frontend E2E And Final Verification

**Files:**
- Modify: `apps/video-agent/e2e/workspace.spec.ts`
- Modify: `openspec/changes/conversational-agent-runtime/tasks.md`

- [ ] Extend Playwright route mocks for conversation creation, message send, message list, and refreshed script detail.
- [ ] Verify the operator can send a script Agent instruction and see the refreshed scene content.
- [ ] Run `npm run lint` and relevant frontend tests inside `ai-agent-video-agent`.
- [ ] Run `cargo test --workspace` inside `ai-agent-api`.
- [ ] Run `openspec instructions apply --change "conversational-agent-runtime" --json` and confirm all tasks are complete.

## Self-Review

- Spec coverage: persistence, runtime interface, script Agent adapter, HTTP API, frontend API client, frontend chat panel, and error semantics are represented.
- Scope control: frontend chat UI is included, but implementation remains gated by Pencil prototype approval.
- Type consistency: conversation IDs, message roles, `agent_type`, and `subject_id` are carried through persistence, runtime, and HTTP models.
