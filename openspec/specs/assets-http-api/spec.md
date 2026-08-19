# assets-http-api Specification

## Purpose
TBD - created by archiving change implement-assets-asset-versions-slice. Update Purpose after archive.
## Requirements
### Requirement: Asset HTTP endpoints
系统 SHALL expose `POST /v1/projects/{projectId}/assets`, `GET /v1/projects/{projectId}/assets` and `GET /v1/assets/{assetId}` with camelCase JSON and stable 404/422/503 errors.

#### Scenario: create and list asset
- **WHEN** a client posts a valid `kind` and `name` for an existing project
- **THEN** API returns 201 and subsequent list/get responses use shared-schema `schema_version`, `projectId` and stable identifiers

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
