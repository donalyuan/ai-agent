# assets-slice Specification

## Purpose
TBD - created by archiving change implement-assets-asset-versions-slice. Update Purpose after archive.
## Requirements
### Requirement: Asset identity and shared kind boundary
系统 SHALL 创建项目归属的 `Asset`，kind 仅允许 `image`、`video`、`audio`、`text`、`document` 五值；`audio` 不得扩展为独立实体。Asset SHALL 以 revision/CAS 管理 `AssetCatalogMetadata`：`sourceType=user_upload|provider_generated|source_material|imported`、可选 `catalogRole=character|location|prop|storyboard|video_take|dialogue|music|ambience|effects|other`、有界 tags、`authorizationStatus=unknown|declared|verified|restricted|expired` 及可选 copyright/license label/reference。目录元数据更新 MUST 产生审计且 MUST NOT 修改任何 AssetVersion、StoredObject 或历史交付 provenance。

#### Scenario: create asset
- **WHEN** application receives a non-blank name, existing project id, shared kind and valid catalog metadata
- **THEN** it persists a draft Asset with stable id, revision 1 and project-scoped catalog metadata

#### Scenario: reject unknown kind or missing project
- **WHEN** kind/catalog enum is outside the allowed values, tags exceed bounds, authorization fields conflict, or project does not exist
- **THEN** it returns a stable validation or project-not-found domain error and writes nothing

#### Scenario: update metadata with CAS without rewriting versions
- **WHEN** caller submits valid catalog metadata with current expected Asset revision
- **THEN** Assets owner saves one new Asset revision/audit while every existing AssetVersion and frozen manifest provenance remains byte-for-byte unchanged

### Requirement: Append-only asset versions
系统 SHALL only append `AssetVersion`; server assigns `versionNumber` starting at 1 within each Asset, and existing versions SHALL remain immutable.

#### Scenario: append versions
- **WHEN** one or more valid storage objects are appended to an Asset
- **THEN** version numbers are 1..N, reads return metadata only, and list order is ascending

#### Scenario: reject in-place update and concurrent duplicate
- **WHEN** caller tries to update an existing version or two transactions choose the same number
- **THEN** immutable error or stable conflict is raised, protected by unique `(asset_id, version_number)`

#### Scenario: nested metadata cannot mutate a persisted fact
- **WHEN** a caller attempts to assign an `AssetVersion`/`StorageObject` field or mutate nested `media`
- **THEN** the domain object rejects the mutation and in-memory/ORM/HTTP boundaries preserve the immutable fact

### Requirement: Storage reference safety
系统 SHALL persist only storage reference, checksum, MIME, non-negative size and optional media metadata; objectKey MUST be relative and reject absolute/path traversal forms; media bytes MUST NOT be accepted or stored.

#### Scenario: unsafe object reference
- **WHEN** objectKey is absolute, drive-qualified, UNC, or escapes via `..`
- **THEN** validation fails before repository write

#### Scenario: workspace legacy reference normalization
- **WHEN** legacy `storage_ref` is `workspace://projects/a/v1.mp4` during `0004` or `0006`
- **THEN** migration stores provider `local_workspace`, bucket `workspace`, and object key `projects/a/v1.mp4`; unsafe URI/path forms fail explicitly

### Requirement: Independent content and storage hashes
系统 SHALL persist `AssetVersion.contentHash` independently from `storageObject.checksum` and SHALL return the original values on reads.

#### Scenario: hashes remain distinct
- **WHEN** a valid version is appended with different contentHash and storageObject.checksum values
- **THEN** get/list responses preserve both values without substitution

### Requirement: Migration integrity and relational constraints
系统 SHALL make version project ownership, positive version numbers, non-negative sizes, supported asset kinds, non-blank asset names and hexadecimal hash values enforceable after migration. `AssetVersion.projectId` SHALL match the project owned by its `Asset` at the database boundary.

#### Scenario: legacy rows are migrated without fabricated integrity data
- **WHEN** a legacy asset version has a real checksum
- **THEN** migration backfills project/storage metadata and contentHash from that checksum while preserving the row
- **WHEN** a legacy asset version has no checksum
- **THEN** the `0004`/`0005` migration sequence fails explicitly before committing schema changes instead of fabricating a hash
- **WHEN** a version references a different or nonexistent project than its Asset, or a checksum/content hash is 64 characters but not hexadecimal
- **THEN** the database rejects the row and migration rejects malformed legacy integrity data before applying the new constraints

#### Scenario: repair migration is replayable
- **WHEN** an already-applied legacy `0004`/`0005` row contains a workspace URI in `object_key`
- **THEN** `0006` repairs it and the `0005 -> 0006 -> 0005 -> 0006` cycle remains valid

### Requirement: Recoverable AssetVersion reservation
Assets owner SHALL 提供 project/asset-scoped `AssetVersionReservation`，冻结 reservation ID、expected Asset revision、declared kind/MIME/size/checksum、StorageProfile snapshot、canonical upload key、operation key 和状态 `reserved|registered|cancelled|failed`。同一 reservation/fingerprint MUST 至多登记一个 AssetVersion；取消或 Storage late result MUST NOT 自动登记版本。

#### Scenario: register one version from a verified reservation
- **WHEN** Storage owner returns a verified StoredObjectRef matching the active reservation and operation fingerprint
- **THEN** Assets owner appends one immutable AssetVersion, marks reservation registered and returns the same version on retry

#### Scenario: reject conflicting or cancelled reservation
- **WHEN** reservation is foreign/stale/cancelled, declared/observed metadata differs, or same key carries another fingerprint
- **THEN** Assets owner returns stable conflict/validation, does not append a version and preserves any late object as unreferenced storage evidence
