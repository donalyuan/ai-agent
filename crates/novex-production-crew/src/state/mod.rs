//! ProductionState 管理模块：产物 CRUD、版本管理、协作建议

pub mod artifacts;
pub mod collaboration;
pub mod repository;
pub mod versioning;

pub use collaboration::CollaborationManager;
pub use repository::ProductionStateRepository;
pub use versioning::ArtifactVersionManager;
