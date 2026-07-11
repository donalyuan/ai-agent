"use client";

import { type FormEvent, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { WorkspaceShell } from "./components/workspace/WorkspaceShell";
import { ModelSelect } from "./components/models/ModelSelect";
import {
  modelSelectionUnavailable,
  reconcileModelSelection,
} from "./components/models/modelSelection";
import {
  AgentMessage,
  ApiClient,
  ApiError,
  AssetGenerationPlanResponse,
  AssetGenerationTask,
  ContentTopic,
  ContentTopicSource,
  ContentTopicStats,
  ContentTopicStatus,
  Material,
  MaterialStatus,
  MaterialStatusFilter,
  MaterialType,
  ModelOption,
  PrepareScriptFromTopicResponse,
  Project,
  SceneAssetCandidate,
  ScriptDetail,
  ScriptStatus,
  ScriptSummary,
  ScriptStyle,
  TopicGenerationBatchSummary,
  TopicGroupSort,
  TopicGroupSummary,
  TopicQualityEvaluation,
  TopicReviewSnapshot,
  WorkspaceMenuNode,
  checkHealth,
  confirmAssetGenerationTask,
  createAgentConversation,
  createApiClient,
  createAssetGenerationTasks,
  createContentTopic,
  createMaterial,
  createSceneAssetGenerationTask,
  createTopicGroupReview,
  deleteContentTopic,
  dismissAssetGenerationTask,
  generateStrategyProfileDraft,
  generateScript,
  getAssetGenerationPlan,
  getLatestTopicQualityEvaluation,
  getLatestTopicGroupReview,
  getMaterial,
  getScript,
  getScriptAgentTurnMetadata,
  listAssetCandidates,
  listAssetGenerationTasks,
  listMaterials,
  listModelOptions,
  listProjects,
  listScripts,
  listContentTopics,
  listTopicGenerationBatches,
  listTopicGroups,
  listWorkspaceMenus,
  prepareScriptFromTopic,
  rejectAssetCandidate,
  sendAgentMessage,
  selectAssetCandidate,
  updateContentTopic,
  updateContentTopicStatus,
  updateMaterial,
  updateMaterialStatus,
  updateProjectStrategyProfile,
  updateScriptStatus,
} from "./lib/api";
import { AccountStrategyPage } from "./pages/content-strategy/AccountStrategyPage";
import { ContentStrategyPage, ScriptPreparationDialog } from "./pages/content-strategy/ContentStrategyPage";
import { TopicHistoryPage } from "./pages/content-strategy/TopicHistoryPage";
import {
  adjustTopicStats,
  accountStrategyPayloadFromForm,
  defaultTopicForm,
  defaultAccountStrategyForm,
  emptyTopicStats,
  projectToAccountStrategyForm,
  sortContentTopicsByScore,
  topicPayloadFromForm,
  topicToForm,
  type AccountStrategyFormState,
  type ContentStrategyView,
  type TopicFormState,
} from "./pages/content-strategy/topicModel";
import { ScriptCreationPage } from "./pages/script-creation/ScriptCreationPage";
import {
  assetGenerationPayload,
  mergeUpdatedCandidate,
  upsertAssetTask,
} from "./pages/script-creation/assetModel";
import { upsertSummary } from "./pages/script-creation/scriptModel";
import { AssetGenerationPage } from "./pages/asset-generation/AssetGenerationPage";
import { MaterialLibraryPage } from "./pages/material-library/MaterialLibraryPage";
import {
  defaultMaterialForm,
  materialPayloadFromForm,
  materialToForm,
  type MaterialFormState,
} from "./pages/material-library/materialModel";

const contentStrategyMenuKey = "content-strategy";
const accountStrategyMenuKey = "account-strategy";
const topicHistoryMenuKey = "topic-history";
const topicGeneratorMenuKey = "topic-generator";
const materialManagementMenuKey = "material-management";
const materialLibraryMenuKey = "material-library";
const assetGenerationMenuKey = "asset-generation";
const scriptCreationMenuKey = "script-creation";
const scriptGeneratorMenuKey = "script-generator";
const defaultMenuKey = contentStrategyMenuKey;

function visibleTopicGenerationBatches(batches: TopicGenerationBatchSummary[]) {
  return batches.filter((batch) => batch.status === "succeeded" && batch.topic_count > 0);
}

function topicBatchRootEntries(batches: TopicGenerationBatchSummary[]) {
  return batches.filter((batch) => !batch.supplement_of_batch_id);
}

function topicBatchGroupIds(batchId: string | null, batches: TopicGenerationBatchSummary[]) {
  if (!batchId) {
    return [];
  }
  const selectedBatch = batches.find((batch) => batch.batch_id === batchId);
  if (!selectedBatch) {
    return [batchId];
  }
  const rootBatchId = selectedBatch.supplement_of_batch_id || selectedBatch.batch_id;
  const supplementIds = batches
    .filter((batch) => batch.supplement_of_batch_id === rootBatchId)
    .map((batch) => batch.batch_id);
  return [rootBatchId, ...supplementIds];
}

function topicBatchRootId(batchId: string | null, batches: TopicGenerationBatchSummary[]) {
  if (!batchId) {
    return null;
  }
  const selectedBatch = batches.find((batch) => batch.batch_id === batchId);
  return selectedBatch?.supplement_of_batch_id || selectedBatch?.batch_id || batchId;
}

function sumTopicStats(responses: { stats: ContentTopicStats }[]) {
  return responses.reduce<ContentTopicStats>(
    (totalStats, response) => ({
      total: totalStats.total + response.stats.total,
      idea: totalStats.idea + response.stats.idea,
      approved: totalStats.approved + response.stats.approved,
      scripted: totalStats.scripted + response.stats.scripted,
      archived: totalStats.archived + response.stats.archived,
    }),
    emptyTopicStats,
  );
}

async function listContentTopicsForBatchGroup(
  client: ApiClient,
  projectId: string,
  filters: ReturnType<typeof topicListFilters>,
  batchIds: string[],
) {
  if (batchIds.length <= 1) {
    return listContentTopics(client, projectId, filters);
  }

  const responses = await Promise.all(
    batchIds.map((batchId) => listContentTopics(client, projectId, { ...filters, batch_id: batchId })),
  );
  const topicsById = new Map<string, ContentTopic>();
  for (const topic of responses.flatMap((response) => response.topics)) {
    topicsById.set(topic.topic_id, topic);
  }
  return {
    topics: Array.from(topicsById.values()),
    stats: sumTopicStats(responses),
  };
}

function accountStrategyFormsEqual(left: AccountStrategyFormState, right: AccountStrategyFormState) {
  return (Object.keys(defaultAccountStrategyForm) as Array<keyof AccountStrategyFormState>).every(
    (field) => left[field] === right[field],
  );
}

function createIdempotencyKey() {
  const bytes = new Uint8Array(16);
  crypto.getRandomValues(bytes);
  bytes[6] = (bytes[6] & 0x0f) | 0x40;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;
  const value = Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
  return `${value.slice(0, 8)}-${value.slice(8, 12)}-${value.slice(12, 16)}-${value.slice(16, 20)}-${value.slice(20)}`;
}

export default function Home() {
  const client = useMemo(() => createApiClient(), []);
  const [apiAvailable, setApiAvailable] = useState<boolean | null>(null);
  const [projects, setProjects] = useState<Project[]>([]);
  const [selectedProjectId, setSelectedProjectId] = useState("");
  const [scripts, setScripts] = useState<ScriptSummary[]>([]);
  const [selectedScript, setSelectedScript] = useState<ScriptDetail | null>(null);
  const [selectedScriptId, setSelectedScriptId] = useState<string | null>(null);
  const [assetCandidates, setAssetCandidates] = useState<SceneAssetCandidate[]>([]);
  const [assetTasks, setAssetTasks] = useState<AssetGenerationTask[]>([]);
  const [assetPlan, setAssetPlan] = useState<AssetGenerationPlanResponse | null>(null);
  const [assetCandidateCount, setAssetCandidateCount] = useState(3);
  const [useReferenceMaterials, setUseReferenceMaterials] = useState(true);
  const [selectedAssetSceneId, setSelectedAssetSceneId] = useState<string | null>(null);
  const [loadingAssetPlan, setLoadingAssetPlan] = useState(false);
  const [loadingAssetCandidates, setLoadingAssetCandidates] = useState(false);
  const [assetActionInProgress, setAssetActionInProgress] = useState(false);
  const [assetError, setAssetError] = useState("");
  const [assetTaskToDismissId, setAssetTaskToDismissId] = useState<string | null>(null);
  const [dismissingAssetTaskId, setDismissingAssetTaskId] = useState<string | null>(null);
  const [topics, setTopics] = useState<ContentTopic[]>([]);
  const [materials, setMaterials] = useState<Material[]>([]);
  const [selectedMaterialId, setSelectedMaterialId] = useState<string | null>(null);
  const [loadingMaterials, setLoadingMaterials] = useState(false);
  const [materialError, setMaterialError] = useState("");
  const [materialActionError, setMaterialActionError] = useState("");
  const [savingMaterial, setSavingMaterial] = useState(false);
  const [creatingMaterial, setCreatingMaterial] = useState(false);
  const [materialFilters, setMaterialFilters] = useState<{
    material_type: MaterialType | "all";
    status: MaterialStatusFilter;
    q: string;
    tag: string;
  }>({
    material_type: "all",
    status: "active",
    q: "",
    tag: "",
  });
  const [materialForm, setMaterialForm] = useState<MaterialFormState>(defaultMaterialForm);
  const [topicStats, setTopicStats] = useState<ContentTopicStats>(emptyTopicStats);
  const [topicBatches, setTopicBatches] = useState<TopicGenerationBatchSummary[]>([]);
  const [topicGroups, setTopicGroups] = useState<TopicGroupSummary[]>([]);
  const [topicBatchesLoaded, setTopicBatchesLoaded] = useState(false);
  const [topicGroupsLoaded, setTopicGroupsLoaded] = useState(false);
  const [topicGroupSort, setTopicGroupSort] = useState<TopicGroupSort>("script_priority");
  const [topicBatchViewMode, setTopicBatchViewMode] = useState<"latest" | "batch" | "all">("latest");
  const [contentStrategyView, setContentStrategyView] = useState<ContentStrategyView>("pool");
  const [historyTopicBatchId, setHistoryTopicBatchId] = useState<string | null>(null);
  const [topicReviewSnapshot, setTopicReviewSnapshot] = useState<TopicReviewSnapshot | null>(null);
  const [topicReviewLoading, setTopicReviewLoading] = useState(false);
  const [topicReviewError, setTopicReviewError] = useState("");
  const [topicQualityEvaluation, setTopicQualityEvaluation] =
    useState<TopicQualityEvaluation | null>(null);
  const [topicQualityLoading, setTopicQualityLoading] = useState(false);
  const [topicQualityError, setTopicQualityError] = useState("");
  const [selectedTopicId, setSelectedTopicId] = useState<string | null>(null);
  const [topicStatusFilter, setTopicStatusFilter] = useState<"all" | ContentTopicStatus>("all");
  const [topicSourceFilter] = useState<"all" | ContentTopicSource>("all");
  const [topicBatchFilter, setTopicBatchFilter] = useState<string | null>(null);
  const [workspaceMenus, setWorkspaceMenus] = useState<WorkspaceMenuNode[]>([]);
  const [selectedMenuKey, setSelectedMenuKey] = useState(defaultMenuKey);
  const [selectedMaterialSubMenuKey, setSelectedMaterialSubMenuKey] = useState(materialLibraryMenuKey);
  const [selectedScriptSubMenuKey, setSelectedScriptSubMenuKey] = useState(scriptGeneratorMenuKey);
  const [statusFilter, setStatusFilter] = useState<"all" | ScriptStatus>("all");
  const [loadingMenus, setLoadingMenus] = useState(true);
  const [loadingProjects, setLoadingProjects] = useState(true);
  const [loadingScripts, setLoadingScripts] = useState(false);
  const [loadingScriptDetail, setLoadingScriptDetail] = useState(false);
  const [loadingTopics, setLoadingTopics] = useState(false);
  const [updatingStatus, setUpdatingStatus] = useState(false);
  const [projectError, setProjectError] = useState("");
  const [menuError, setMenuError] = useState("");
  const [scriptError, setScriptError] = useState("");
  const [statusError, setStatusError] = useState("");
  const [topicError, setTopicError] = useState("");
  const [topicBatchError, setTopicBatchError] = useState("");
  const [topicGroupError, setTopicGroupError] = useState("");
  const [topicActionError, setTopicActionError] = useState("");
  const [accountStrategyError, setAccountStrategyError] = useState("");
  const [accountStrategyForm, setAccountStrategyForm] =
    useState<AccountStrategyFormState>(defaultAccountStrategyForm);
  const [accountStrategyDraftNotes, setAccountStrategyDraftNotes] = useState("");
  const [accountStrategyDraftSummary, setAccountStrategyDraftSummary] = useState("");
  const [generatingAccountStrategyDraft, setGeneratingAccountStrategyDraft] = useState(false);
  const [savingAccountStrategy, setSavingAccountStrategy] = useState(false);
  const [deletingTopicId, setDeletingTopicId] = useState<string | null>(null);
  const [showTopicForm, setShowTopicForm] = useState(false);
  const [editingTopicId, setEditingTopicId] = useState<string | null>(null);
  const [topicForm, setTopicForm] = useState<TopicFormState>(defaultTopicForm);
  const [savingTopic, setSavingTopic] = useState(false);
  const [topicAgentConversationId, setTopicAgentConversationId] = useState<string | null>(null);
  const [topicAgentMessages, setTopicAgentMessages] = useState<AgentMessage[]>([]);
  const [topicAgentDraft, setTopicAgentDraft] = useState("");
  const [topicAgentError, setTopicAgentError] = useState("");
  const [sendingTopicAgentMessage, setSendingTopicAgentMessage] = useState(false);
  const [scriptPreparation, setScriptPreparation] =
    useState<PrepareScriptFromTopicResponse | null>(null);
  const [scriptPrepareOptions, setScriptPrepareOptions] = useState<{
    style: ScriptStyle;
    scene_count: number;
  }>({ style: "knowledge", scene_count: 6 });
  const [preparingScript, setPreparingScript] = useState(false);
  const [generatingTopicScript, setGeneratingTopicScript] = useState(false);
  const [topicScriptError, setTopicScriptError] = useState("");
  const [agentConversationId, setAgentConversationId] = useState<string | null>(null);
  const [agentMessages, setAgentMessages] = useState<AgentMessage[]>([]);
  const [agentDraft, setAgentDraft] = useState("");
  const [agentError, setAgentError] = useState("");
  const [sendingAgentMessage, setSendingAgentMessage] = useState(false);
  const [textModelOptions, setTextModelOptions] = useState<ModelOption[]>([]);
  const [imageModelOptions, setImageModelOptions] = useState<ModelOption[]>([]);
  const [loadingTextModels, setLoadingTextModels] = useState(true);
  const [loadingImageModels, setLoadingImageModels] = useState(true);
  const [textModelError, setTextModelError] = useState("");
  const [imageModelError, setImageModelError] = useState("");
  const [strategyModelId, setStrategyModelId] = useState("");
  const [topicModelId, setTopicModelId] = useState("");
  const [reviewModelId, setReviewModelId] = useState("");
  const [scriptGenerateModelId, setScriptGenerateModelId] = useState("");
  const [scriptAgentModelId, setScriptAgentModelId] = useState("");
  const [imageModelId, setImageModelId] = useState("");
  const selectedScriptIdRef = useRef<string | null>(null);
  const selectedProjectIdRef = useRef("");
  const preserveAgentConversationRef = useRef<string | null>(null);
  const sceneRegenerationInFlightRef = useRef(false);
  const sceneRegenerationIdempotencyKeysRef = useRef(new Map<string, string>());
  const assetTaskDismissalInFlightRef = useRef(false);

  const selectedProject = projects.find((project) => project.project_id === selectedProjectId);
  const savedAccountStrategyForm = useMemo(
    () => projectToAccountStrategyForm(selectedProject),
    [selectedProject],
  );
  const hasAccountStrategyUnsavedChanges =
    !accountStrategyFormsEqual(accountStrategyForm, savedAccountStrategyForm) ||
    accountStrategyDraftNotes.length > 0 ||
    accountStrategyDraftSummary.length > 0;
  const poolActiveTopicBatchId =
    topicBatchViewMode === "all"
      ? null
      : topicBatchViewMode === "batch"
        ? topicBatchFilter
        : topicBatches[0]?.batch_id || null;
  const firstHistoryRootBatchId =
    topicGroups[0]?.root_batch_id || topicBatchRootEntries(topicBatches)[0]?.batch_id || topicBatches[0]?.batch_id || null;
  const selectedHistoryBatch =
    topicBatches.find((batch) => batch.batch_id === historyTopicBatchId) ||
    (firstHistoryRootBatchId
      ? topicBatches.find((batch) => batch.batch_id === firstHistoryRootBatchId)
      : null) ||
    topicBatchRootEntries(topicBatches)[0] ||
    topicBatches[0] ||
    null;
  const historyActiveTopicBatchId = selectedHistoryBatch?.batch_id || null;
  const activeTopicBatchId =
    contentStrategyView === "history" ? historyActiveTopicBatchId : poolActiveTopicBatchId;
  const activeTopicReviewRootBatchId =
    selectedMenuKey === contentStrategyMenuKey &&
    !(contentStrategyView === "pool" && topicBatchViewMode === "all")
      ? topicBatchRootId(activeTopicBatchId, topicBatches)
      : null;
  const selectedTopic = topics.find((topic) => topic.topic_id === selectedTopicId) || null;
  const selectedMaterial =
    materials.find((material) => material.material_id === selectedMaterialId) || null;
  const currentAssetPayload = useMemo(
    () => assetGenerationPayload(imageModelId, assetCandidateCount, useReferenceMaterials),
    [assetCandidateCount, imageModelId, useReferenceMaterials],
  );
  const strategyModelUnavailable = modelSelectionUnavailable(strategyModelId, textModelOptions);
  const topicModelUnavailable = modelSelectionUnavailable(topicModelId, textModelOptions);
  const reviewModelUnavailable = modelSelectionUnavailable(reviewModelId, textModelOptions);
  const scriptGenerateModelUnavailable = modelSelectionUnavailable(scriptGenerateModelId, textModelOptions);
  const scriptAgentModelUnavailable = modelSelectionUnavailable(scriptAgentModelId, textModelOptions);
  const imageModelUnavailable = modelSelectionUnavailable(imageModelId, imageModelOptions);
  const writesDisabled = apiAvailable === false;
  const selectedSubMenuKey =
    selectedMenuKey === contentStrategyMenuKey
      ? contentStrategyView === "account"
        ? accountStrategyMenuKey
        : contentStrategyView === "history"
        ? topicHistoryMenuKey
        : topicGeneratorMenuKey
      : selectedMenuKey === scriptCreationMenuKey
        ? selectedScriptSubMenuKey
        : selectedMenuKey === materialManagementMenuKey
          ? selectedMaterialSubMenuKey
        : null;

  const refreshModelOptions = useCallback(async () => {
    async function refreshTextModels() {
      setLoadingTextModels(true);
      setTextModelError("");
      try {
        const response = await listModelOptions(client, "text");
        setTextModelOptions(response.models);
        setStrategyModelId((current) => reconcileModelSelection(current, response.models));
        setTopicModelId((current) => reconcileModelSelection(current, response.models));
        setReviewModelId((current) => reconcileModelSelection(current, response.models));
        setScriptGenerateModelId((current) => reconcileModelSelection(current, response.models));
        setScriptAgentModelId((current) => reconcileModelSelection(current, response.models));
      } catch (error) {
        setTextModelError(errorToMessage(error));
      } finally {
        setLoadingTextModels(false);
      }
    }

    async function refreshImageModels() {
      setLoadingImageModels(true);
      setImageModelError("");
      try {
        const response = await listModelOptions(client, "image");
        setImageModelOptions(response.models);
        setImageModelId((current) => reconcileModelSelection(current, response.models));
      } catch (error) {
        setImageModelError(errorToMessage(error));
      } finally {
        setLoadingImageModels(false);
      }
    }

    await Promise.all([refreshTextModels(), refreshImageModels()]);
  }, [client]);

  useEffect(() => {
    void refreshModelOptions();
  }, [refreshModelOptions]);

  useEffect(() => {
    let active = true;

    async function loadWorkspaceMenus() {
      setLoadingMenus(true);
      setMenuError("");

      try {
        const response = await listWorkspaceMenus(client);
        if (!active) {
          return;
        }
        setWorkspaceMenus(response.menus);
        if (!response.menus.some((menu) => menu.menu_key === defaultMenuKey && menu.is_enabled)) {
          const firstEnabled = response.menus.find((menu) => menu.is_enabled);
          setSelectedMenuKey(firstEnabled?.menu_key || defaultMenuKey);
        }
      } catch (error) {
        if (active) {
          setWorkspaceMenus([]);
          setMenuError(errorToMessage(error));
        }
      } finally {
        if (active) {
          setLoadingMenus(false);
        }
      }
    }

    loadWorkspaceMenus();

    return () => {
      active = false;
    };
  }, [client]);

  useEffect(() => {
    let active = true;

    async function loadInitialData() {
      setLoadingProjects(true);
      setProjectError("");
      const healthy = await checkHealth(client);
      if (!active) {
        return;
      }
      setApiAvailable(healthy);

      try {
        const response = await listProjects(client);
        if (!active) {
          return;
        }
        setProjects(response.projects);
        setSelectedProjectId(response.projects[0]?.project_id || "");
      } catch (error) {
        if (!active) {
          return;
        }
        setProjectError(errorToMessage(error));
        setApiAvailable(false);
      } finally {
        if (active) {
          setLoadingProjects(false);
        }
      }
    }

    loadInitialData();

    return () => {
      active = false;
    };
  }, [client]);

  useEffect(() => {
    if (!selectedProjectId) {
      setScripts([]);
      setSelectedScript(null);
      selectedScriptIdRef.current = null;
      setSelectedScriptId(null);
      return;
    }

    let active = true;

    async function loadProjectScripts() {
      setLoadingScripts(true);
      setScriptError("");
      setSelectedScript(null);
      selectedScriptIdRef.current = null;
      setSelectedScriptId(null);

      try {
        const response = await listScripts(client, selectedProjectId, { status: statusFilter });
        if (!active) {
          return;
        }
        setScripts(response.scripts);

        if (response.scripts[0]) {
          if (active) {
            selectedScriptIdRef.current = response.scripts[0].script_id;
            setSelectedScriptId(response.scripts[0].script_id);
          }
          await openScript(
            client,
            response.scripts[0].script_id,
            active,
            setLoadingScriptDetail,
            setSelectedScript,
            setScriptError,
            () => selectedScriptIdRef.current === response.scripts[0].script_id,
          );
        }
      } catch (error) {
        if (active) {
          setScriptError(errorToMessage(error));
        }
      } finally {
        if (active) {
          setLoadingScripts(false);
        }
      }
    }

    loadProjectScripts();

    return () => {
      active = false;
    };
  }, [client, selectedProjectId, statusFilter]);

  useEffect(() => {
    if (!selectedProjectId || selectedMenuKey !== contentStrategyMenuKey) {
      if (!selectedProjectId) {
        setTopicBatches([]);
        setTopicBatchesLoaded(false);
      }
      return;
    }

    let active = true;

    async function loadTopicBatches() {
      setTopicBatchesLoaded(false);
      setTopicBatchError("");

      try {
        const response = await listTopicGenerationBatches(client, selectedProjectId);
        if (!active) {
          return;
        }
        setTopicBatches(visibleTopicGenerationBatches(response.batches));
      } catch (error) {
        if (active) {
          setTopicBatches([]);
          setTopicBatchError(errorToMessage(error));
        }
      } finally {
        if (active) {
          setTopicBatchesLoaded(true);
        }
      }
    }

    loadTopicBatches();

    return () => {
      active = false;
    };
  }, [client, selectedProjectId, selectedMenuKey]);

  useEffect(() => {
    if (!selectedProjectId || selectedMenuKey !== contentStrategyMenuKey) {
      if (!selectedProjectId) {
        setTopicGroups([]);
        setTopicGroupsLoaded(false);
      }
      return;
    }

    let active = true;

    async function loadTopicGroups() {
      setTopicGroupsLoaded(false);
      setTopicGroupError("");

      try {
        const response = await listTopicGroups(client, selectedProjectId, { sort: topicGroupSort });
        if (!active) {
          return;
        }
        setTopicGroups(response.topic_groups);
      } catch (error) {
        if (active) {
          setTopicGroups([]);
          setTopicGroupError(errorToMessage(error));
        }
      } finally {
        if (active) {
          setTopicGroupsLoaded(true);
        }
      }
    }

    loadTopicGroups();

    return () => {
      active = false;
    };
  }, [client, selectedProjectId, selectedMenuKey, topicGroupSort]);

  useEffect(() => {
    if (
      !selectedProjectId ||
      selectedMenuKey !== contentStrategyMenuKey ||
      contentStrategyView === "account"
    ) {
      if (!selectedProjectId) {
        setTopics([]);
        setTopicStats(emptyTopicStats);
        setSelectedTopicId(null);
      }
      return;
    }
    if (!topicBatchesLoaded) {
      return;
    }
    if (contentStrategyView === "history" && !activeTopicBatchId) {
      setTopics([]);
      setTopicStats(emptyTopicStats);
      setSelectedTopicId(null);
      return;
    }

    let active = true;

    async function loadProjectTopics() {
      setLoadingTopics(true);
      setTopicError("");

      try {
        const filters = topicListFilters(topicStatusFilter, topicSourceFilter, activeTopicBatchId);
        const response = await listContentTopicsForBatchGroup(
          client,
          selectedProjectId,
          filters,
          topicBatchGroupIds(activeTopicBatchId, topicBatches),
        );
        if (!active) {
          return;
        }
        const sortedTopics = sortContentTopicsByScore(response.topics);
        setTopics(sortedTopics);
        setTopicStats(response.stats);
        setSelectedTopicId((currentTopicId) => {
          if (sortedTopics.some((topic) => topic.topic_id === currentTopicId)) {
            return currentTopicId;
          }
          return sortedTopics[0]?.topic_id || null;
        });
      } catch (error) {
        if (active) {
          setTopicError(errorToMessage(error));
        }
      } finally {
        if (active) {
          setLoadingTopics(false);
        }
      }
    }

    loadProjectTopics();

    return () => {
      active = false;
    };
  }, [
    activeTopicBatchId,
    client,
    contentStrategyView,
    selectedProjectId,
    selectedMenuKey,
    topicBatches,
    topicBatchesLoaded,
    topicSourceFilter,
    topicStatusFilter,
  ]);

  useEffect(() => {
    if (
      !selectedProjectId ||
      selectedMenuKey !== contentStrategyMenuKey ||
      contentStrategyView === "account" ||
      !topicBatchesLoaded ||
      !activeTopicReviewRootBatchId
    ) {
      setTopicReviewSnapshot(null);
      setTopicReviewLoading(false);
      setTopicReviewError("");
      return;
    }

    let active = true;
    const reviewRootBatchId = activeTopicReviewRootBatchId;

    async function loadLatestTopicReview() {
      setTopicReviewLoading(true);
      setTopicReviewError("");

      try {
        const snapshot = await getLatestTopicGroupReview(client, reviewRootBatchId);
        if (active) {
          setTopicReviewSnapshot(snapshot);
        }
      } catch (error) {
        if (active) {
          setTopicReviewSnapshot(null);
          setTopicReviewError(errorToMessage(error));
        }
      } finally {
        if (active) {
          setTopicReviewLoading(false);
        }
      }
    }

    loadLatestTopicReview();

    return () => {
      active = false;
    };
  }, [
    activeTopicReviewRootBatchId,
    client,
    contentStrategyView,
    selectedMenuKey,
    selectedProjectId,
    topicBatchesLoaded,
  ]);

  useEffect(() => {
    if (
      !selectedProjectId ||
      selectedMenuKey !== contentStrategyMenuKey ||
      contentStrategyView === "account" ||
      !topicBatchesLoaded ||
      !activeTopicBatchId
    ) {
      setTopicQualityEvaluation(null);
      setTopicQualityLoading(false);
      setTopicQualityError("");
      return;
    }

    let active = true;
    const batchId = activeTopicBatchId;

    async function loadLatestTopicQualityEvaluation() {
      setTopicQualityLoading(true);
      setTopicQualityError("");

      try {
        const evaluation = await getLatestTopicQualityEvaluation(client, batchId, selectedProjectId);
        if (active) {
          setTopicQualityEvaluation(evaluation);
        }
      } catch (error) {
        if (active) {
          setTopicQualityEvaluation(null);
          setTopicQualityError(errorToMessage(error));
        }
      } finally {
        if (active) {
          setTopicQualityLoading(false);
        }
      }
    }

    loadLatestTopicQualityEvaluation();

    return () => {
      active = false;
    };
  }, [
    activeTopicBatchId,
    client,
    contentStrategyView,
    selectedMenuKey,
    selectedProjectId,
    topicBatchesLoaded,
  ]);

  useEffect(() => {
    if (!selectedProjectId || selectedMenuKey !== materialManagementMenuKey) {
      if (!selectedProjectId) {
        setMaterials([]);
        setSelectedMaterialId(null);
        setMaterialForm(defaultMaterialForm);
      }
      return;
    }

    let active = true;

    async function loadProjectMaterials() {
      setLoadingMaterials(true);
      setMaterialError("");

      try {
        const response = await listMaterials(client, selectedProjectId, materialFilters);
        if (!active) {
          return;
        }
        setMaterials(response.materials);
        let nextSelected =
          response.materials.find((material) => material.material_id === selectedMaterialId) ||
          response.materials[0] ||
          null;
        setSelectedMaterialId((currentMaterialId) => {
          const currentMaterial = response.materials.find(
            (material) => material.material_id === currentMaterialId,
          );
          if (currentMaterial) {
            nextSelected = currentMaterial;
            return currentMaterialId;
          }
          return response.materials[0]?.material_id || null;
        });
        setMaterialForm(nextSelected ? materialToForm(nextSelected) : defaultMaterialForm);
      } catch (error) {
        if (active) {
          setMaterialError(errorToMessage(error));
        }
      } finally {
        if (active) {
          setLoadingMaterials(false);
        }
      }
    }

    loadProjectMaterials();

    return () => {
      active = false;
    };
  }, [client, materialFilters, selectedMenuKey, selectedProjectId]);

  useEffect(() => {
    if (
      !selectedScript ||
      selectedMenuKey !== materialManagementMenuKey ||
      selectedMaterialSubMenuKey !== assetGenerationMenuKey
    ) {
      setAssetCandidates([]);
      setAssetTasks([]);
      setAssetPlan(null);
      setSelectedAssetSceneId(null);
      setLoadingAssetCandidates(false);
      setLoadingAssetPlan(false);
      setAssetError("");
      setAssetTaskToDismissId(null);
      setDismissingAssetTaskId(null);
      assetTaskDismissalInFlightRef.current = false;
      return;
    }

    let active = true;
    const script = selectedScript;
    const firstSceneId =
      [...script.scenes].sort((left, right) => left.sequence - right.sequence)[0]?.scene_id || null;

    setSelectedAssetSceneId((currentSceneId) =>
      script.scenes.some((scene) => scene.scene_id === currentSceneId)
        ? currentSceneId
        : firstSceneId,
    );
    setAssetTasks([]);
    setAssetTaskToDismissId(null);
    setDismissingAssetTaskId(null);
    assetTaskDismissalInFlightRef.current = false;

    async function loadAssetState() {
      setLoadingAssetCandidates(true);
      setAssetError("");

      try {
        const [candidateResponse, taskResponse] = await Promise.all([
          listAssetCandidates(client, script.script_id),
          listAssetGenerationTasks(client, script.script_id),
        ]);
        if (active) {
          setAssetCandidates(candidateResponse.candidates);
          setAssetTasks(taskResponse.tasks);
        }
      } catch (error) {
        if (active) {
          setAssetCandidates([]);
          setAssetTasks([]);
          setAssetError(errorToMessage(error));
        }
      } finally {
        if (active) {
          setLoadingAssetCandidates(false);
        }
      }
    }

    loadAssetState();

    return () => {
      active = false;
    };
  }, [client, selectedMaterialSubMenuKey, selectedMenuKey, selectedScript]);

  useEffect(() => {
    const scriptId = selectedScript?.script_id;
    const hasInFlightImageTask = assetTasks.some(
      (task) =>
        task.task_type === "image_candidates" &&
        (task.status === "pending" || task.status === "processing"),
    );
    if (
      !scriptId ||
      selectedMenuKey !== materialManagementMenuKey ||
      selectedMaterialSubMenuKey !== assetGenerationMenuKey ||
      !hasInFlightImageTask
    ) {
      return;
    }
    const activeScriptId = scriptId;

    let active = true;
    let polling = false;
    async function pollAssetState() {
      if (polling) {
        return;
      }
      polling = true;
      try {
        const [candidateResponse, taskResponse] = await Promise.all([
          listAssetCandidates(client, activeScriptId),
          listAssetGenerationTasks(client, activeScriptId),
        ]);
        if (active && selectedScriptIdRef.current === activeScriptId) {
          setAssetCandidates(candidateResponse.candidates);
          setAssetTasks(taskResponse.tasks);
        }
      } catch (error) {
        if (active && selectedScriptIdRef.current === activeScriptId) {
          setAssetError(errorToMessage(error));
        }
      } finally {
        polling = false;
      }
    }

    const intervalId = window.setInterval(pollAssetState, 3000);
    return () => {
      active = false;
      window.clearInterval(intervalId);
    };
  }, [assetTasks, client, selectedMaterialSubMenuKey, selectedMenuKey, selectedScript]);

  useEffect(() => {
    if (
      !selectedScript ||
      selectedMenuKey !== materialManagementMenuKey ||
      selectedMaterialSubMenuKey !== assetGenerationMenuKey ||
      imageModelUnavailable
    ) {
      setAssetPlan(null);
      setLoadingAssetPlan(false);
      if (selectedScript && selectedMenuKey === materialManagementMenuKey && selectedMaterialSubMenuKey === assetGenerationMenuKey) {
        setAssetError("请选择可用的图片模型");
      }
      return;
    }

    let active = true;
    const script = selectedScript;

    async function loadAssetPlan() {
      setLoadingAssetPlan(true);
      setAssetError("");

      try {
        const plan = await getAssetGenerationPlan(client, script.script_id, currentAssetPayload);
        if (active) {
          setAssetPlan(plan);
        }
      } catch (error) {
        if (active) {
          setAssetPlan(null);
          setAssetError(errorToMessage(error));
          if (isModelDisabledError(error)) {
            void refreshModelOptions();
          }
        }
      } finally {
        if (active) {
          setLoadingAssetPlan(false);
        }
      }
    }

    loadAssetPlan();

    return () => {
      active = false;
    };
  }, [client, currentAssetPayload, imageModelUnavailable, refreshModelOptions, selectedMaterialSubMenuKey, selectedMenuKey, selectedScript]);

  useEffect(() => {
    selectedProjectIdRef.current = selectedProjectId;
    setAgentConversationId(null);
    setAgentMessages([]);
    setAgentDraft("");
    setAgentError("");
    setSendingAgentMessage(false);
    setTopicAgentConversationId(null);
    setTopicAgentMessages([]);
    setTopicAgentDraft("");
    setTopicAgentError("");
    setSendingTopicAgentMessage(false);
    setShowTopicForm(false);
    setEditingTopicId(null);
    setScriptPreparation(null);
    setTopicScriptError("");
    setTopicBatchFilter(null);
    setTopicBatchViewMode("latest");
    setContentStrategyView("pool");
    setHistoryTopicBatchId(null);
    setTopicBatches([]);
    setTopicBatchesLoaded(false);
    setTopicBatchError("");
    setTopicGroups([]);
    setTopicGroupsLoaded(false);
    setTopicGroupError("");
    setTopicGroupSort("script_priority");
    setDeletingTopicId(null);
    setTopicReviewSnapshot(null);
    setTopicReviewLoading(false);
    setTopicReviewError("");
    setTopicQualityEvaluation(null);
    setTopicQualityLoading(false);
    setTopicQualityError("");
    setAssetCandidates([]);
    setAssetTasks([]);
    setAssetPlan(null);
    setAssetCandidateCount(3);
    setUseReferenceMaterials(true);
    setSelectedAssetSceneId(null);
    setLoadingAssetPlan(false);
    setLoadingAssetCandidates(false);
    setAssetActionInProgress(false);
    setAssetError("");
    setMaterials([]);
    setSelectedMaterialId(null);
    setMaterialError("");
    setMaterialActionError("");
    setSavingMaterial(false);
    setCreatingMaterial(false);
    setMaterialFilters({ material_type: "all", status: "active", q: "", tag: "" });
    setMaterialForm(defaultMaterialForm);
  }, [selectedProjectId]);

  useEffect(() => {
    setAccountStrategyForm(projectToAccountStrategyForm(selectedProject));
    setAccountStrategyDraftNotes("");
    setAccountStrategyDraftSummary("");
    setAccountStrategyError("");
    setGeneratingAccountStrategyDraft(false);
    setSavingAccountStrategy(false);
  }, [selectedProjectId]);

  useEffect(() => {
    selectedScriptIdRef.current = selectedScriptId;
    if (preserveAgentConversationRef.current) {
      setAgentConversationId(preserveAgentConversationRef.current);
      preserveAgentConversationRef.current = null;
      return;
    }
    setAgentConversationId(null);
    setAgentMessages([]);
    setAgentDraft("");
    setAgentError("");
    setSendingAgentMessage(false);
  }, [selectedScriptId]);

  async function handleOpenScript(scriptId: string) {
    selectedScriptIdRef.current = scriptId;
    setSelectedScriptId(scriptId);
    setSelectedScript(null);
    await openScript(
      client,
      scriptId,
      true,
      setLoadingScriptDetail,
      setSelectedScript,
      setScriptError,
      () => selectedScriptIdRef.current === scriptId,
    );
  }

  function handleSelectWorkspaceMenu(menuKey: string) {
    setSelectedMenuKey(menuKey);
    if (menuKey === contentStrategyMenuKey) {
      setContentStrategyView("pool");
    }
    if (menuKey === scriptCreationMenuKey) {
      setSelectedScriptSubMenuKey(scriptGeneratorMenuKey);
    }
  }

  function handleSelectWorkspaceSubMenu(menuKey: string) {
    if (menuKey === accountStrategyMenuKey) {
      setSelectedMenuKey(contentStrategyMenuKey);
      setContentStrategyView("account");
      return;
    }
    if (menuKey === topicHistoryMenuKey) {
      setSelectedMenuKey(contentStrategyMenuKey);
      setContentStrategyView("history");
      return;
    }
    if (menuKey === topicGeneratorMenuKey) {
      setSelectedMenuKey(contentStrategyMenuKey);
      setContentStrategyView("pool");
      return;
    }
    if (menuKey === scriptGeneratorMenuKey) {
      setSelectedMenuKey(scriptCreationMenuKey);
      setSelectedScriptSubMenuKey(scriptGeneratorMenuKey);
      return;
    }
    if (menuKey === materialLibraryMenuKey) {
      setSelectedMenuKey(materialManagementMenuKey);
      setSelectedMaterialSubMenuKey(materialLibraryMenuKey);
      return;
    }
    if (menuKey === assetGenerationMenuKey) {
      setSelectedMenuKey(materialManagementMenuKey);
      setSelectedMaterialSubMenuKey(assetGenerationMenuKey);
    }
  }

  function handleSelectHistoryTopicBatch(batchId: string) {
    setHistoryTopicBatchId(batchId);
    setTopicBatchFilter(batchId);
    setTopicBatchViewMode("batch");
  }

  async function handleSelectMaterial(materialId: string) {
    setSelectedMaterialId(materialId);
    setCreatingMaterial(false);
    setMaterialActionError("");
    const localMaterial = materials.find((material) => material.material_id === materialId);
    if (localMaterial) {
      setMaterialForm(materialToForm(localMaterial));
      return;
    }

    try {
      const material = await getMaterial(client, materialId);
      setMaterials((currentMaterials) => {
        const withoutMaterial = currentMaterials.filter(
          (currentMaterial) => currentMaterial.material_id !== material.material_id,
        );
        return [material, ...withoutMaterial];
      });
      setMaterialForm(materialToForm(material));
    } catch (error) {
      setMaterialActionError(errorToMessage(error));
    }
  }

  function handleNewMaterial() {
    setSelectedMaterialId(null);
    setCreatingMaterial(true);
    setMaterialActionError("");
    setMaterialForm(defaultMaterialForm);
  }

  async function handleSaveMaterial() {
    if (!selectedProjectId) {
      setMaterialActionError("请先选择账号");
      return;
    }

    const payload = materialPayloadFromForm(materialForm);
    if (!payload.file_name) {
      setMaterialActionError("素材名称不能为空");
      return;
    }
    if (!payload.file_url) {
      setMaterialActionError("素材 URL 不能为空");
      return;
    }

    setSavingMaterial(true);
    setMaterialActionError("");

    try {
      const savedMaterial = selectedMaterialId
        ? await updateMaterial(client, selectedMaterialId, payload)
        : await createMaterial(client, selectedProjectId, payload);
      setMaterials((currentMaterials) => {
        const withoutSaved = currentMaterials.filter(
          (material) => material.material_id !== savedMaterial.material_id,
        );
        return [savedMaterial, ...withoutSaved];
      });
      setSelectedMaterialId(savedMaterial.material_id);
      setCreatingMaterial(false);
      setMaterialForm(materialToForm(savedMaterial));
    } catch (error) {
      setMaterialActionError(errorToMessage(error));
    } finally {
      setSavingMaterial(false);
    }
  }

  async function handleUpdateMaterialStatus(status: MaterialStatus) {
    if (!selectedMaterial || selectedMaterial.status === status) {
      return;
    }

    setSavingMaterial(true);
    setMaterialActionError("");

    try {
      const updatedMaterial = await updateMaterialStatus(
        client,
        selectedMaterial.material_id,
        status,
      );
      const shouldRemainVisible =
        materialFilters.status === "all" || materialFilters.status === updatedMaterial.status;
      setMaterials((currentMaterials) => {
        const withoutUpdated = currentMaterials.filter(
          (material) => material.material_id !== updatedMaterial.material_id,
        );
        return shouldRemainVisible ? [updatedMaterial, ...withoutUpdated] : withoutUpdated;
      });
      if (shouldRemainVisible) {
        setSelectedMaterialId(updatedMaterial.material_id);
        setCreatingMaterial(false);
        setMaterialForm(materialToForm(updatedMaterial));
      } else {
        setSelectedMaterialId(null);
        setCreatingMaterial(false);
        setMaterialForm(defaultMaterialForm);
      }
    } catch (error) {
      setMaterialActionError(errorToMessage(error));
    } finally {
      setSavingMaterial(false);
    }
  }

  function handleNewScript() {
    selectedScriptIdRef.current = null;
    preserveAgentConversationRef.current = null;
    setSelectedScriptId(null);
    setSelectedScript(null);
    setLoadingScriptDetail(false);
    setStatusError("");
    setAgentConversationId(null);
    setAgentMessages([]);
    setAgentDraft("");
    setAgentError("");
    setSendingAgentMessage(false);
  }

  async function handleUpdateStatus(status: ScriptStatus) {
    if (!selectedScript || selectedScript.status === status) {
      return;
    }

    setStatusError("");
    setUpdatingStatus(true);

    try {
      const response = await updateScriptStatus(client, selectedScript.script_id, status);
      setSelectedScript({
        ...selectedScript,
        status: response.status,
        updated_at: response.updated_at,
      });
      setScripts((currentScripts) =>
        currentScripts.map((script) =>
          script.script_id === response.script_id ? { ...script, status: response.status } : script,
        ),
      );
    } catch (error) {
      setStatusError(errorToMessage(error));
    } finally {
      setUpdatingStatus(false);
    }
  }

  async function refreshAssetCandidates(scriptId: string) {
    setLoadingAssetCandidates(true);

    try {
      const response = await listAssetCandidates(client, scriptId);
      if (selectedScriptIdRef.current === scriptId) {
        setAssetCandidates(response.candidates);
      }
    } catch (error) {
      if (selectedScriptIdRef.current === scriptId) {
        setAssetError(errorToMessage(error));
      }
    } finally {
      if (selectedScriptIdRef.current === scriptId) {
        setLoadingAssetCandidates(false);
      }
    }
  }

  async function handleCreateAssetGenerationTasks() {
    if (!selectedScript || imageModelUnavailable) {
      setAssetError("请选择可用的图片模型");
      return;
    }

    const scriptId = selectedScript.script_id;
    setAssetActionInProgress(true);
    setAssetError("");

    try {
      const response = await createAssetGenerationTasks(client, scriptId, currentAssetPayload);
      if (selectedScriptIdRef.current === scriptId) {
        setAssetTasks(response.tasks);
      }
      await refreshAssetCandidates(scriptId);
    } catch (error) {
      if (isModelDisabledError(error)) {
        await refreshModelOptions();
      }
      if (selectedScriptIdRef.current === scriptId) {
        setAssetError(errorToMessage(error));
      }
    } finally {
      if (selectedScriptIdRef.current === scriptId) {
        setAssetActionInProgress(false);
      }
    }
  }

  async function handleSelectAssetCandidate(sceneId: string, candidateId: string) {
    setAssetActionInProgress(true);
    setAssetError("");

    try {
      const updatedCandidate = await selectAssetCandidate(client, sceneId, candidateId);
      setAssetCandidates((currentCandidates) =>
        mergeUpdatedCandidate(currentCandidates, updatedCandidate),
      );
    } catch (error) {
      setAssetError(errorToMessage(error));
    } finally {
      setAssetActionInProgress(false);
    }
  }

  async function handleRejectAssetCandidate(sceneId: string, candidateId: string) {
    setAssetActionInProgress(true);
    setAssetError("");

    try {
      const updatedCandidate = await rejectAssetCandidate(client, sceneId, candidateId);
      setAssetCandidates((currentCandidates) =>
        mergeUpdatedCandidate(currentCandidates, updatedCandidate),
      );
    } catch (error) {
      setAssetError(errorToMessage(error));
    } finally {
      setAssetActionInProgress(false);
    }
  }

  async function handleRegenerateSceneAsset(sceneId: string) {
    if (sceneRegenerationInFlightRef.current || imageModelUnavailable) {
      setAssetError("请选择可用的图片模型");
      return;
    }
    sceneRegenerationInFlightRef.current = true;
    setAssetActionInProgress(true);
    setAssetError("");
    const idempotencyKey =
      sceneRegenerationIdempotencyKeysRef.current.get(sceneId) || createIdempotencyKey();
    sceneRegenerationIdempotencyKeysRef.current.set(sceneId, idempotencyKey);

    try {
      const task = await createSceneAssetGenerationTask(
        client,
        sceneId,
        currentAssetPayload,
        idempotencyKey,
      );
      sceneRegenerationIdempotencyKeysRef.current.delete(sceneId);
      setAssetTasks((currentTasks) => upsertAssetTask(currentTasks, task));
    } catch (error) {
      if (isModelDisabledError(error)) {
        await refreshModelOptions();
      }
      setAssetError(errorToMessage(error));
    } finally {
      sceneRegenerationInFlightRef.current = false;
      setAssetActionInProgress(false);
    }
  }

  async function handleConfirmAssetGenerationTask(taskId: string) {
    setAssetActionInProgress(true);
    setAssetError("");

    try {
      const task = await confirmAssetGenerationTask(client, taskId);
      setAssetTasks((currentTasks) => upsertAssetTask(currentTasks, task));
    } catch (error) {
      setAssetError(errorToMessage(error));
    } finally {
      setAssetActionInProgress(false);
    }
  }

  async function handleDismissAssetGenerationTask() {
    if (!selectedScript || !assetTaskToDismissId || assetTaskDismissalInFlightRef.current) {
      return;
    }

    const scriptId = selectedScript.script_id;
    const taskId = assetTaskToDismissId;
    assetTaskDismissalInFlightRef.current = true;
    setDismissingAssetTaskId(taskId);
    setAssetError("");

    try {
      await dismissAssetGenerationTask(client, taskId);
      const [candidateResponse, taskResponse] = await Promise.all([
        listAssetCandidates(client, scriptId),
        listAssetGenerationTasks(client, scriptId),
      ]);
      if (selectedScriptIdRef.current === scriptId) {
        setAssetCandidates(candidateResponse.candidates);
        setAssetTasks(taskResponse.tasks);
        setAssetTaskToDismissId(null);
      }
    } catch (error) {
      if (selectedScriptIdRef.current === scriptId) {
        setAssetError(errorToMessage(error));
      }
    } finally {
      assetTaskDismissalInFlightRef.current = false;
      if (selectedScriptIdRef.current === scriptId) {
        setDismissingAssetTaskId(null);
      }
    }
  }

  async function refreshContentTopics(
    batchId: string | null = activeTopicBatchId,
    batches: TopicGenerationBatchSummary[] = topicBatches,
  ) {
    if (!selectedProjectId) {
      return;
    }
    const filters = topicListFilters(topicStatusFilter, topicSourceFilter, batchId);
    const response = await listContentTopicsForBatchGroup(
      client,
      selectedProjectId,
      filters,
      topicBatchGroupIds(batchId, batches),
    );
    const sortedTopics = sortContentTopicsByScore(response.topics);
    setTopics(sortedTopics);
    setTopicStats(response.stats);
    setSelectedTopicId((currentTopicId) => {
      if (sortedTopics.some((topic) => topic.topic_id === currentTopicId)) {
        return currentTopicId;
      }
      return sortedTopics[0]?.topic_id || null;
    });
  }

  async function refreshTopicBatches() {
    if (!selectedProjectId) {
      return [];
    }
    const response = await listTopicGenerationBatches(client, selectedProjectId);
    const visibleBatches = visibleTopicGenerationBatches(response.batches);
    setTopicBatches(visibleBatches);
    setTopicBatchesLoaded(true);
    setTopicBatchError("");
    return visibleBatches;
  }

  async function refreshTopicGroups(sort: TopicGroupSort = topicGroupSort) {
    if (!selectedProjectId) {
      return [];
    }
    const response = await listTopicGroups(client, selectedProjectId, { sort });
    setTopicGroups(response.topic_groups);
    setTopicGroupsLoaded(true);
    setTopicGroupError("");
    return response.topic_groups;
  }

  async function refreshTopicGroupsSafely(sort: TopicGroupSort = topicGroupSort) {
    try {
      return await refreshTopicGroups(sort);
    } catch (error) {
      setTopicGroupError(errorToMessage(error));
      setTopicGroupsLoaded(true);
      return [];
    }
  }

  async function refreshProjectScripts() {
    if (!selectedProjectId) {
      return;
    }
    const response = await listScripts(client, selectedProjectId, { status: "all" });
    setScripts(response.scripts);
  }

  function handleNewTopic() {
    setTopicActionError("");
    setShowTopicForm(true);
    setEditingTopicId(null);
    setTopicForm(defaultTopicForm);
  }

  function handleEditTopic(topic: ContentTopic) {
    setTopicActionError("");
    setShowTopicForm(true);
    setEditingTopicId(topic.topic_id);
    setTopicForm(topicToForm(topic));
  }

  function handleCancelTopicForm() {
    setShowTopicForm(false);
    setEditingTopicId(null);
    setTopicForm(defaultTopicForm);
  }

  async function handleSubmitTopic(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!selectedProjectId) {
      setTopicActionError("请先选择项目");
      return;
    }

    const payload = topicPayloadFromForm(topicForm);
    if (!payload.title.trim()) {
      setTopicActionError("选题标题不能为空");
      return;
    }

    setSavingTopic(true);
    setTopicActionError("");

    try {
      const savedTopic = editingTopicId
        ? await updateContentTopic(client, editingTopicId, payload)
        : await createContentTopic(client, selectedProjectId, payload);
      setTopics((currentTopics) => {
        const withoutSaved = currentTopics.filter((topic) => topic.topic_id !== savedTopic.topic_id);
        return sortContentTopicsByScore([savedTopic, ...withoutSaved]);
      });
      setSelectedTopicId(savedTopic.topic_id);
      setShowTopicForm(false);
      setEditingTopicId(null);
      setTopicForm(defaultTopicForm);
      if (!editingTopicId) {
        setTopicStats((currentStats) => ({
          ...currentStats,
          total: currentStats.total + 1,
          idea: currentStats.idea + (savedTopic.status === "idea" ? 1 : 0),
        }));
      }
    } catch (error) {
      setTopicActionError(errorToMessage(error));
    } finally {
      setSavingTopic(false);
    }
  }

  function handleAccountStrategyFormChange(field: keyof AccountStrategyFormState, value: string) {
    setAccountStrategyForm((currentForm) => ({ ...currentForm, [field]: value }));
  }

  function handleCancelAccountStrategyEdit() {
    setAccountStrategyForm(projectToAccountStrategyForm(selectedProject));
    setAccountStrategyDraftNotes("");
    setAccountStrategyDraftSummary("");
    setAccountStrategyError("");
  }

  async function handleGenerateAccountStrategyDraft() {
    if (!selectedProjectId) {
      setAccountStrategyError("请先选择账号");
      return;
    }
    if (strategyModelUnavailable) {
      setAccountStrategyError("请选择可用的文本模型");
      return;
    }

    setGeneratingAccountStrategyDraft(true);
    setAccountStrategyError("");
    setAccountStrategyDraftSummary("");
    const projectIdAtSend = selectedProjectId;

    try {
      const response = await generateStrategyProfileDraft(client, selectedProjectId, {
        direction_notes: accountStrategyDraftNotes.trim(),
        model_id: strategyModelId,
      });
      if (selectedProjectIdRef.current !== projectIdAtSend) {
        return;
      }
      setAccountStrategyForm((currentForm) => ({
        ...currentForm,
        target_audience: response.draft.target_audience,
        content_pillars: response.draft.content_pillars.join("\n"),
        tone_style: response.draft.tone_style,
        forbidden_topics: response.draft.forbidden_topics.join("\n"),
        reference_accounts: response.draft.reference_accounts.join("\n"),
        topic_preferences: response.draft.topic_preferences,
      }));
      setAccountStrategyDraftSummary(response.draft_summary);
    } catch (error) {
      if (isModelDisabledError(error)) {
        await refreshModelOptions();
      }
      if (selectedProjectIdRef.current === projectIdAtSend) {
        setAccountStrategyError(errorToMessage(error));
      }
    } finally {
      if (selectedProjectIdRef.current === projectIdAtSend) {
        setGeneratingAccountStrategyDraft(false);
      }
    }
  }

  async function handleSubmitAccountStrategy(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!selectedProjectId) {
      setAccountStrategyError("请先选择账号");
      return;
    }

    const payload = accountStrategyPayloadFromForm(accountStrategyForm);
    if (!payload.name) {
      setAccountStrategyError("账号名称不能为空");
      return;
    }

    setSavingAccountStrategy(true);
    setAccountStrategyError("");

    try {
      const updatedProject = await updateProjectStrategyProfile(client, selectedProjectId, payload);
      setProjects((currentProjects) =>
        currentProjects.map((project) =>
          project.project_id === updatedProject.project_id ? updatedProject : project,
        ),
      );
      setSelectedProjectId(updatedProject.project_id);
      setAccountStrategyForm(projectToAccountStrategyForm(updatedProject));
      setAccountStrategyDraftSummary("");
    } catch (error) {
      setAccountStrategyError(errorToMessage(error));
    } finally {
      setSavingAccountStrategy(false);
    }
  }

  async function handleUpdateTopicStatus(topic: ContentTopic, status: ContentTopicStatus) {
    if (topic.status === status || status === "scripted") {
      return;
    }
    setTopicActionError("");

    try {
      const updatedTopic = await updateContentTopicStatus(client, topic.topic_id, status);
      setTopics((currentTopics) =>
        currentTopics.map((currentTopic) =>
          currentTopic.topic_id === updatedTopic.topic_id ? updatedTopic : currentTopic,
        ),
      );
      setSelectedTopicId(updatedTopic.topic_id);
      setTopicStats((currentStats) => adjustTopicStats(currentStats, topic.status, updatedTopic.status));
    } catch (error) {
      setTopicActionError(errorToMessage(error));
    }
  }

  async function handleDeleteTopic(topic: ContentTopic) {
    if (topic.status === "scripted") {
      return;
    }
    if (
      typeof window !== "undefined" &&
      !window.confirm(`确认从管理视图移除「${topic.title}」吗？`)
    ) {
      return;
    }

    setTopicActionError("");
    setDeletingTopicId(topic.topic_id);

    try {
      await deleteContentTopic(client, topic.topic_id);
      const refreshedBatches = await refreshTopicBatches();
      await refreshTopicGroupsSafely();
      await refreshContentTopics(activeTopicBatchId, refreshedBatches);
    } catch (error) {
      setTopicActionError(errorToMessage(error));
    } finally {
      setDeletingTopicId(null);
    }
  }

  async function handleSupplementTopicBatch(batchId: string, content: string) {
    if (!selectedProjectId) {
      throw new Error("请先选择项目");
    }
    if (topicModelUnavailable) {
      throw new Error("请选择可用的文本模型");
    }

    const projectIdAtSend = selectedProjectId;
    let conversationId = topicAgentConversationId;
    if (!conversationId) {
      const conversation = await createAgentConversation(client, {
        project_id: selectedProjectId,
        agent_type: "topic",
        title: "选题 Agent 对话",
      });
      if (selectedProjectIdRef.current !== projectIdAtSend) {
        return;
      }
      conversationId = conversation.conversation_id;
      setTopicAgentConversationId(conversationId);
    }

    let response: Awaited<ReturnType<typeof sendAgentMessage>>;
    try {
      response = await sendAgentMessage(client, conversationId, {
        content,
        model_id: topicModelId,
        supplement_of_batch_id: batchId,
      });
    } catch (error) {
      if (isModelDisabledError(error)) {
        await refreshModelOptions();
      }
      throw error;
    }
    if (selectedProjectIdRef.current !== projectIdAtSend) {
      return;
    }
    setTopicAgentMessages((currentMessages) => [
      ...currentMessages,
      response.user_message,
      response.assistant_message,
    ]);

    const newBatchId = getTopicAgentBatchId(response.assistant_message);
    if (!newBatchId) {
      throw new Error("补充生成未返回批次");
    }

    const refreshedBatches = await refreshTopicBatches();
    setHistoryTopicBatchId(newBatchId);
    setTopicBatchViewMode("batch");
    setTopicBatchFilter(newBatchId);
    await refreshTopicGroupsSafely();
    await refreshContentTopics(newBatchId, refreshedBatches);
  }

  async function handleReviewTopicGroup() {
    if (!activeTopicReviewRootBatchId) {
      setTopicReviewError("请先选择历史主题组");
      return;
    }
    if (reviewModelUnavailable) {
      setTopicReviewError("请选择可用的文本模型");
      return;
    }

    const rootBatchId = activeTopicReviewRootBatchId;
    setTopicReviewLoading(true);
    setTopicReviewError("");

    try {
      const createdSnapshot = await createTopicGroupReview(client, rootBatchId, {
        model_id: reviewModelId,
      });
      const latestSnapshot = await getLatestTopicGroupReview(client, rootBatchId);
      setTopicReviewSnapshot(latestSnapshot || createdSnapshot);
      await refreshTopicGroupsSafely();
    } catch (error) {
      if (isModelDisabledError(error)) {
        await refreshModelOptions();
      }
      setTopicReviewError(errorToMessage(error));
    } finally {
      setTopicReviewLoading(false);
    }
  }

  async function handleSendTopicAgentMessage(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const content = topicAgentDraft.trim();

    if (!selectedProjectId) {
      setTopicAgentError("请先选择项目");
      return;
    }

    if (!content) {
      setTopicAgentError("请输入选题生成要求");
      return;
    }
    if (topicModelUnavailable) {
      setTopicAgentError("请选择可用的文本模型");
      return;
    }

    setTopicAgentError("");
    setSendingTopicAgentMessage(true);
    const projectIdAtSend = selectedProjectId;

    try {
      let conversationId = topicAgentConversationId;
      if (!conversationId) {
        const conversation = await createAgentConversation(client, {
          project_id: selectedProjectId,
          agent_type: "topic",
          title: "选题 Agent 对话",
        });
        if (selectedProjectIdRef.current !== projectIdAtSend) {
          return;
        }
        conversationId = conversation.conversation_id;
        setTopicAgentConversationId(conversationId);
      }

      const response = await sendAgentMessage(client, conversationId, {
        content,
        model_id: topicModelId,
      });
      if (selectedProjectIdRef.current !== projectIdAtSend) {
        return;
      }
      setTopicAgentMessages((currentMessages) => [
        ...currentMessages,
        response.user_message,
        response.assistant_message,
      ]);
      setTopicAgentDraft("");
      const batchId = getTopicAgentBatchId(response.assistant_message);
      if (batchId) {
        setTopicBatchViewMode("batch");
        setTopicBatchFilter(batchId);
      }
      try {
        await refreshTopicBatches();
        await refreshTopicGroupsSafely();
      } catch (error) {
        setTopicBatchError(errorToMessage(error));
      }
      await refreshContentTopics(batchId);
    } catch (error) {
      if (isModelDisabledError(error)) {
        await refreshModelOptions();
      }
      if (selectedProjectIdRef.current === projectIdAtSend) {
        setTopicAgentError(errorToMessage(error));
      }
    } finally {
      if (selectedProjectIdRef.current === projectIdAtSend) {
        setSendingTopicAgentMessage(false);
      }
    }
  }

  async function handlePrepareScriptFromTopic(topic: ContentTopic) {
    setTopicScriptError("");
    setPreparingScript(true);
    const options = { style: "knowledge" as ScriptStyle, scene_count: 6 };
    setScriptPrepareOptions(options);

    try {
      const response = await prepareScriptFromTopic(client, topic.topic_id, options);
      setScriptPreparation(response);
    } catch (error) {
      setTopicScriptError(errorToMessage(error));
    } finally {
      setPreparingScript(false);
    }
  }

  async function handleConfirmTopicScriptGeneration() {
    if (!scriptPreparation || !selectedProjectId) {
      return;
    }
    if (scriptGenerateModelUnavailable) {
      setTopicScriptError("请选择可用的文本模型");
      return;
    }

    setGeneratingTopicScript(true);
    setTopicScriptError("");

    try {
      const script = await generateScript(client, {
        project_id: selectedProjectId,
        model_id: scriptGenerateModelId,
        topic_id: scriptPreparation.script_request.topic_id,
        style: scriptPrepareOptions.style,
        scene_count: scriptPrepareOptions.scene_count,
      });
      await refreshContentTopics();
      await refreshProjectScripts();
      setScripts((currentScripts) => upsertSummary(currentScripts, script));
      selectedScriptIdRef.current = script.script_id;
      setSelectedScriptId(script.script_id);
      setSelectedScript(script);
      setStatusFilter("all");
      setSelectedMenuKey(scriptCreationMenuKey);
      setScriptPreparation(null);
    } catch (error) {
      if (isModelDisabledError(error)) {
        await refreshModelOptions();
      }
      setTopicScriptError(errorToMessage(error));
    } finally {
      setGeneratingTopicScript(false);
    }
  }

  async function handleSendAgentMessage(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const content = agentDraft.trim();

    if (!selectedProjectId) {
      setAgentError("请先选择项目");
      return;
    }

    if (!content) {
      setAgentError("请输入脚本需求或修改方向");
      return;
    }
    if (scriptAgentModelUnavailable) {
      setAgentError("请选择可用的文本模型");
      return;
    }

    setAgentError("");
    setSendingAgentMessage(true);
    const projectIdAtSend = selectedProjectId;
    const scriptIdAtSend = selectedScript?.script_id ?? null;
    const stillCurrentContext = () =>
      selectedProjectIdRef.current === projectIdAtSend && selectedScriptIdRef.current === scriptIdAtSend;

    try {
      let conversationId = agentConversationId;
      if (!conversationId) {
        const payload = selectedScript
          ? {
              project_id: selectedProjectId,
              agent_type: "script" as const,
              subject_type: "script",
              subject_id: selectedScript.script_id,
              title: "脚本 Agent 对话",
            }
          : {
              project_id: selectedProjectId,
              agent_type: "script" as const,
              title: "脚本 Agent 对话",
            };
        const conversation = await createAgentConversation(client, {
          ...payload,
        });
        if (!stillCurrentContext()) {
          return;
        }
        conversationId = conversation.conversation_id;
        setAgentConversationId(conversationId);
      }

      const response = await sendAgentMessage(client, conversationId, {
        content,
        model_id: scriptAgentModelId,
      });
      if (!stillCurrentContext()) {
        return;
      }
      setAgentMessages((currentMessages) => [
        ...currentMessages,
        response.user_message,
        response.assistant_message,
      ]);
      setAgentDraft("");

      if (selectedScript) {
        const refreshedScript = await getScript(client, selectedScript.script_id);
        if (!stillCurrentContext()) {
          return;
        }
        setSelectedScript(refreshedScript);
        setScripts((currentScripts) => upsertSummary(currentScripts, refreshedScript));
        return;
      }

      const metadata = getScriptAgentTurnMetadata(response.assistant_message);
      if (!metadata.script_created || !metadata.script_id) {
        return;
      }
      const createdScript = await getScript(client, metadata.script_id);
      if (!stillCurrentContext()) {
        return;
      }
      preserveAgentConversationRef.current = conversationId;
      selectedScriptIdRef.current = createdScript.script_id;
      setSelectedScriptId(createdScript.script_id);
      setSelectedScript(createdScript);
      setScripts((currentScripts) => upsertSummary(currentScripts, createdScript));
      setStatusFilter("all");
    } catch (error) {
      if (isModelDisabledError(error)) {
        await refreshModelOptions();
      }
      if (stillCurrentContext()) {
        setAgentError(errorToMessage(error));
      }
    } finally {
      if (stillCurrentContext()) {
        setSendingAgentMessage(false);
      }
    }
  }

  return (
    <WorkspaceShell
      apiAvailable={apiAvailable}
      loadingMenus={loadingMenus}
      menuError={menuError}
      projects={projects}
      selectedMenuKey={selectedMenuKey}
      selectedSubMenuKey={selectedSubMenuKey}
      selectedProjectId={selectedProjectId}
      workspaceMenus={workspaceMenus}
      onSelectMenu={handleSelectWorkspaceMenu}
      onSelectSubMenu={handleSelectWorkspaceSubMenu}
      onSelectProject={setSelectedProjectId}
      overlay={
        scriptPreparation ? (
          <ScriptPreparationDialog
            error={topicScriptError}
            generating={generatingTopicScript}
            modelUnavailable={scriptGenerateModelUnavailable}
            modelSelect={
              <ModelSelect
                error={textModelError}
                id="script-generate-model"
                label="推理模型"
                loading={loadingTextModels}
                models={textModelOptions}
                value={scriptGenerateModelId}
                onChange={setScriptGenerateModelId}
              />
            }
            options={scriptPrepareOptions}
            preparation={scriptPreparation}
            onClose={() => setScriptPreparation(null)}
            onConfirm={handleConfirmTopicScriptGeneration}
            onOptionsChange={setScriptPrepareOptions}
          />
        ) : null
      }
    >
      {selectedMenuKey === contentStrategyMenuKey && contentStrategyView === "account" ? (
        <AccountStrategyPage
          project={selectedProject}
          form={accountStrategyForm}
          hasUnsavedChanges={hasAccountStrategyUnsavedChanges}
          draftNotes={accountStrategyDraftNotes}
          draftSummary={accountStrategyDraftSummary}
          error={accountStrategyError}
          generatingDraft={generatingAccountStrategyDraft}
          modelUnavailable={strategyModelUnavailable}
          modelSelect={
            <ModelSelect
              error={textModelError}
              id="strategy-draft-model"
              label="推理模型"
              loading={loadingTextModels}
              models={textModelOptions}
              value={strategyModelId}
              onChange={setStrategyModelId}
            />
          }
          saving={savingAccountStrategy}
          writesDisabled={writesDisabled}
          onBackToTopicPool={() => setContentStrategyView("pool")}
          onCancel={handleCancelAccountStrategyEdit}
          onDraftNotesChange={setAccountStrategyDraftNotes}
          onFormChange={handleAccountStrategyFormChange}
          onGenerateDraft={handleGenerateAccountStrategyDraft}
          onSubmit={handleSubmitAccountStrategy}
        />
      ) : selectedMenuKey === contentStrategyMenuKey && contentStrategyView === "history" ? (
        <TopicHistoryPage
          project={selectedProject}
          topics={topics}
          stats={topicStats}
          activeTopicBatchId={historyActiveTopicBatchId}
          topicBatches={topicBatches}
          topicGroups={topicGroups}
          topicGroupSort={topicGroupSort}
          qualityError={topicQualityError}
          qualityEvaluation={topicQualityEvaluation}
          qualityLoading={topicQualityLoading}
          loadingTopicBatches={!topicBatchesLoaded || !topicGroupsLoaded}
          topicBatchError={topicBatchError || topicGroupError}
          loading={loadingTopics}
          preparingScript={preparingScript}
          error={topicError}
          actionError={topicActionError}
          reviewError={topicReviewError}
          reviewLoading={topicReviewLoading}
          reviewSnapshot={topicReviewSnapshot}
          reviewModelUnavailable={reviewModelUnavailable}
          reviewModelSelect={
            <ModelSelect
              error={textModelError}
              id="topic-review-model"
              label="评审模型"
              loading={loadingTextModels}
              models={textModelOptions}
              value={reviewModelId}
              onChange={setReviewModelId}
            />
          }
          topicModelUnavailable={topicModelUnavailable}
          topicModelSelect={
            <ModelSelect
              error={textModelError}
              id="topic-supplement-model"
              label="推理模型"
              loading={loadingTextModels}
              models={textModelOptions}
              value={topicModelId}
              onChange={setTopicModelId}
            />
          }
          deletingTopicId={deletingTopicId}
          writesDisabled={writesDisabled}
          onDeleteTopic={handleDeleteTopic}
          onPrepareScript={handlePrepareScriptFromTopic}
          onReviewTopicGroup={handleReviewTopicGroup}
          onSelectTopicBatch={handleSelectHistoryTopicBatch}
          onSupplementTopicBatch={handleSupplementTopicBatch}
          onTopicGroupSortChange={setTopicGroupSort}
          onUpdateTopicStatus={handleUpdateTopicStatus}
        />
      ) : selectedMenuKey === contentStrategyMenuKey ? (
        <ContentStrategyPage
          project={selectedProject}
          topics={topics}
          stats={topicStats}
          selectedTopic={selectedTopic}
          statusFilter={topicStatusFilter}
          activeTopicBatchId={poolActiveTopicBatchId}
          activeTopicReviewRootBatchId={activeTopicReviewRootBatchId}
          reviewError={topicReviewError}
          reviewLoading={topicReviewLoading}
          reviewSnapshot={topicReviewSnapshot}
          showingAllTopicBatches={topicBatchViewMode === "all"}
          loading={loadingTopics}
          error={topicError}
          actionError={topicActionError}
          deletingTopicId={deletingTopicId}
          writesDisabled={writesDisabled}
          showTopicForm={showTopicForm}
          editingTopicId={editingTopicId}
          topicForm={topicForm}
          savingTopic={savingTopic}
          agentDraft={topicAgentDraft}
          agentError={topicAgentError}
          agentMessages={topicAgentMessages}
          sendingAgentMessage={sendingTopicAgentMessage}
          topicModelUnavailable={topicModelUnavailable}
          topicModelSelect={
            <ModelSelect
              error={textModelError}
              id="topic-generation-model"
              label="推理模型"
              loading={loadingTextModels}
              models={textModelOptions}
              value={topicModelId}
              onChange={setTopicModelId}
            />
          }
          preparingScript={preparingScript}
          onSelectTopic={setSelectedTopicId}
          onClearTopicBatchFilter={() => {
            setTopicBatchViewMode("all");
            setTopicBatchFilter(null);
          }}
          onDeleteTopic={handleDeleteTopic}
          onStatusFilterChange={setTopicStatusFilter}
          onNewTopic={handleNewTopic}
          onEditTopic={handleEditTopic}
          onCancelTopicForm={handleCancelTopicForm}
          onTopicFormChange={(field, value) =>
            setTopicForm((currentForm) => ({ ...currentForm, [field]: value }))
          }
          onSubmitTopic={handleSubmitTopic}
          onUpdateTopicStatus={handleUpdateTopicStatus}
          onPrepareScript={handlePrepareScriptFromTopic}
          setAgentDraft={setTopicAgentDraft}
          onSubmitAgentMessage={handleSendTopicAgentMessage}
        />
      ) : selectedMenuKey === materialManagementMenuKey && selectedMaterialSubMenuKey === assetGenerationMenuKey ? (
        <AssetGenerationPage
          assetCandidatePanel={
            selectedScript
              ? {
                  actionInProgress: assetActionInProgress,
                  dismissingTaskId: dismissingAssetTaskId,
                  candidates: assetCandidates,
                  candidateCount: assetCandidateCount,
                  error: assetError,
                  loadingCandidates: loadingAssetCandidates,
                  loadingPlan: loadingAssetPlan,
                  plan: assetPlan,
                  modelUnavailable: imageModelUnavailable,
                  modelSelect: (
                    <ModelSelect
                      error={imageModelError}
                      id="asset-image-model"
                      label="图片模型"
                      loading={loadingImageModels}
                      models={imageModelOptions}
                      value={imageModelId}
                      onChange={setImageModelId}
                    />
                  ),
                  selectedSceneId: selectedAssetSceneId,
                  tasks: assetTasks,
                  taskToDismissId: assetTaskToDismissId,
                  useReferenceMaterials,
                  onCandidateCountChange: setAssetCandidateCount,
                  onConfirmVideoTask: handleConfirmAssetGenerationTask,
                  onCancelDismissTask: () => setAssetTaskToDismissId(null),
                  onConfirmDismissTask: handleDismissAssetGenerationTask,
                  onGenerateCandidates: handleCreateAssetGenerationTasks,
                  onRegenerateScene: handleRegenerateSceneAsset,
                  onRequestDismissTask: setAssetTaskToDismissId,
                  onRejectCandidate: handleRejectAssetCandidate,
                  onSelectCandidate: handleSelectAssetCandidate,
                  onSelectScene: setSelectedAssetSceneId,
                  onUseReferenceMaterialsChange: setUseReferenceMaterials,
                }
              : null
          }
          loadingProjects={loadingProjects}
          loadingScriptDetail={loadingScriptDetail}
          loadingScripts={loadingScripts}
          project={selectedProject}
          scriptError={scriptError}
          scripts={scripts}
          selectedProjectId={selectedProjectId}
          selectedScript={selectedScript}
          selectedScriptId={selectedScriptId}
          writesDisabled={writesDisabled}
          onOpenScript={handleOpenScript}
        />
      ) : selectedMenuKey === materialManagementMenuKey ? (
        <MaterialLibraryPage
          actionError={materialActionError}
          error={materialError}
          filters={materialFilters}
          form={materialForm}
          creatingMaterial={creatingMaterial}
          loading={loadingMaterials}
          materials={materials}
          saving={savingMaterial}
          selectedMaterial={selectedMaterial}
          onFilterChange={setMaterialFilters}
          onFormChange={setMaterialForm}
          onNewMaterial={handleNewMaterial}
          onSaveMaterial={handleSaveMaterial}
          onSelectMaterial={handleSelectMaterial}
          onUpdateStatus={handleUpdateMaterialStatus}
        />
      ) : (
        <ScriptCreationPage
          agentDraft={agentDraft}
          agentError={agentError}
          agentMessages={agentMessages}
          loadingProjects={loadingProjects}
          loadingScriptDetail={loadingScriptDetail}
          loadingScripts={loadingScripts}
          modelUnavailable={scriptAgentModelUnavailable}
          modelSelect={
            <ModelSelect
              error={textModelError}
              id="script-agent-model"
              label="推理模型"
              loading={loadingTextModels}
              models={textModelOptions}
              value={scriptAgentModelId}
              onChange={setScriptAgentModelId}
            />
          }
          projectError={projectError}
          scriptError={scriptError}
          scripts={scripts}
          selectedProject={selectedProject}
          selectedProjectId={selectedProjectId}
          selectedScript={selectedScript}
          selectedScriptId={selectedScriptId}
          sendingAgentMessage={sendingAgentMessage}
          statusError={statusError}
          statusFilter={statusFilter}
          updatingStatus={updatingStatus}
          writesDisabled={writesDisabled}
          onNewScript={handleNewScript}
          onOpenScript={handleOpenScript}
          onStatusFilterChange={setStatusFilter}
          onSubmitAgentMessage={handleSendAgentMessage}
          onUpdateStatus={handleUpdateStatus}
          setAgentDraft={setAgentDraft}
        />
      )}
    </WorkspaceShell>
  );
}

