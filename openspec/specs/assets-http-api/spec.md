# assets-http-api Specification

## Purpose
TBD - created by archiving change implement-assets-asset-versions-slice. Update Purpose after archive.
## Requirements
### Requirement: Asset HTTP endpoints
系统 SHALL expose `POST /v1/projects/{projectId}/assets`, cursor-paginated `GET /v1/projects/{projectId}/assets`, `GET /v1/assets/{assetId}` and CAS `PATCH /v1/assets/{assetId}` with camelCase JSON and stable 404/409/422/503 errors。create/patch SHALL accept owner-defined AssetCatalogMetadata；list SHALL support bounded kind、catalogRole、tag、sourceType、authorizationStatus 和 processingStatus filters，并以 `(updatedAt,id)` 稳定排序。读取、分页和筛选 MUST 无 mutation。

#### Scenario: create and list asset
- **WHEN** a client posts a valid `kind`, `name` and catalog metadata for an existing project
- **THEN** API returns 201 and subsequent list/get responses use shared-schema `schemaVersion`, `projectId`, stable identifiers, owner revision and safe metadata without bytes/objectKey/presigned URL

#### Scenario: filter and paginate assets without mutation
- **WHEN** a client requests bounded filters and a valid cursor for the same project
- **THEN** API returns only matching authorized assets with a stable next cursor and creates no UploadSession, AssetVersion, ProviderCall, RunEvent or derivative

#### Scenario: patch metadata with If-Match
- **WHEN** a client patches valid tags/catalogRole/authorization metadata with current `expectedRevision`/`If-Match`
- **THEN** API returns the new Asset revision/audit projection and leaves all AssetVersions unchanged

#### Scenario: reject foreign or stale asset operation
- **WHEN** project/asset scope is foreign, cursor/filter is invalid or expected revision is stale
- **THEN** API returns stable 404/403/422/409 with zero Asset/AssetVersion/Storage/Outbox partial write

### Requirement: Asset version HTTP endpoints
系统 SHALL expose `POST /v1/assets/{assetId}/versions`, `GET /v1/assets/{assetId}/versions` and `GET /v1/asset-versions/{versionId}`. POST SHALL append only and response SHALL include `versionNumber` and storage metadata without bytes.

#### Scenario: append and retrieve version
- **WHEN** a client posts a valid `contentHash` and `storageObject` to an existing asset
- **THEN** API returns 201 with `versionNumber` and metadata-only storage object, and get/list can retrieve it

#### Scenario: camelCase contract
- **WHEN** a valid request uses `projectId`, `storageObject.objectKey`, `mimeType`, `sizeBytes` and optional `media`
- **THEN** response uses the same transport names while persistence uses explicit snake_case mapping

#### Scenario: invalid request and unavailable database
- **WHEN** payload violates kind/hash/path/size rules or business UoW is absent
- **THEN** API returns 422 with validation code or 503 with `database_unavailable`, never silently switching to memory

#### Scenario: missing resources
- **WHEN** a client requests an unknown asset or asset version
- **THEN** API returns 404 with a stable not-found error code

### Requirement: Project scope header is mandatory
All phase-one project-scoped creative, scene, text-generation, video-generation, agent-edit and Asset Center routes SHALL reject a missing or empty `X-Project-Scope` header before reading or mutating project data; a mismatched header SHALL return forbidden.

#### Scenario: Missing scope is rejected
- **WHEN** a client calls a project-scoped route without `X-Project-Scope`
- **THEN** API returns 403 and performs no owner read or write

#### Scenario: Foreign scope is rejected
- **WHEN** a client supplies a scope header different from the project path/body
- **THEN** API returns 403 and performs no owner read or write

### Requirement: Asset upload reservation endpoints
系统 SHALL expose owner commands/queries for create/read/cancel/reconcile of an `AssetVersionReservation` and SHALL return the stable `operationKey=asset-upload:{projectId}:{assetId}:{reservationId}` needed by Storage owner。HTTP MUST NOT proxy media bytes through business DTO or report registered before verified StoredObjectRef is appended by Assets owner。

#### Scenario: create and resume an upload reservation
- **WHEN** caller creates a valid project/asset reservation or re-reads it after refresh
- **THEN** API returns the same reservation ID/revision/fingerprint/operation key and authoritative Storage/registration status without creating a second reservation

#### Scenario: cancel without late registration
- **WHEN** caller cancels an active reservation with current revision and Storage later reports terminal completion
- **THEN** API preserves cancelled/unreferenced diagnostic and does not append AssetVersion or change current business references

### Requirement: Asset usage and media projection endpoints
系统 SHALL expose project-scoped read endpoints for exact AssetVersion usage and MediaInspection/Derivative summaries. Usage response SHALL include owner type/ID/revision/scope/state/source hash/deep link or explicit unavailable/partial diagnostic；media response SHALL only expose safe metadata/readiness and short-lived grants through the owning authorization path，MUST NOT expose objectKey、workspace URI or persisted presigned URL。

#### Scenario: read exact usage and ready media summary
- **WHEN** same-project owner references and matching ready derivative facts are available
- **THEN** API returns revision/hash-bound usage plus safe proxy/thumbnail/waveform readiness and authorized short TTL access

#### Scenario: fail closed on unavailable usage or stale derivative
- **WHEN** owner query is unavailable, usage revision is unverifiable, derivative fingerprint is stale or authorization fails
- **THEN** API returns explicit unavailable/partial/stale/forbidden diagnostic rather than an empty usage list or usable media URL
