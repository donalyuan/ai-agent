## ADDED Requirements

### Requirement: Temporal text execution closes the review gate
文本 Run 的 Temporal activity SHALL 调用 `TextGenerationService.generate()`，并在生成完整候选图后通过 `RunsService.enter_text_review` 将 matching `text.generate` NodeRun 和 Run 置于 `waiting_review`。

#### Scenario: Text run enters review
- **WHEN** a running text node is dispatched with a configured Mock/Local selection
- **THEN** one idempotent `TextReviewBatch` is persisted and the node/run expose `waiting_review` with its batch evidence

#### Scenario: Activity is scope safe
- **WHEN** the Temporal payload references a missing, foreign, or stale Project/Run/Brief snapshot
- **THEN** generation is rejected without candidates or review-state transition

### Requirement: Collection documents use revision CAS
collection-backed owner documents SHALL persist updates with the revision loaded at UoW entry in the `WHERE` predicate; a concurrent revision mismatch SHALL raise a conflict and SHALL NOT delete or overwrite the current collection payload.

#### Scenario: Concurrent append preserves both facts
- **WHEN** two UoWs append different audit/outbox/text facts from the same loaded collection revision
- **THEN** one commit succeeds and the other returns a revision conflict, leaving the successful payload intact for retry/reconciliation

### Requirement: Text provider calls are catalog facts
Text generation SHALL create a pending catalog `ProviderCall` before invoking the model Port and SHALL finalize it with request id, filtered native usage, capability snapshot and a type-only failure diagnostic.

#### Scenario: Successful text call is queryable
- **WHEN** the model returns a valid candidate graph and usage
- **THEN** `provider-calls` projection contains one succeeded call keyed by run/logical operation with stable request fingerprint and usage

#### Scenario: Failed text call is queryable
- **WHEN** the model Port or candidate validation fails
- **THEN** the call is finalized as failed with a脱敏 diagnostic and no candidate batch is committed

### Requirement: Source bindings use owner facts
Creative source binding SHALL compare the submitted snapshot with the same-project SourceMaterial current version, including revision, content hash and parse/validation state; caller-supplied status or hash SHALL NOT establish provenance.

#### Scenario: Forged source binding is rejected
- **WHEN** a binding names an unknown/foreign SourceMaterial or mismatches its current version facts
- **THEN** the API rejects the command and leaves the current binding unchanged

### Requirement: Uploaded source facts derive from AssetVersion
An uploaded SourceMaterial version SHALL derive content hash and verified parse/validation state from a same-project AssetVersion whose storage metadata is complete and status is allowed; request-provided hash/status SHALL be ignored or rejected.

#### Scenario: Registered upload becomes usable
- **WHEN** an uploaded source references a same-project registered AssetVersion with matching checksum metadata
- **THEN** the persisted SourceMaterialVersion stores that AssetVersion content hash with `parsed` and `valid` statuses

#### Scenario: Foreign or incomplete upload is rejected
- **WHEN** the AssetVersion is missing, foreign, rejected, or has inconsistent checksum metadata
- **THEN** append fails without advancing SourceMaterial revision