async function openScript(
  client: ApiClient,
  scriptId: string,
  active: boolean,
  setLoading: (value: boolean) => void,
  setScript: (script: ScriptDetail | null) => void,
  setError: (message: string) => void,
  shouldApply: () => boolean = () => true,
) {
  setLoading(true);
  setError("");
  try {
    const script = await getScript(client, scriptId);
    if (active && shouldApply()) {
      setScript(script);
    }
  } catch (error) {
    if (active && shouldApply()) {
      setError(errorToMessage(error));
    }
  } finally {
    if (active && shouldApply()) {
      setLoading(false);
    }
  }
}

function errorToMessage(error: unknown) {
  if (error instanceof ApiError) {
    return error.message;
  }
  if (error instanceof Error) {
    return error.message;
  }
  return "请求失败";
}

function isModelDisabledError(error: unknown) {
  if (!(error instanceof ApiError) || !error.details || typeof error.details !== "object") {
    return false;
  }
  const body = error.details as { error?: { code?: unknown } };
  return body.error?.code === "model_disabled";
}

function getTopicAgentBatchId(message: AgentMessage) {
  const batchId = message.metadata.batch_id;
  return typeof batchId === "string" && batchId.trim() ? batchId : null;
}

function topicListFilters(
  status: "all" | ContentTopicStatus,
  source: "all" | ContentTopicSource,
  batchId: string | null,
) {
  return {
    status,
    source,
    ...(batchId ? { batch_id: batchId } : {}),
  };
}
