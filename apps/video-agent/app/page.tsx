"use client";

import { type FormEvent, useEffect, useMemo, useRef, useState } from "react";
import { WorkspaceShell } from "./components/workspace/WorkspaceShell";
import {
  AgentMessage,
  ApiClient,
  ApiError,
  ContentTopic,
  ContentTopicSource,
  ContentTopicStats,
  ContentTopicStatus,
  PrepareScriptFromTopicResponse,
  Project,
  ScriptDetail,
  ScriptStatus,
  ScriptSummary,
  ScriptStyle,
  TopicGenerationBatchSummary,
  WorkspaceMenuNode,
  checkHealth,
  createAgentConversation,
  createApiClient,
  createContentTopic,
  generateScript,
  getScript,
  getScriptAgentTurnMetadata,
  listProjects,
  listScripts,
  listContentTopics,
  listTopicGenerationBatches,
  listWorkspaceMenus,
  prepareScriptFromTopic,
  sendAgentMessage,
  updateContentTopic,
  updateContentTopicStatus,
  updateScriptStatus,
} from "./lib/api";
import { ContentStrategyPage, ScriptPreparationDialog } from "./pages/content-strategy/ContentStrategyPage";
import {
  adjustTopicStats,
  defaultTopicForm,
  emptyTopicStats,
  sortContentTopicsByScore,
  topicPayloadFromForm,
  topicToForm,
  type TopicFormState,
} from "./pages/content-strategy/topicModel";
import { ScriptCreationPage } from "./pages/script-creation/ScriptCreationPage";
import { upsertSummary } from "./pages/script-creation/scriptModel";

const contentStrategyMenuKey = "content-strategy";
const scriptCreationMenuKey = "script-creation";
const defaultMenuKey = contentStrategyMenuKey;

function visibleTopicGenerationBatches(batches: TopicGenerationBatchSummary[]) {
  return batches.filter((batch) => batch.status === "succeeded" && batch.topic_count > 0);
}

