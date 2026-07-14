# Material Asset Upload Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace manual material URL entry with validated file upload, automatic metadata extraction, read-only file details, and image lightbox preview.

**Architecture:** The Rust API owns synchronous upload orchestration and writes to the existing shared asset volume before creating the material record. The Next.js client sends multipart data, preserves immutable system fields during edits, and manages a local image preview dialog. Every behavior is introduced through a failing test first.

**Tech Stack:** Rust, Axum multipart, Tokio filesystem/process, PostgreSQL/SQLx, `image`, `ffprobe`, Next.js 14, React, TypeScript, Vitest, Testing Library, Playwright, OpenSpec.

---

## Task 1: Upload detection and storage

**Files:**
- Create: `backend/src/application/material_upload.rs`
- Modify: `backend/src/application/mod.rs`
- Modify: `backend/Cargo.toml`
- Modify: `backend/Dockerfile`
- Test: inline unit tests in `backend/src/application/material_upload.rs`

- [x] Write tests for image detection, subtitle validation, unsupported extension, empty content, metadata parsing and storage cleanup.
- [x] Run `cargo test -p novex-api material_upload -- --nocapture` in `ai-agent-api` and verify failures are caused by missing upload functions.
- [x] Implement `UploadedMaterialFile`, `DetectedMaterial`, `LocalMaterialStorage`, 500 MiB validation, image dimension reading, UTF-8 subtitle validation and `ffprobe` JSON parsing.
- [x] Add `axum` multipart, Tokio `fs/io-util/process`, `image` decoding and MIME helpers; install `ffmpeg` in the API image.
- [x] Re-run the focused tests and keep them green.

## Task 2: Multipart upload route

**Files:**
- Modify: `backend/src/api/materials/mod.rs`
- Modify: `backend/src/api/materials/handlers.rs`
- Modify: `backend/src/application/materials.rs`
- Modify: `backend/src/bootstrap/state.rs`
- Modify: `backend/src/api/error.rs`
- Test: `backend/tests/material_routes.rs`

- [x] Add a route test posting a minimal valid PNG multipart body and asserting `201`, `material_type=image`, `/assets/uploads/` URL, image metadata and persisted bytes.
- [x] Add route tests for missing file, unsupported content, oversized request and invalid project; assert no file remains.
- [x] Run the material route test and verify the new cases fail because the upload route does not exist.
- [x] Add `POST /api/projects/:project_id/materials/upload` with a route-local 500 MiB body limit.
- [x] Parse `file`, optional `file_name` and JSON `tags`; validate the project before writing, probe after writing, create `CreateMaterialInput`, and clean up on every post-write failure.
- [x] Map validation to 400, body limit to 413, missing project to 404 and storage/probe failures to 500 without leaking absolute paths.
- [x] Re-run material route and repository tests.

## Task 3: Frontend API and model contract

**Files:**
- Modify: `apps/video-agent/app/lib/api.ts`
- Modify: `apps/video-agent/app/lib/api.test.ts`
- Modify: `apps/video-agent/app/pages/material-library/materialModel.ts`
- Modify: `apps/video-agent/app/pages/material-library/materialModel.test.ts`

- [x] Add failing API tests asserting `uploadMaterial` sends `FormData` without a JSON content type and normalizes returned `/assets` URLs.
- [x] Add failing model tests asserting edit payload preserves type, URL, thumbnail and metadata while changing only name/tags; assert file summary formatting by material type.
- [x] Implement `requestFormData`, `uploadMaterial`, `MaterialEditableFormState`, `materialEditPayload`, tag parsing and `formatMaterialFileSummary`.
- [x] Remove manual metadata fields and the URL-create payload from the operator form model.
- [x] Run focused Vitest files and verify green.

## Task 4: Material library upload UI

**Files:**
- Modify: `apps/video-agent/app/page.tsx`
- Modify: `apps/video-agent/app/pages/material-library/MaterialLibraryPage.tsx`
- Modify: `apps/video-agent/app/page.test.tsx`
- Modify: `apps/video-agent/app/styles.css`

- [x] Replace existing create tests with failing tests for “上传素材”, required file selection, automatic name fill, optional tags, upload success and error input preservation.
- [x] Add failing tests proving no URL/thumbnail/source/license/width/height/format inputs render in create or edit state.
- [x] Implement selected `File` state in `page.tsx`, call `uploadMaterial` for creation, and continue using immutable-preserving `updateMaterial` for edits.
- [x] Replace the type picker and technical fields with a file picker/upload summary for create state and a compact read-only system summary for edit state.
- [x] Update empty-state copy and search placeholder so they no longer mention URL registration.
- [x] Run page tests and keep existing archive/restore and canvas selection behavior green.

## Task 5: Image preview dialog

**Files:**
- Modify: `apps/video-agent/app/pages/material-library/MaterialLibraryPage.tsx`
- Modify: `apps/video-agent/app/styles.css`
- Test: `apps/video-agent/app/page.test.tsx`

- [x] Add failing tests that image preview opens a named dialog, zooms from 100% within 50%-200%, closes by button/Escape/backdrop, and non-image placeholders are not buttons.
- [x] Implement the preview trigger, dialog state, focus restoration, keyboard handling and stable zoom controls.
- [x] Add fixed dialog/media dimensions and responsive desktop constraints without changing the established workspace layout.
- [x] Re-run page tests and lint.

## Task 6: E2E, memory, and verification

**Files:**
- Modify: `apps/video-agent/e2e/workspace.spec.ts`
- Modify: `docs/memory/video-agent-workspace-flow.md`
- Modify: `openspec/changes/upload-material-assets/tasks.md`

- [x] Update mocked upload routes and E2E assertions for upload, hidden address, automatic details and large image preview.
- [x] Record the confirmed upload and hidden-address decision in the material workflow memory.
- [x] Run Rust format, Clippy, focused and full Rust tests in Compose.
- [x] Run frontend unit tests, Playwright E2E, lint and build in Compose.
- [x] Run `openspec validate upload-material-assets --strict` and `openspec instructions apply --change upload-material-assets --json`; reconcile every task checkbox.
- [x] Archive the completed change and strictly validate all OpenSpec specs.
- [x] Do not run `git add`, `git commit` or `git push`; report the dirty worktree for user review.

## Self-Review

- Spec coverage: upload success/failure, metadata, cleanup, hidden address, immutable edit, preview and regression verification all map to tasks.
- Placeholder scan: no deferred implementation or unspecified error handling remains.
- Type consistency: upload returns the existing `Material`; edits use the existing `MaterialPayload`; display-only summary reads existing metadata keys.
