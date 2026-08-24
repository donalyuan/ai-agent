"""共享状态的内存 UoW，仅用于测试和显式注入。"""

from __future__ import annotations

from copy import deepcopy

from video_agent_api.domain.assets import Asset, AssetVersion, AssetVersionReservation
from video_agent_api.domain.entities import Episode, Project


class _State:
    def __init__(self) -> None:
        self.projects: dict[str, Project] = {}
        self.episodes: dict[str, Episode] = {}
        self.assets: dict[str, Asset] = {}
        self.asset_versions: dict[str, AssetVersion] = {}
        self.creative_modes: dict[str, str] = {}
        self.creative_briefs: dict[str, list[object]] = {}
        self.creative_brief_current: dict[str, object] = {}
        self.creative_settings: dict[str, list[object]] = {}
        self.creative_settings_current: dict[str, object] = {}
        self.source_bindings: dict[str, list[object]] = {}
        self.source_bindings_current: dict[str, object] = {}
        self.handoff_acks: dict[str, object] = {}
        self.audit_events: list[dict[str, object]] = []
        self.outbox_events: list[dict[str, object]] = []
        self.scenes: dict[str, object] = {}
        self.shots: dict[str, object] = {}
        self.scenes_by_episode: dict[str, list[object]] = {}
        self.scene_order_revisions: dict[str, int] = {}
        self.scene_handoff_acks: dict[str, object] = {}
        self.asset_bible_entries: dict[str, object] = {}
        self.asset_bibles_by_project: dict[str, object] = {}
        self.asset_bible_by_project: dict[str, list[object]] = {}
        self.asset_bible_assignments: list[object] = []
        self.asset_bible_relationships: list[object] = []
        self.asset_bible_snapshots: dict[str, object] = {}
        self.asset_bible_tasks: dict[str, object] = {}
        self.asset_bible_impacts: dict[str, object] = {}
        self.asset_bible_impact_payloads: dict[str, dict[str, object]] = {}
        self.asset_bible_decisions: dict[str, object] = {}
        self.asset_bible_handoff_acks: dict[str, object] = {}
        self.workflow_by_project: dict[str, object] = {}
        self.workflow_bindings: dict[str, object] = {}
        self.workflow_runs: dict[str, object] = {}
        self.workflow_run_keys: dict[str, str] = {}
        self.workflow_run_key_fingerprints: dict[str, str] = {}
        self.workflow_signal_keys: dict[str, tuple[str, str]] = {}
        self.run_events: dict[str, list[object]] = {}
        self.run_input_snapshots: dict[str, object] = {}
        self.budget_gates: dict[str, object] = {}
        self.temporal_starts: dict[str, object] = {}
        self.providers: dict[str, object] = {}
        self.profiles: dict[str, object] = {}
        self.models: dict[str, object] = {}
        self.skills: list[object] = []
        self.asset_reservations: dict[str, AssetVersionReservation] = {}
        self.timeline_cuts: dict[str, object] = {}
        self.timeline_versions: dict[str, object] = {}
        self.media_inspections: dict[str, object] = {}
        self.media_derivatives: dict[str, object] = {}
        self.preview_artifacts: dict[str, object] = {}
        self.asset_edit_plans: dict[str, object] = {}
        self.asset_edit_candidates: dict[str, object] = {}
        self.asset_edit_sessions: dict[str, object] = {}
        self.asset_edit_executions: dict[str, object] = {}
        self.accept_decisions: dict[str, object] = {}
        self.edit_impacts: dict[str, object] = {}
        self.storage_profiles: dict[str, object] = {}
        self.text_review_batches: dict[str, object] = {}
        self.text_candidates: dict[str, object] = {}
        self.text_handoffs: dict[str, object] = {}
        self.text_handoff_acks: dict[str, object] = {}
        self.skill_route_decisions: dict[str, object] = {}
        self.skill_route_selections: dict[str, object] = {}
        self.source_materials: dict[str, object] = {}
        self.provider_calls: dict[str, object] = {}
        self.provider_call_keys: dict[tuple[str, str], str] = {}
        self.cost_confirmations: dict[tuple[str, str], object] = {}
        self.credential_envelopes: dict[str, object] = {}
        self.model_sync_candidates: dict[str, object] = {}
        self.skill_access_audits: list[object] = []
        self.usage_audits: list[dict[str, object]] = []
        self.catalog_overrides: dict[str, object] = {}
        self.export_jobs: dict[str, object] = {}
        self.export_batches: dict[str, object] = {}
        self.export_dispatch_outbox: dict[str, object] = {}
        self.conversations: dict[str, object] = {}
        self.image_generation_candidates: dict[tuple[str, str], object] = {}
        self.video_operations: dict[tuple[str, str], object] = {}
        self.video_take_candidates: dict[str, object] = {}