export default function Home() {
  const client = useMemo(() => createApiClient(), []);
  const [apiAvailable, setApiAvailable] = useState<boolean | null>(null);
  const [projects, setProjects] = useState<Project[]>([]);
  const [selectedProjectId, setSelectedProjectId] = useState("");
  const [scripts, setScripts] = useState<ScriptSummary[]>([]);
  const [selectedScript, setSelectedScript] = useState<ScriptDetail | null>(null);
  const [selectedScriptId, setSelectedScriptId] = useState<string | null>(null);
  const [topics, setTopics] = useState<ContentTopic[]>([]);
  const [topicStats, setTopicStats] = useState<ContentTopicStats>(emptyTopicStats);
  const [topicBatches, setTopicBatches] = useState<TopicGenerationBatchSummary[]>([]);
  const [topicBatchesLoaded, setTopicBatchesLoaded] = useState(false);
  const [topicBatchViewMode, setTopicBatchViewMode] = useState<"latest" | "batch" | "all">("latest");
  const [selectedTopicId, setSelectedTopicId] = useState<string | null>(null);
  const [topicStatusFilter, setTopicStatusFilter] = useState<"all" | ContentTopicStatus>("all");
  const [topicSourceFilter] = useState<"all" | ContentTopicSource>("all");
  const [topicBatchFilter, setTopicBatchFilter] = useState<string | null>(null);
  const [workspaceMenus, setWorkspaceMenus] = useState<WorkspaceMenuNode[]>([]);
  const [selectedMenuKey, setSelectedMenuKey] = useState(defaultMenuKey);
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
  const [topicActionError, setTopicActionError] = useState("");
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
  const selectedScriptIdRef = useRef<string | null>(null);
  const selectedProjectIdRef = useRef("");
  const preserveAgentConversationRef = useRef<string | null>(null);

  const selectedProject = projects.find((project) => project.project_id === selectedProjectId);
  const activeTopicBatchId =
    topicBatchViewMode === "all"
      ? null
      : topicBatchViewMode === "batch"
        ? topicBatchFilter
        : topicBatches[0]?.batch_id || null;
  const selectedTopic = topics.find((topic) => topic.topic_id === selectedTopicId) || null;
  const writesDisabled = apiAvailable === false;

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
    if (!selectedProjectId || selectedMenuKey !== "content-strategy") {
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

    let active = true;

    async function loadProjectTopics() {
      setLoadingTopics(true);
      setTopicError("");

      try {
        const response = await listContentTopics(
          client,
          selectedProjectId,
          topicListFilters(topicStatusFilter, topicSourceFilter, activeTopicBatchId),
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
    selectedProjectId,
    selectedMenuKey,
    topicBatchesLoaded,
    topicSourceFilter,
    topicStatusFilter,
  ]);

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
    setTopicBatches([]);
    setTopicBatchesLoaded(false);
    setTopicBatchError("");
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

  async function refreshContentTopics(batchId: string | null = activeTopicBatchId) {
    if (!selectedProjectId) {
      return;
    }
    const response = await listContentTopics(
      client,
      selectedProjectId,
      topicListFilters(topicStatusFilter, topicSourceFilter, batchId),
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
      return;
    }
    const response = await listTopicGenerationBatches(client, selectedProjectId);
    setTopicBatches(visibleTopicGenerationBatches(response.batches));
    setTopicBatchesLoaded(true);
    setTopicBatchError("");
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

      const response = await sendAgentMessage(client, conversationId, { content });
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
      } catch (error) {
        setTopicBatchError(errorToMessage(error));
      }
      await refreshContentTopics(batchId);
    } catch (error) {
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

    setGeneratingTopicScript(true);
    setTopicScriptError("");

    try {
      const script = await generateScript(client, {
        project_id: selectedProjectId,
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

      const response = await sendAgentMessage(client, conversationId, { content });
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
      selectedProjectId={selectedProjectId}
      workspaceMenus={workspaceMenus}
      onSelectMenu={setSelectedMenuKey}
      onSelectProject={setSelectedProjectId}
      overlay={
        scriptPreparation ? (
          <ScriptPreparationDialog
            error={topicScriptError}
            generating={generatingTopicScript}
            options={scriptPrepareOptions}
            preparation={scriptPreparation}
            onClose={() => setScriptPreparation(null)}
            onConfirm={handleConfirmTopicScriptGeneration}
            onOptionsChange={setScriptPrepareOptions}
          />
        ) : null
      }
    >
      {selectedMenuKey === contentStrategyMenuKey ? (
        <ContentStrategyPage
          project={selectedProject}
          topics={topics}
          stats={topicStats}
          selectedTopic={selectedTopic}
          statusFilter={topicStatusFilter}
          activeTopicBatchId={activeTopicBatchId}
          showingAllTopicBatches={topicBatchViewMode === "all"}
          topicBatches={topicBatches}
          loadingTopicBatches={!topicBatchesLoaded}
          topicBatchError={topicBatchError}
          loading={loadingTopics}
          error={topicError}
          actionError={topicActionError}
          writesDisabled={writesDisabled}
          showTopicForm={showTopicForm}
          editingTopicId={editingTopicId}
          topicForm={topicForm}
          savingTopic={savingTopic}
          agentDraft={topicAgentDraft}
          agentError={topicAgentError}
          agentMessages={topicAgentMessages}
          sendingAgentMessage={sendingTopicAgentMessage}
          preparingScript={preparingScript}
          onSelectTopic={setSelectedTopicId}
          onClearTopicBatchFilter={() => {
            setTopicBatchViewMode("all");
            setTopicBatchFilter(null);
          }}
          onSelectTopicBatch={(batchId) => {
            setTopicBatchViewMode("batch");
            setTopicBatchFilter(batchId);
          }}
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
      ) : (
        <ScriptCreationPage
          agentDraft={agentDraft}
          agentError={agentError}
          agentMessages={agentMessages}
          loadingProjects={loadingProjects}
          loadingScriptDetail={loadingScriptDetail}
          loadingScripts={loadingScripts}
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
