## 1. Scope and storage contracts

- [x] 1.1 Add one mandatory `X-Project-Scope` dependency to creative, scenes, text-generation, video-generation, agent-edit and Asset Center router mounts; cover missing and foreign scope regressions.
- [x] 1.2 Replace Local storage intent/reference key checks with `canonical_object_key()` and reject all non-canonical forms.
- [x] 1.3 Persist and compare complete multipart session binding fields on operation-key reuse and completion.
- [x] 1.4 Make Local `prove_no_references()` fail closed and mount a shared Compose workspace volume for API and Media Worker.

## 2. Owner provenance and provider catalog

- [x] 2.1 Validate Creative source binding against the real same-project SourceMaterial current version and immutable facts.
- [x] 2.2 Derive uploaded SourceMaterial hash and `parsed`/`valid` state from a verified AssetVersion, rejecting foreign/incomplete versions.
- [x] 2.3 Record and finalize text `ProviderCall` facts with request fingerprint, capability snapshot, filtered usage and sanitized failure diagnostics.

## 3. Temporal execution and concurrency

- [x] 3.1 Execute text generation from the Temporal activity and enter the matching node/run into `waiting_review` idempotently.
- [x] 3.2 Add loaded-revision CAS for collection-backed `PhaseOneDocument` persistence and preserve successful concurrent facts.

## 4. Regression verification

- [x] 4.1 Add focused tests for scope authorization, canonical keys, multipart binding, Local proof, SourceMaterial provenance and ProviderCall lifecycle.
- [x] 4.2 Add Temporal activity, collection concurrency and Compose config tests; run direct API tests, `openspec instructions apply`, and repository checks.