class InMemoryProjectRepository:
    def __init__(self, state: _State) -> None:
        self._state = state

    async def get(self, project_id: str) -> Project | None:
        return self._state.projects.get(project_id)

    async def list(self) -> list[Project]:
        return list(self._state.projects.values())

    async def add(self, project: Project) -> None:
        self._state.projects[project.id] = project

    async def save(self, project: Project) -> None:
        self._state.projects[project.id] = project


class InMemoryEpisodeRepository:
    def __init__(self, state: _State) -> None:
        self._state = state

    async def get(self, episode_id: str) -> Episode | None:
        return self._state.episodes.get(episode_id)

    async def list_by_project(self, project_id: str) -> list[Episode]:
        return [value for value in self._state.episodes.values() if value.project_id == project_id]

    async def add(self, episode: Episode) -> None:
        self._state.episodes[episode.id] = episode

    async def save(self, episode: Episode) -> None:
        self._state.episodes[episode.id] = episode


class InMemoryAssetRepository:
    def __init__(self, state: _State) -> None:
        self._state = state

    async def get(self, asset_id: str) -> Asset | None:
        return self._state.assets.get(asset_id)

    async def list_by_project(self, project_id: str) -> list[Asset]:
        return [value for value in self._state.assets.values() if value.project_id == project_id]

    async def add(self, asset: Asset) -> None:
        self._state.assets[asset.id] = asset

    async def save(self, asset: Asset) -> None:
        self._state.assets[asset.id] = asset


class InMemoryAssetVersionRepository:
    def __init__(self, state: _State) -> None:
        self._state = state

    async def get(self, version_id: str) -> AssetVersion | None:
        return self._state.asset_versions.get(version_id)

    async def list_by_asset(self, asset_id: str) -> list[AssetVersion]:
        return [
            value for value in self._state.asset_versions.values() if value.asset_id == asset_id
        ]

    async def next_version_number(self, asset_id: str) -> int:
        versions = await self.list_by_asset(asset_id)
        return max((value.version_number for value in versions), default=0) + 1

    async def add(self, version: AssetVersion) -> None:
        self._state.asset_versions[version.id] = version


