## ADDED Requirements

### Requirement: Multipart operation reuse is fully bound
Reusing a multipart `operation_key` SHALL compare project, profile, canonical object key, expected size, checksum and MIME against the persisted session before returning a session reference or completing it.

#### Scenario: Cross-project reuse is rejected
- **WHEN** a second project reuses an existing operation key with any differing frozen field
- **THEN** storage rejects the request and never returns a session pointing at the first project's object

### Requirement: Object keys use the canonical contract
Storage intent and reference validation SHALL call the shared `canonical_object_key()` contract and reject dot segments, empty segments, trailing slash, query and fragment delimiters, schemes, absolute paths and backslashes.

#### Scenario: Non-canonical key is rejected
- **WHEN** a caller supplies `projects/p/a/./file`, `projects/p/a//file`, `projects/p/file?x` or `projects/p/a/`
- **THEN** storage rejects the intent before creating a session or file

### Requirement: Local proof is fail closed
`LocalWorkspaceAdapter` SHALL NOT issue a successful no-reference `DeleteProof` because it cannot query all owner reference indexes; callers SHALL use a complete composite owner proof.

#### Scenario: Proof index unavailable
- **WHEN** a caller asks Local storage to prove an object has no references
- **THEN** storage raises an object-in-use/unsafe proof error and deletion cannot proceed

### Requirement: Compose Local workspace is shared
When `STORAGE_MODE=local_workspace`, API and Media Worker SHALL mount the same named workspace volume at the configured `WORKSPACE_ROOT`.

#### Scenario: Worker reads API upload
- **WHEN** API writes a Local workspace object and Media Worker materializes the same reference
- **THEN** both containers resolve the same bytes and checksum
