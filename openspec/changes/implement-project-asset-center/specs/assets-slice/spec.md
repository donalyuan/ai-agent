## MODIFIED Requirements

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

## ADDED Requirements

### Requirement: Recoverable AssetVersion reservation
Assets owner SHALL 提供 project/asset-scoped `AssetVersionReservation`，冻结 reservation ID、expected Asset revision、declared kind/MIME/size/checksum、StorageProfile snapshot、canonical upload key、operation key 和状态 `reserved|registered|cancelled|failed`。同一 reservation/fingerprint MUST 至多登记一个 AssetVersion；取消或 Storage late result MUST NOT 自动登记版本。

#### Scenario: register one version from a verified reservation
- **WHEN** Storage owner returns a verified StoredObjectRef matching the active reservation and operation fingerprint
- **THEN** Assets owner appends one immutable AssetVersion, marks reservation registered and returns the same version on retry

#### Scenario: reject conflicting or cancelled reservation
- **WHEN** reservation is foreign/stale/cancelled, declared/observed metadata differs, or same key carries another fingerprint
- **THEN** Assets owner returns stable conflict/validation, does not append a version and preserves any late object as unreferenced storage evidence