class InMemoryUnitOfWork:
    def __init__(self, state: _State | None = None) -> None:
        self.state = state or _State()
        self._snapshot: dict[str, object] | None = None
        self._bind_state()

    def _bind_state(self) -> None:
        """让 repository 与 owner maps 始终指向当前 state。"""
        self.projects = InMemoryProjectRepository(self.state)
        self.episodes = InMemoryEpisodeRepository(self.state)
        self.assets = InMemoryAssetRepository(self.state)
        self.asset_versions = InMemoryAssetVersionRepository(self.state)
        self.creative_modes = self.state.creative_modes
        self.creative_briefs = self.state.creative_briefs
        self.creative_brief_current = self.state.creative_brief_current
        self.creative_settings = self.state.creative_settings
        self.creative_settings_current = self.state.creative_settings_current
        self.source_bindings = self.state.source_bindings
        self.source_bindings_current = self.state.source_bindings_current
        self.handoff_acks = self.state.handoff_acks
        self.audit_events = self.state.audit_events
        self.outbox_events = self.state.outbox_events
        self.scenes = self.state.scenes
        self.shots = self.state.shots
        self.scenes_by_episode = self.state.scenes_by_episode
        self.scene_order_revisions = self.state.scene_order_revisions
        self.scene_handoff_acks = self.state.scene_handoff_acks
        self.asset_bible_entries = self.state.asset_bible_entries
        self.asset_bibles_by_project = self.state.asset_bibles_by_project
        self.asset_bible_by_project = self.state.asset_bible_by_project
        self.asset_bible_assignments = self.state.asset_bible_assignments
        self.asset_bible_relationships = self.state.asset_bible_relationships
        self.asset_bible_snapshots = self.state.asset_bible_snapshots
        self.asset_bible_tasks = self.state.asset_bible_tasks
        self.asset_bible_impacts = self.state.asset_bible_impacts
        self.asset_bible_impact_payloads = self.state.asset_bible_impact_payloads
        self.asset_bible_decisions = self.state.asset_bible_decisions
        self.asset_bible_handoff_acks = self.state.asset_bible_handoff_acks
        self.workflow_by_project = self.state.workflow_by_project
        self.workflow_bindings = self.state.workflow_bindings
        self.workflow_runs = self.state.workflow_runs
        self.workflow_run_keys = self.state.workflow_run_keys
        self.workflow_run_key_fingerprints = self.state.workflow_run_key_fingerprints
        self.workflow_signal_keys = self.state.workflow_signal_keys
        self.run_events = self.state.run_events
        self.run_input_snapshots = self.state.run_input_snapshots
        self.budget_gates = self.state.budget_gates
        self.temporal_starts = self.state.temporal_starts
        self.providers = self.state.providers
        self.profiles = self.state.profiles
        self.models = self.state.models
        self.skills = self.state.skills
        self.asset_reservations = self.state.asset_reservations
        self.timeline_cuts = self.state.timeline_cuts
        self.timeline_versions = self.state.timeline_versions
        self.media_inspections = self.state.media_inspections
        self.media_derivatives = self.state.media_derivatives
        self.preview_artifacts = self.state.preview_artifacts
        self.asset_edit_plans = self.state.asset_edit_plans
        self.asset_edit_candidates = self.state.asset_edit_candidates
        self.asset_edit_sessions = self.state.asset_edit_sessions
        self.asset_edit_executions = self.state.asset_edit_executions
        self.accept_decisions = self.state.accept_decisions
        self.edit_impacts = self.state.edit_impacts
        self.storage_profiles = self.state.storage_profiles
        self.text_review_batches = self.state.text_review_batches
        self.text_candidates = self.state.text_candidates
        self.text_handoffs = self.state.text_handoffs
        self.text_handoff_acks = self.state.text_handoff_acks
        self.skill_route_decisions = self.state.skill_route_decisions
        self.skill_route_selections = self.state.skill_route_selections
        self.source_materials = self.state.source_materials
        self.provider_calls = self.state.provider_calls
        self.provider_call_keys = self.state.provider_call_keys
        self.cost_confirmations = self.state.cost_confirmations
        self.credential_envelopes = self.state.credential_envelopes
        self.model_sync_candidates = self.state.model_sync_candidates
        self.skill_access_audits = self.state.skill_access_audits
        self.usage_audits = self.state.usage_audits
        self.catalog_overrides = self.state.catalog_overrides
        self.export_jobs = self.state.export_jobs
        self.export_batches = self.state.export_batches
        self.export_dispatch_outbox = self.state.export_dispatch_outbox
        self.conversations = self.state.conversations
        self.image_generation_candidates = self.state.image_generation_candidates
        self.video_operations = self.state.video_operations
        self.video_take_candidates = self.state.video_take_candidates
        self.commits = 0

    def __call__(self) -> InMemoryUnitOfWork:
        return type(self)(self.state)

    async def __aenter__(self) -> InMemoryUnitOfWork:
        self._snapshot = deepcopy(self.state.__dict__)
        return self

    async def __aexit__(self, exc_type: object, exc: object, tb: object) -> None:
        if exc_type is not None:
            await self.rollback()

    async def commit(self) -> None:
        self.commits += 1

    async def rollback(self) -> None:
        if self._snapshot is not None:
            self.state.__dict__.clear()
            self.state.__dict__.update(deepcopy(self._snapshot))
            self._bind_state()
            self._snapshot = None


def in_memory_uow_factory() -> InMemoryUnitOfWork:
    # Factory closure is useful to callers that need one shared state per app.
    return InMemoryUnitOfWork()
