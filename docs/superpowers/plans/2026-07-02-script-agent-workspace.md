# Script Agent Workspace Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the confirmed desktop `VEDIO-AGENT / 视频工作台` with six-agent navigation, project selection/creation, script generation/list/detail/status workflows, and a timeline comparison detail view.

**Architecture:** Add minimal project list/create support to the existing Rust API, then replace the `admin/` landing check page with a client-side Next.js workbench. The UI will use a typed fetch client and keep state local to the page; only the script agent module is implemented while the other five agent entries remain reserved navigation items.

**Tech Stack:** Rust + Axum + SQLx + PostgreSQL, Next.js 14 + TypeScript + React, Vitest + Testing Library for frontend unit/component tests.

---

### Task 1: Backend Project API

**Files:**
- Modify: `backend/src/repositories/project_repository.rs`
- Modify: `backend/src/agents/models/request.rs`
- Modify: `backend/src/agents/models/mod.rs`
- Modify: `backend/src/lib.rs`
- Test: `backend/tests/project_repository_contract.rs`
- Test: `backend/tests/project_routes.rs`

- [ ] Add failing repository contract tests for creating and listing projects.
- [ ] Implement `Project`, `CreateProjectInput`, `create_project`, and `list_projects` in `ProjectRepository`.
- [ ] Add failing route tests for `GET /api/projects` and `POST /api/projects`.
- [ ] Implement `CreateProjectRequest`, `ProjectResponse`, route handlers, validation, and router registration.
- [ ] Run `cargo test -p novex-api project` and full backend tests.

### Task 2: Frontend API Client And Tests

**Files:**
- Modify: `admin/package.json`
- Modify: `admin/package-lock.json`
- Create: `admin/vitest.config.ts`
- Create: `admin/test/setup.ts`
- Create: `admin/app/lib/api.ts`
- Test: `admin/app/lib/api.test.ts`

- [ ] Add Vitest and Testing Library dependencies and a `test` script.
- [ ] Write failing tests for API base URL default/env behavior and project/script request helpers.
- [ ] Implement typed API client with structured `ApiError` handling.
- [ ] Run `npm run test` and `npm run lint`.

### Task 3: Workspace UI

**Files:**
- Modify: `admin/app/page.tsx`
- Modify: `admin/app/styles.css`
- Modify: `admin/app/layout.tsx`
- Test: `admin/app/page.test.tsx`

- [ ] Write failing component tests for brand title, six-agent menu, empty project state, and timeline detail view.
- [ ] Implement the confirmed desktop workbench UI and local workflow state.
- [ ] Wire project creation/listing, script list/detail/generate/status update calls through the API client.
- [ ] Keep the UI desktop-only per memory and OpenSpec.
- [ ] Run frontend tests, lint, and build.

### Task 4: Verification And Docs

**Files:**
- Modify: `openspec/changes/script-agent-workspace/tasks.md`
- Modify: `README.md`

- [ ] Mark completed OpenSpec tasks as verification passes.
- [ ] Update README with workbench URL and validation commands.
- [ ] Run OpenSpec validation, backend tests, frontend tests, lint, and build.
