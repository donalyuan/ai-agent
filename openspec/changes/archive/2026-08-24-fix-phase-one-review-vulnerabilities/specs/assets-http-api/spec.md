## ADDED Requirements

### Requirement: Project scope header is mandatory
All phase-one project-scoped creative, scene, text-generation, video-generation, agent-edit and Asset Center routes SHALL reject a missing or empty `X-Project-Scope` header before reading or mutating project data; a mismatched header SHALL return forbidden.

#### Scenario: Missing scope is rejected
- **WHEN** a client calls a project-scoped route without `X-Project-Scope`
- **THEN** API returns 403 and performs no owner read or write

#### Scenario: Foreign scope is rejected
- **WHEN** a client supplies a scope header different from the project path/body
- **THEN** API returns 403 and performs no owner read or write
