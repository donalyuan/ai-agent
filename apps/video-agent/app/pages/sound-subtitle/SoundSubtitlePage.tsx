"use client";

import {
  type KeyboardEvent as ReactKeyboardEvent,
  useCallback,
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
} from "react";
import {
  AgentMessage,
  ApiClient,
  ApiError,
  AudioInspection,
  Material,
  ModelOption,
  ScriptDetail,
  ScriptSummary,
  SoundTask,
  SoundTaskIntent,
  SoundTaskPreflight,
  VoiceCatalog,
  VoiceCatalogEntry,
  cancelSoundTask,
  createAgentConversation,
  createSoundTask,
  getAudioInspection,
  getScript,
  getVoiceCatalog,
  listMaterials,
  listModelOptions,
  listScripts,
  listSoundTasks,
  preflightSoundTask,
  requestAudioInspection,
  requestWorkspaceVoiceCatalogCheck,
  retrySoundTask,
  sendAgentMessage,
} from "../../lib/api";
import {
  VoiceCatalogSelect as SharedVoiceCatalogSelect,
  extractLanguageOptions,
  languageLabel,
} from "../../components/VoiceCatalogSelect";

type SoundTab = "tts" | "subtitle";
type SubtitleSource = "tts" | "asr";
type SoundTaskFilter = "all" | "active" | "failed";

type PendingConfirmation = {
  intent: SoundTaskIntent;
  preflight: SoundTaskPreflight;
  retryTaskId?: string;
};

type CatalogOption = { value: string; label: string };
type ImportScriptStatusFilter = "all" | "draft" | "approved";

type ImportedScriptSource = {
  scriptId: string;
  scriptTitle: string;
  updatedAt: string;
  sceneIds: string[];
};

function LanguageCatalogSelect({
  options,
  value,
  invalid,
  disabled,
  onChange,
}: {
  options: CatalogOption[];
  value: string;
  invalid: boolean;
  disabled: boolean;
  onChange: (value: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const [activeIndex, setActiveIndex] = useState(0);
  const rootRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const optionRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const listboxId = useId();
  const selectedOption = options.find((option) => option.value === value);
  const displayLabel = invalid
    ? `${languageLabel(value)}（已失效）`
    : selectedOption?.label || "请选择语言";

  const closeMenu = useCallback((restoreFocus = false) => {
    setOpen(false);
    if (restoreFocus) triggerRef.current?.focus();
  }, []);

  useEffect(() => {
    if (!open) return;
    const handleOutsideClick = (event: MouseEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) closeMenu();
    };
    const handleEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") closeMenu(true);
    };
    document.addEventListener("mousedown", handleOutsideClick);
    document.addEventListener("keydown", handleEscape);
    return () => {
      document.removeEventListener("mousedown", handleOutsideClick);
      document.removeEventListener("keydown", handleEscape);
    };
  }, [closeMenu, open]);

  useEffect(() => {
    if (open) optionRefs.current[activeIndex]?.focus();
  }, [open]);

  function openMenu() {
    if (disabled || !options.length) return;
    const selectedIndex = options.findIndex((option) => option.value === value);
    setActiveIndex(selectedIndex >= 0 ? selectedIndex : 0);
    setOpen(true);
  }

  function toggleMenu() {
    if (open) closeMenu();
    else openMenu();
  }

  function chooseOption(option: CatalogOption) {
    onChange(option.value);
    closeMenu(true);
  }

  function focusOption(index: number) {
    const nextIndex = Math.min(Math.max(index, 0), options.length - 1);
    setActiveIndex(nextIndex);
    optionRefs.current[nextIndex]?.focus();
  }

  function handleTriggerKeyDown(event: ReactKeyboardEvent<HTMLButtonElement>) {
    if (!["ArrowDown", "ArrowUp"].includes(event.key)) return;
    event.preventDefault();
    if (!open) openMenu();
  }

  function handleOptionKeyDown(event: ReactKeyboardEvent<HTMLButtonElement>, index: number) {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      focusOption(index + 1);
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      focusOption(index - 1);
    } else if (event.key === "Home") {
      event.preventDefault();
      focusOption(0);
    } else if (event.key === "End") {
      event.preventDefault();
      focusOption(options.length - 1);
    }
  }

  return (
    <div className="languageCatalogSelect" ref={rootRef}>
      <button
        ref={triggerRef}
        type="button"
        role="combobox"
        aria-label="语言 / 口音"
        aria-expanded={open}
        aria-controls={open ? listboxId : undefined}
        aria-haspopup="listbox"
        className={`languageCatalogTrigger${invalid ? " invalid" : ""}`}
        disabled={disabled || !options.length}
        onClick={toggleMenu}
        onKeyDown={handleTriggerKeyDown}
      >
        <span className="languageCatalogTriggerCopy">
          <span>语言 / 口音</span>
          <strong>{displayLabel}</strong>
        </span>
        <span className={`voiceCatalogChevron${open ? " open" : ""}`} aria-hidden="true" />
      </button>

      {open && (
        <div id={listboxId} className="languageCatalogList" role="listbox" aria-label="语言 / 口音选项">
          {options.map((option, index) => (
            <button
              ref={(element) => { optionRefs.current[index] = element; }}
              type="button"
              role="option"
              aria-selected={option.value === value}
              className={`languageCatalogOption${option.value === value ? " selected" : ""}${index === activeIndex ? " active" : ""}`}
              data-value={option.value}
              key={option.value}
              onMouseEnter={() => setActiveIndex(index)}
              onKeyDown={(event) => handleOptionKeyDown(event, index)}
              onClick={() => chooseOption(option)}
            >
              {option.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

function TtsModelSelect({
  models,
  value,
  disabled,
  onChange,
}: {
  models: ModelOption[];
  value: string;
  disabled: boolean;
  onChange: (value: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const [activeIndex, setActiveIndex] = useState(0);
  const rootRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const optionRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const listboxId = useId();
  const selectedModel = models.find((model) => model.model_id === value);

  const closeMenu = useCallback((restoreFocus = false) => {
    setOpen(false);
    if (restoreFocus) triggerRef.current?.focus();
  }, []);

  useEffect(() => {
    if (!open) return;
    const handleOutsideClick = (event: MouseEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) closeMenu();
    };
    const handleEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") closeMenu(true);
    };
    document.addEventListener("mousedown", handleOutsideClick);
    document.addEventListener("keydown", handleEscape);
    optionRefs.current[activeIndex]?.focus();
    return () => {
      document.removeEventListener("mousedown", handleOutsideClick);
      document.removeEventListener("keydown", handleEscape);
    };
  }, [activeIndex, closeMenu, open]);

  function openMenu() {
    if (disabled || !models.length) return;
    const selectedIndex = models.findIndex((model) => model.model_id === value);
    setActiveIndex(selectedIndex >= 0 ? selectedIndex : 0);
    setOpen(true);
  }

  function toggleMenu() {
    if (open) closeMenu();
    else openMenu();
  }

  function chooseModel(model: ModelOption) {
    onChange(model.model_id);
    closeMenu(true);
  }

  function focusOption(index: number) {
    const nextIndex = Math.min(Math.max(index, 0), models.length - 1);
    setActiveIndex(nextIndex);
    optionRefs.current[nextIndex]?.focus();
  }

  function handleTriggerKeyDown(event: ReactKeyboardEvent<HTMLButtonElement>) {
    if (!["ArrowDown", "ArrowUp"].includes(event.key)) return;
    event.preventDefault();
    if (!open) openMenu();
  }

  function handleOptionKeyDown(event: ReactKeyboardEvent<HTMLButtonElement>, index: number) {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      focusOption(index + 1);
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      focusOption(index - 1);
    } else if (event.key === "Home") {
      event.preventDefault();
      focusOption(0);
    } else if (event.key === "End") {
      event.preventDefault();
      focusOption(models.length - 1);
    }
  }

  return (
    <div className="soundModelSelect" ref={rootRef}>
      <button
        ref={triggerRef}
        type="button"
        role="combobox"
        aria-label="TTS 模型"
        aria-expanded={open}
        aria-controls={open ? listboxId : undefined}
        aria-haspopup="listbox"
        className="soundModelTrigger"
        disabled={disabled || !models.length}
        onClick={toggleMenu}
        onKeyDown={handleTriggerKeyDown}
      >
        <span className="soundModelTriggerCopy">
          <span>TTS 模型</span>
          <strong>{selectedModel?.display_name || "暂无启用模型"}</strong>
        </span>
        <span className={`voiceCatalogChevron${open ? " open" : ""}`} aria-hidden="true" />
      </button>

      {open && (
        <div id={listboxId} className="soundModelList" role="listbox" aria-label="TTS 模型选项">
          {models.map((model, index) => (
            <button
              ref={(element) => { optionRefs.current[index] = element; }}
              type="button"
              role="option"
              aria-selected={model.model_id === value}
              className={`soundModelOption${model.model_id === value ? " selected" : ""}${index === activeIndex ? " active" : ""}`}
              key={model.model_id}
              onMouseEnter={() => setActiveIndex(index)}
              onKeyDown={(event) => handleOptionKeyDown(event, index)}
              onClick={() => chooseModel(model)}
            >
              <strong>{model.display_name}</strong>
              <span>{model.provider_name} · {model.upstream_model}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

export function SoundSubtitlePage({
  client,
  projectId,
  projectName,
  writesDisabled = false,
}: {
  client: ApiClient;
  projectId: string;
  projectName: string;
  writesDisabled?: boolean;
}) {
  const [activeTab, setActiveTab] = useState<SoundTab>("tts");
  const [subtitleSource, setSubtitleSource] = useState<SubtitleSource>("tts");
  const [speechModels, setSpeechModels] = useState<ModelOption[]>([]);
  const [textModels, setTextModels] = useState<ModelOption[]>([]);
  const [ttsModelId, setTtsModelId] = useState("");
  const [asrModelId, setAsrModelId] = useState("");
  const [agentModelId, setAgentModelId] = useState("");
  const [modelError, setModelError] = useState("");
  const [loadingModels, setLoadingModels] = useState(true);
  const [catalog, setCatalog] = useState<VoiceCatalog | null>(null);
  const [catalogLoading, setCatalogLoading] = useState(false);
  const [catalogError, setCatalogError] = useState("");
  const [catalogMessage, setCatalogMessage] = useState("");
  const [checkingCatalog, setCheckingCatalog] = useState(false);
  const [voiceType, setVoiceType] = useState("");
  const [voiceLabels, setVoiceLabels] = useState<Record<string, string>>({});
  const [language, setLanguage] = useState("");
  const [ttsText, setTtsText] = useState("");
  const [subtitleSegments, setSubtitleSegments] = useState("");
  const [scriptImportOpen, setScriptImportOpen] = useState(false);
  const [importScripts, setImportScripts] = useState<ScriptSummary[]>([]);
  const [importScriptsLoading, setImportScriptsLoading] = useState(false);
  const [importScriptQuery, setImportScriptQuery] = useState("");
  const [importScriptStatus, setImportScriptStatus] = useState<ImportScriptStatusFilter>("all");
  const [selectedImportScriptId, setSelectedImportScriptId] = useState("");
  const [selectedImportScript, setSelectedImportScript] = useState<ScriptDetail | null>(null);
  const [selectedImportSceneIds, setSelectedImportSceneIds] = useState<string[]>([]);
  const [importScriptLoading, setImportScriptLoading] = useState(false);
  const [importScriptError, setImportScriptError] = useState("");
  const [importedScriptSource, setImportedScriptSource] = useState<ImportedScriptSource | null>(null);
  const [speechParameters, setSpeechParameters] = useState<Record<string, unknown>>({});
  const [audioMaterials, setAudioMaterials] = useState<Material[]>([]);
  const [sourceAudioId, setSourceAudioId] = useState("");
  const [inspection, setInspection] = useState<AudioInspection | null>(null);
  const [inspectionLoading, setInspectionLoading] = useState(false);
  const [inspectionError, setInspectionError] = useState("");
  const [tasks, setTasks] = useState<SoundTask[]>([]);
  const [taskFilter, setTaskFilter] = useState<SoundTaskFilter>("all");
  const [tasksLoading, setTasksLoading] = useState(false);
  const [taskError, setTaskError] = useState("");
  const [preflighting, setPreflighting] = useState(false);
  const [submittingTask, setSubmittingTask] = useState(false);
  const [pendingConfirmation, setPendingConfirmation] = useState<PendingConfirmation | null>(null);
  const [conversationId, setConversationId] = useState<string | null>(null);
  const [agentMessages, setAgentMessages] = useState<AgentMessage[]>([]);
  const [agentDraft, setAgentDraft] = useState("");
  const [agentError, setAgentError] = useState("");
  const [sendingAgent, setSendingAgent] = useState(false);
  const ttsTextRef = useRef<HTMLTextAreaElement>(null);

  const ttsModels = useMemo(
    () => speechModels.filter((model) => [
      "volcengine_tts_v3",
      "openai_audio_speech",
    ].includes(model.api_protocol)),
    [speechModels],
  );
  const asrModels = useMemo(
    () => speechModels.filter((model) => model.api_protocol === "volcengine_asr_v3"),
    [speechModels],
  );
  const selectedVoice = catalog?.voices.find((voice) => voice.voice_type === voiceType) ?? null;
  const availableVoices = catalog?.voices.filter((voice) => voice.is_available) ?? [];
  const voiceInvalid = Boolean(voiceType) && (!selectedVoice || !selectedVoice.is_available);
  const languageOptions = extractLanguageOptions(selectedVoice?.languages);
  const languageInvalid = Boolean(language)
    && Boolean(selectedVoice)
    && !languageOptions.some((option) => option.value === language);
  const parameterDefinitions = useMemo(() => {
    const value = catalog?.model_settings.parameters;
    return value && typeof value === "object" && !Array.isArray(value)
      ? value as Record<string, Record<string, unknown>>
      : {};
  }, [catalog]);
  const parameterInvalid = Object.keys(speechParameters).some(
    (name) => !["audio_format", "sample_rate"].includes(name) && !(name in parameterDefinitions),
  ) || Object.entries(parameterDefinitions).some(([name, definition]) => {
    const value = speechParameters[name];
    if (definition.type !== "number" || typeof value !== "number") return false;
    const minimum = numericValue(definition.minimum ?? definition.min);
    const maximum = numericValue(definition.maximum ?? definition.max);
    return (minimum !== null && value < minimum) || (maximum !== null && value > maximum);
  });
  const audioFormatInvalid = typeof speechParameters.audio_format === "string"
    && Array.isArray(catalog?.model_settings.supported_audio_formats)
    && !(catalog.model_settings.supported_audio_formats as unknown[]).includes(speechParameters.audio_format);
  const sampleRateInvalid = typeof speechParameters.sample_rate === "number"
    && Array.isArray(catalog?.model_settings.supported_sample_rates)
    && !(catalog.model_settings.supported_sample_rates as unknown[]).includes(speechParameters.sample_rate);
  const ttsSelectionInvalid = !catalog
    || !voiceType
    || !language
    || voiceInvalid
    || languageInvalid
    || parameterInvalid
    || audioFormatInvalid
    || sampleRateInvalid;
  const ttsTimestampUnavailable = Boolean(catalog)
    && catalog?.model_settings.supports_word_timestamps !== true;
  const selectedAudio = audioMaterials.find((material) => material.material_id === sourceAudioId) ?? null;
  const recommendation = [...agentMessages]
    .reverse()
    .find((message) => message.role === "assistant" && message.metadata.requires_confirmation === true) ?? null;
  const activeTasks = tasks.filter((task) => task.status === "queued" || task.status === "running");
  const failedTasks = tasks.filter((task) => task.status === "failed");
  const visibleTasks = tasks.filter((task) => {
    if (taskFilter === "active") return task.status === "queued" || task.status === "running";
    if (taskFilter === "failed") return task.status === "failed";
    return true;
  });
  const latestTask = tasks[0] ?? null;
  const latestPlayableTask = tasks.find((task) => (
    task.status === "succeeded" && typeof task.result?.audio_file_url === "string"
  )) ?? null;
  const maxInputCharacters = Math.max(
    1,
    Math.floor(numberValue(catalog?.model_settings.max_input_characters) ?? 3000),
  );
  const visibleImportScripts = useMemo(() => {
    const query = importScriptQuery.trim().toLocaleLowerCase();
    return importScripts.filter((script) => (
      (importScriptStatus === "all" || script.status === importScriptStatus)
      && (!query || `${script.title}\n${script.source_topic_title ?? ""}`.toLocaleLowerCase().includes(query))
    ));
  }, [importScriptQuery, importScriptStatus, importScripts]);
  const selectableImportScenes = useMemo(
    () => (selectedImportScript?.scenes ?? [])
      .filter((scene) => scene.narration.trim())
      .sort((left, right) => left.sequence - right.sequence),
    [selectedImportScript],
  );
  const selectedImportScenes = selectableImportScenes.filter(
    (scene) => selectedImportSceneIds.includes(scene.scene_id),
  );
  const selectedImportText = selectedImportScenes
    .map((scene) => scene.narration.trim())
    .join("\n");
  const selectedImportCharacterCount = Array.from(selectedImportText).length;
  const importExceedsLimit = selectedImportCharacterCount > maxInputCharacters;

  useEffect(() => {
    let active = true;
    setLoadingModels(true);
    setModelError("");
    Promise.all([listModelOptions(client, "speech"), listModelOptions(client, "text")])
      .then(([speech, text]) => {
        if (!active) return;
        setSpeechModels(speech.models);
        setTextModels(text.models);
        const nextTts = speech.models.find((model) => [
          "volcengine_tts_v3",
          "openai_audio_speech",
        ].includes(model.api_protocol) && model.is_default)
          ?? speech.models.find((model) => [
            "volcengine_tts_v3",
            "openai_audio_speech",
          ].includes(model.api_protocol));
        const nextAsr = speech.models.find((model) => model.api_protocol === "volcengine_asr_v3" && model.is_default)
          ?? speech.models.find((model) => model.api_protocol === "volcengine_asr_v3");
        const nextText = text.models.find((model) => model.is_default) ?? text.models[0];
        setTtsModelId((current) => current || nextTts?.model_id || "");
        setAsrModelId((current) => current || nextAsr?.model_id || "");
        setAgentModelId((current) => current || nextText?.model_id || "");
      })
      .catch((error) => {
        if (active) setModelError(errorMessage(error));
      })
      .finally(() => {
        if (active) setLoadingModels(false);
      });
    return () => { active = false; };
  }, [client]);

  useEffect(() => {
    let active = true;
    setCatalog(null);
    setCatalogMessage("");
    setCatalogError("");
    if (!ttsModelId) return () => { active = false; };
    setCatalogLoading(true);
    getVoiceCatalog(client, ttsModelId, true)
      .then((response) => {
        if (!active) return;
        setCatalog(response);
        setVoiceLabels((current) => ({
          ...current,
          ...Object.fromEntries(response.voices.map((voice) => [voice.voice_type, voice.name])),
        }));
        setVoiceType((current) => current || response.voices.find((voice) => voice.is_available)?.voice_type || "");
        setSpeechParameters((current) => initialSpeechParameters(response, current));
      })
      .catch((error) => {
        if (active) setCatalogError(errorMessage(error));
      })
      .finally(() => {
        if (active) setCatalogLoading(false);
      });
    return () => { active = false; };
  }, [client, ttsModelId]);

  useEffect(() => {
    if (!ttsModelId || !catalog?.last_sync || !["queued", "running"].includes(catalog.last_sync.status)) return;
    const timer = globalThis.setInterval(() => {
      getVoiceCatalog(client, ttsModelId, true)
        .then((response) => {
          setCatalog(response);
          setVoiceLabels((current) => ({
            ...current,
            ...Object.fromEntries(response.voices.map((voice) => [voice.voice_type, voice.name])),
          }));
          if (response.last_sync?.status === "succeeded") setCatalogMessage("音色目录同步完成");
          if (response.last_sync?.status === "failed") setCatalogError(response.last_sync.error_summary || "音色目录同步失败");
        })
        .catch((error) => setCatalogError(errorMessage(error)));
    }, 3000);
    return () => globalThis.clearInterval(timer);
  }, [catalog?.last_sync, client, ttsModelId]);

  useEffect(() => {
    if (!selectedVoice) return;
    if (!language) setLanguage(languageOptions[0]?.value ?? "");
  }, [language, languageOptions, selectedVoice]);

  const refreshTasks = useCallback(async () => {
    if (!projectId) return;
    setTasksLoading(true);
    try {
      const response = await listSoundTasks(client, projectId);
      setTasks(response.tasks);
    } catch (error) {
      setTaskError(errorMessage(error));
    } finally {
      setTasksLoading(false);
    }
  }, [client, projectId]);

  useEffect(() => {
    let active = true;
    setTaskError("");
    setInspection(null);
    setSourceAudioId("");
    if (!projectId) {
      setAudioMaterials([]);
      setTasks([]);
      return () => { active = false; };
    }
    Promise.all([
      listMaterials(client, projectId, { material_type: "audio", status: "active" }),
      listSoundTasks(client, projectId),
    ]).then(([materials, taskList]) => {
      if (!active) return;
      setAudioMaterials(materials.materials);
      setTasks(taskList.tasks);
    }).catch((error) => {
      if (active) setTaskError(errorMessage(error));
    });
    return () => { active = false; };
  }, [client, projectId]);

  useEffect(() => {
    if (!tasks.some((task) => task.status === "queued" || task.status === "running")) return;
    const timer = globalThis.setInterval(() => { void refreshTasks(); }, 4000);
    return () => globalThis.clearInterval(timer);
  }, [refreshTasks, tasks]);

  useEffect(() => {
    if (!inspection || !sourceAudioId || !["queued", "running"].includes(inspection.status)) return;
    const timer = globalThis.setInterval(() => {
      getAudioInspection(client, projectId, sourceAudioId)
        .then(setInspection)
        .catch((error) => setInspectionError(errorMessage(error)));
    }, 3000);
    return () => globalThis.clearInterval(timer);
  }, [client, inspection, projectId, sourceAudioId]);

  useEffect(() => {
    setConversationId(null);
    setAgentMessages([]);
    setAgentError("");
  }, [projectId, ttsModelId]);

  useEffect(() => {
    setImportedScriptSource(null);
    setScriptImportOpen(false);
  }, [projectId]);

  useEffect(() => {
    if (!scriptImportOpen) return;
    if (visibleImportScripts.some((script) => script.script_id === selectedImportScriptId)) return;
    setSelectedImportScriptId(visibleImportScripts[0]?.script_id ?? "");
  }, [scriptImportOpen, selectedImportScriptId, visibleImportScripts]);

  useEffect(() => {
    let active = true;
    setSelectedImportScript(null);
    setSelectedImportSceneIds([]);
    setImportScriptError("");
    if (!scriptImportOpen || !selectedImportScriptId) {
      setImportScriptLoading(false);
      return () => { active = false; };
    }
    setImportScriptLoading(true);
    getScript(client, selectedImportScriptId)
      .then((script) => {
        if (!active) return;
        if (script.project_id !== projectId || !["draft", "approved"].includes(script.status)) {
          setImportScriptError("该脚本已不属于当前账号或已不可导入，请重新选择");
          return;
        }
        setSelectedImportScript(script);
        setSelectedImportSceneIds(
          script.scenes
            .filter((scene) => scene.narration.trim())
            .sort((left, right) => left.sequence - right.sequence)
            .map((scene) => scene.scene_id),
        );
      })
      .catch((error) => {
        if (active) setImportScriptError(errorMessage(error));
      })
      .finally(() => {
        if (active) setImportScriptLoading(false);
      });
    return () => { active = false; };
  }, [client, projectId, scriptImportOpen, selectedImportScriptId]);

  function handleVoiceChange(nextVoiceType: string) {
    const voice = catalog?.voices.find((item) => item.voice_type === nextVoiceType);
    setVoiceType(nextVoiceType);
    const languages = extractLanguageOptions(voice?.languages);
    setLanguage(languages[0]?.value ?? "");
  }

  async function checkCatalog() {
    if (!ttsModelId) return;
    setCheckingCatalog(true);
    setCatalogError("");
    try {
      const sync = await requestWorkspaceVoiceCatalogCheck(client, ttsModelId);
      setCatalog((current) => current ? { ...current, last_sync: sync } : current);
      setCatalogMessage(sync.status === "succeeded" ? "音色目录已是最新版本" : "目录检查已进入队列");
    } catch (error) {
      setCatalogError(errorMessage(error));
    } finally {
      setCheckingCatalog(false);
    }
  }

  async function openScriptImport() {
    setScriptImportOpen(true);
    setImportScriptsLoading(true);
    setImportScriptError("");
    setImportScriptQuery("");
    setImportScriptStatus("all");
    setSelectedImportScriptId("");
    setSelectedImportScript(null);
    setSelectedImportSceneIds([]);
    try {
      const scripts: ScriptSummary[] = [];
      let offset = 0;
      let total = 0;
      do {
        const response = await listScripts(client, projectId, { limit: 100, offset });
        scripts.push(...response.scripts);
        total = response.total;
        offset += response.scripts.length;
        if (!response.scripts.length) break;
      } while (offset < total);
      const selectable = scripts
        .filter((script) => script.status === "draft" || script.status === "approved")
        .sort((left, right) => right.updated_at.localeCompare(left.updated_at));
      setImportScripts(selectable);
      setSelectedImportScriptId(selectable[0]?.script_id ?? "");
      if (!selectable.length) setImportScriptError("当前账号暂无可导入的草稿或已通过脚本");
    } catch (error) {
      setImportScripts([]);
      setImportScriptError(errorMessage(error));
    } finally {
      setImportScriptsLoading(false);
    }
  }

  function toggleImportScene(sceneId: string) {
    setSelectedImportSceneIds((current) => current.includes(sceneId)
      ? current.filter((id) => id !== sceneId)
      : [...current, sceneId]);
  }

  function confirmScriptImport() {
    if (!selectedImportScript || !selectedImportScenes.length || importExceedsLimit) return;
    setTtsText(selectedImportText);
    setSubtitleSegments(selectedImportScenes.map((scene) => scene.narration.trim()).join("\n"));
    setImportedScriptSource({
      scriptId: selectedImportScript.script_id,
      scriptTitle: selectedImportScript.title,
      updatedAt: selectedImportScript.updated_at,
      sceneIds: selectedImportScenes.map((scene) => scene.scene_id),
    });
    setTaskError("");
    setScriptImportOpen(false);
  }

  function currentTtsIntent(taskType: "tts" | "tts_preview", generateSubtitle: boolean): SoundTaskIntent {
    const segments = subtitleSegments.split("\n").map((item) => item.trim()).filter(Boolean);
    return {
      task_type: taskType,
      model_id: ttsModelId,
      text_content: taskType === "tts_preview" ? ttsText.slice(0, 120) : ttsText,
      voice_type: voiceType,
      language,
      parameters: speechParameters,
      generate_subtitle: generateSubtitle,
      subtitle_segments: generateSubtitle ? (segments.length ? segments : [ttsText.trim()]) : [],
      source_audio_material_id: null,
      audio_inspection_id: null,
      ...(importedScriptSource ? {
        source_script_id: importedScriptSource.scriptId,
        source_script_updated_at: importedScriptSource.updatedAt,
        source_script_scene_ids: importedScriptSource.sceneIds,
      } : {}),
    };
  }

  async function prepareTask(intent: SoundTaskIntent, retryTaskId?: string) {
    setPreflighting(true);
    setTaskError("");
    try {
      const preflight = await preflightSoundTask(client, projectId, intent);
      setPendingConfirmation({ intent, preflight, retryTaskId });
    } catch (error) {
      setTaskError(errorMessage(error));
    } finally {
      setPreflighting(false);
    }
  }

  async function confirmTask() {
    if (!pendingConfirmation) return;
    setSubmittingTask(true);
    setTaskError("");
    const payload = {
      ...pendingConfirmation.intent,
      confirmation_token: pendingConfirmation.preflight.confirmation_token,
    };
    try {
      const task = pendingConfirmation.retryTaskId
        ? await retrySoundTask(
            client,
            projectId,
            pendingConfirmation.retryTaskId,
            payload,
            newIdempotencyKey("retry"),
          )
        : await createSoundTask(client, projectId, payload, newIdempotencyKey("sound"));
      setTasks((current) => [task, ...current.filter((item) => item.task_id !== task.task_id)]);
      setPendingConfirmation(null);
    } catch (error) {
      setTaskError(errorMessage(error));
      setPendingConfirmation(null);
    } finally {
      setSubmittingTask(false);
    }
  }

  async function inspectAudio() {
    if (!sourceAudioId) return;
    setInspectionLoading(true);
    setInspectionError("");
    try {
      const response = await requestAudioInspection(
        client,
        projectId,
        sourceAudioId,
        newIdempotencyKey(`inspection-${sourceAudioId}`),
      );
      setInspection(response);
    } catch (error) {
      setInspectionError(errorMessage(error));
    } finally {
      setInspectionLoading(false);
    }
  }

  async function prepareAsrTask() {
    if (!inspection || inspection.status !== "succeeded") return;
    await prepareTask({
      task_type: "asr",
      model_id: asrModelId,
      text_content: "",
      voice_type: null,
      language: null,
      parameters: {},
      generate_subtitle: true,
      subtitle_segments: [],
      source_audio_material_id: sourceAudioId,
      audio_inspection_id: inspection.inspection_id,
    });
  }

  async function handleCancelTask(taskId: string) {
    setTaskError("");
    try {
      const task = await cancelSoundTask(client, projectId, taskId);
      setTasks((current) => current.map((item) => item.task_id === task.task_id ? task : item));
    } catch (error) {
      setTaskError(errorMessage(error));
    }
  }

  async function prepareRetry(task: SoundTask) {
    await prepareTask({
      task_type: task.task_type,
      model_id: task.model_id,
      text_content: task.text_content,
      voice_type: task.voice_type,
      language: task.language,
      parameters: task.parameters,
      generate_subtitle: task.generate_subtitle,
      subtitle_segments: task.subtitle_segments,
      source_audio_material_id: task.source_audio_material_id,
      audio_inspection_id: task.audio_inspection_id,
    }, task.task_id);
  }

  async function sendAgentSuggestion() {
    const content = agentDraft.trim();
    if (!content || !ttsModelId || !agentModelId) return;
    setSendingAgent(true);
    setAgentError("");
    try {
      let activeConversationId = conversationId;
      if (!activeConversationId) {
        const conversation = await createAgentConversation(client, {
          project_id: projectId,
          agent_type: "sound",
          title: `${projectName}声音建议`,
          metadata: { speech_model_id: ttsModelId },
        });
        activeConversationId = conversation.conversation_id;
        setConversationId(activeConversationId);
      }
      const turn = await sendAgentMessage(client, activeConversationId, {
        content,
        model_id: agentModelId,
        sound_context: {
          speech_model_id: ttsModelId,
          tts_text: ttsText.trim(),
          voice_type: voiceType,
          language,
          parameters: { ...speechParameters },
          subtitle_segments: subtitleSegments
            .split("\n")
            .map((segment) => segment.trim())
            .filter(Boolean),
        },
      });
      setAgentMessages((current) => [...current, turn.user_message, turn.assistant_message]);
      setAgentDraft("");
    } catch (error) {
      setAgentError(errorMessage(error));
    } finally {
      setSendingAgent(false);
    }
  }

  function applyRecommendation() {
    if (!recommendation) return;
    const metadata = recommendation.metadata;
    const recommendedVoice = stringValue(metadata.recommended_voice_type);
    if (!catalog?.voices.some((voice) => voice.voice_type === recommendedVoice && voice.is_available)) {
      setAgentError("Agent 推荐音色已不在当前可用目录中");
      return;
    }
    setVoiceType(recommendedVoice);
    setLanguage(stringValue(metadata.language));
    setTtsText(stringValue(metadata.tts_text));
    setImportedScriptSource(null);
    const segments = Array.isArray(metadata.subtitle_segments)
      ? metadata.subtitle_segments.filter((item): item is string => typeof item === "string")
      : [];
    setSubtitleSegments(segments.join("\n"));
    if (metadata.parameters && typeof metadata.parameters === "object" && !Array.isArray(metadata.parameters)) {
      setSpeechParameters((current) => ({ ...current, ...metadata.parameters as Record<string, unknown> }));
    }
  }

  const generationDisabled = writesDisabled
    || preflighting
    || !ttsModelId
    || !ttsText.trim()
    || ttsSelectionInvalid
    || (activeTab === "subtitle" && subtitleSource === "tts" && ttsTimestampUnavailable)
    || catalogLoading;

  return (
    <div className="soundSubtitlePage">
      <header className="soundSubtitleHeader">
        <div className="soundHeaderCopy">
          <p className="sectionKicker">素材管理 / 声音与字幕生成</p>
          <div className="soundHeaderTitleRow">
            <h2>声音与字幕生成</h2>
            <div className="soundTabs" role="tablist" aria-label="声音生成类型">
              <button role="tab" aria-selected={activeTab === "tts"} className={activeTab === "tts" ? "active" : ""} type="button" onClick={() => setActiveTab("tts")}>TTS配音</button>
              <button role="tab" aria-selected={activeTab === "subtitle"} className={activeTab === "subtitle" ? "active" : ""} type="button" onClick={() => setActiveTab("subtitle")}>字幕</button>
            </div>
          </div>
        </div>
        <button
          className="primaryAction soundNewTaskButton"
          type="button"
          onClick={() => {
            setActiveTab("tts");
            globalThis.requestAnimationFrame(() => ttsTextRef.current?.focus());
          }}
        >
          新建 TTS 任务
        </button>
      </header>

      <div className="soundWorkspaceGrid" aria-label="声音与字幕三栏工作区">
        <aside className="soundTaskPanel" aria-label="配音任务列表">
          <header className="soundTaskPanelHeader">
            <div>
              <h3>配音任务</h3>
              <p>{tasks.length} 个任务 · {activeTasks.length} 个运行中</p>
            </div>
            <button type="button" disabled={tasksLoading} onClick={() => void refreshTasks()}>{tasksLoading ? "刷新中" : "刷新"}</button>
          </header>

          <div className="soundTaskFilters" role="group" aria-label="任务筛选">
            <button className={taskFilter === "all" ? "active" : ""} aria-pressed={taskFilter === "all"} type="button" onClick={() => setTaskFilter("all")}>全部 {tasks.length}</button>
            <button className={taskFilter === "active" ? "active" : ""} aria-pressed={taskFilter === "active"} type="button" onClick={() => setTaskFilter("active")}>运行中 {activeTasks.length}</button>
            <button className={taskFilter === "failed" ? "active" : ""} aria-pressed={taskFilter === "failed"} type="button" onClick={() => setTaskFilter("failed")}>失败 {failedTasks.length}</button>
          </div>

          <div className="soundTaskCards" aria-live="polite">
            {tasksLoading && !tasks.length ? <p className="soundTaskEmpty">正在加载任务</p> : null}
            {!tasksLoading && !visibleTasks.length ? <p className="soundTaskEmpty">当前筛选下暂无任务</p> : null}
            {visibleTasks.map((task) => (
              <article className={`soundTaskCard ${task.status}`} key={task.task_id}>
                <strong>{taskDisplayTitle(task)}</strong>
                <p>{taskModelVoiceText(task)}</p>
                <div className="soundTaskCardFooter">
                  <span className={`soundTaskStatus ${task.status}`}>{taskStatusLabel(task.status)}</span>
                  <small>{taskCardMetric(task)}</small>
                </div>
                {task.cleanup_error_summary && <small className="soundTaskCleanup">暂存清理待重试</small>}
                {task.error_summary && <small className="soundTaskFailure">{task.error_summary}</small>}
                {task.status === "succeeded" && (
                  <div className="soundTaskResultLinks">
                    {stringValue(task.result?.audio_file_url) && <a href={stringValue(task.result?.audio_file_url)} target="_blank" rel="noreferrer">音频</a>}
                    {stringValue(task.result?.subtitle_file_url) && <a href={stringValue(task.result?.subtitle_file_url)} download>字幕</a>}
                  </div>
                )}
                {(task.status === "failed" || task.status === "queued" || task.status === "running") && (
                  <div className="soundTaskActions">
                    {task.status === "failed" && <button type="button" onClick={() => void prepareRetry(task)}>重试</button>}
                    {(task.status === "queued" || task.status === "running") && <button type="button" onClick={() => void handleCancelTask(task.task_id)}>取消</button>}
                  </div>
                )}
              </article>
            ))}
          </div>

          <div className="soundConcurrencyStatus">
            <strong>项目并发 {activeTasks.length} / 2</strong>
            <span>临时错误自动重试最多 1 次</span>
            <div aria-hidden="true"><i style={{ width: `${Math.min(activeTasks.length / 2, 1) * 100}%` }} /></div>
          </div>
        </aside>

        <section className={`soundEditorPanel${activeTab === "subtitle" ? " subtitleMode" : ""}`} aria-label={activeTab === "tts" ? "TTS 配音配置" : "字幕生成配置"}>
          <header className="soundEditorHeader">
            <h3>{activeTab === "tts" ? "生成 TTS 配音" : "生成字幕"}</h3>
            <p>{projectName || "当前账号"} · {Array.from(ttsText).length} 字</p>
          </header>

          {activeTab === "subtitle" && (
            <div className="subtitleSourceSwitch" role="group" aria-label="字幕时间轴来源">
              <button type="button" disabled={ttsTimestampUnavailable} className={subtitleSource === "tts" ? "active" : ""} onClick={() => setSubtitleSource("tts")}>TTS 字词时间戳</button>
              <button type="button" className={subtitleSource === "asr" ? "active" : ""} onClick={() => setSubtitleSource("asr")}>已有音频 ASR</button>
            </div>
          )}

          {activeTab === "tts" || subtitleSource === "tts" ? (
            <div className="soundTtsLayout">
              <div className="soundModelCatalogRow">
                <TtsModelSelect
                  models={ttsModels}
                  value={ttsModelId}
                  disabled={loadingModels}
                  onChange={setTtsModelId}
                />
                <div className={`soundCatalogStatus${catalogError ? " failed" : ""}`}>
                  <div>
                    <strong>{catalogLoading ? "正在读取声音目录" : `声音目录 · ${availableVoices.length} 个音色`}</strong>
                    <span>{catalogMessage || catalogError || (catalog?.last_sync?.completed_at ? `${formatRelativeDate(catalog.last_sync.completed_at)}同步` : "尚无成功同步记录")}</span>
                  </div>
                  <button type="button" disabled={!ttsModelId || checkingCatalog} onClick={() => void checkCatalog()}>{checkingCatalog ? "检查中" : "检查更新"}</button>
                </div>
              </div>

              <div className="soundFormGrid">
                <div className="soundVoiceField">
                  <SharedVoiceCatalogSelect
                    voices={availableVoices}
                    selectedVoice={selectedVoice}
                    selectedVoiceType={voiceType}
                    selectedVoiceLabel={voiceLabels[voiceType] || voiceType}
                    invalid={voiceInvalid}
                    disabled={catalogLoading || !ttsModelId}
                    onChange={handleVoiceChange}
                  />
                </div>
                <LanguageCatalogSelect
                  options={languageOptions}
                  value={language}
                  invalid={languageInvalid}
                  disabled={!selectedVoice || voiceInvalid}
                  onChange={setLanguage}
                />
              </div>

              <div className="soundFeedbackStack">
                {catalogMessage && <span className="soundA11yStatus" role="status">{catalogMessage}</span>}
                {catalogError && <span className="soundA11yStatus" role="alert">{catalogError}</span>}
                {voiceInvalid && <p className="soundInlineError" role="alert">原音色在当前模型中不可用，请重新选择后生成。</p>}
                {(languageInvalid || parameterInvalid || audioFormatInvalid || sampleRateInvalid) && !voiceInvalid && <p className="soundInlineError" role="alert">原语言或参数在当前模型中已失效，请重新确认。</p>}
                {modelError && <p className="soundInlineError" role="alert">{modelError}</p>}
                {activeTab === "subtitle" && subtitleSource === "tts" && ttsTimestampUnavailable && (
                  <p className="soundInlineError" role="alert">当前 TTS 中转模型不返回可信字词时间戳，请使用已有音频 ASR。</p>
                )}
                {taskError && <p className="soundInlineError" role="alert">{taskError}</p>}
              </div>

              <div className="soundNarrationField">
                <div className="soundNarrationHeader">
                  <div className="soundNarrationTitle">
                    <strong>旁白文本</strong>
                    {importedScriptSource && <small title={importedScriptSource.scriptTitle}>来源：{importedScriptSource.scriptTitle}</small>}
                  </div>
                  <div className="soundNarrationActions">
                    <button type="button" disabled={writesDisabled || !projectId} onClick={() => void openScriptImport()}>导入脚本</button>
                    <span>{Array.from(ttsText).length} / {maxInputCharacters} 字</span>
                  </div>
                </div>
                <textarea ref={ttsTextRef} aria-label="配音文本" rows={activeTab === "tts" ? 7 : 5} maxLength={maxInputCharacters} value={ttsText} onChange={(event) => setTtsText(event.target.value)} />
              </div>

              {activeTab === "subtitle" && (
                <label className="soundSubtitleSegmentsField">字幕断句
                  <textarea aria-label="字幕断句" rows={4} placeholder="每行一个字幕片段" value={subtitleSegments} onChange={(event) => setSubtitleSegments(event.target.value)} />
                </label>
              )}

              <section className="soundParameterSection" aria-label="声音参数">
                <h4>声音参数</h4>
                <div className="speechParameterGrid">
                  {Object.entries(parameterDefinitions).map(([name, definition]) => {
                    if (definition.type !== "number") return null;
                    const minimum = numericValue(definition.minimum ?? definition.min) ?? 0;
                    const maximum = numericValue(definition.maximum ?? definition.max) ?? 2;
                    const value = typeof speechParameters[name] === "number"
                      ? speechParameters[name] as number
                      : speechParameterDefault(name, definition, minimum, maximum);
                    return (
                      <label key={name}>{parameterLabel(name)} <output>{value.toFixed(1)}</output>
                        <input aria-label={parameterLabel(name)} type="range" min={minimum} max={maximum} step={numericValue(definition.step) ?? 0.1} value={value} onChange={(event) => setSpeechParameters((current) => ({ ...current, [name]: Number(event.target.value) }))} />
                      </label>
                    );
                  })}
                  {!Object.keys(parameterDefinitions).length && <p>当前模型无可调参数</p>}
                </div>
              </section>

              <div className="soundPreviewPlayer" aria-label="试听音频">
                <div>
                  <strong>试听音频</strong>
                  <span>{latestPlayableTask ? taskModelVoiceText(latestPlayableTask) : "尚无试听"}</span>
                </div>
                {latestPlayableTask
                  ? <audio aria-label="生成音频" controls preload="none" src={stringValue(latestPlayableTask.result?.audio_file_url)} />
                  : <button type="button" aria-label="播放试听" disabled>▶</button>}
              </div>

              <div className="soundPrimaryActions">
                {activeTab === "tts" && <button type="button" disabled={generationDisabled} onClick={() => void prepareTask(currentTtsIntent("tts_preview", false))}>试听</button>}
                <button className="primaryAction" type="button" disabled={generationDisabled} onClick={() => void prepareTask(currentTtsIntent("tts", activeTab === "subtitle"))}>{preflighting ? "正在预检" : activeTab === "tts" ? "生成配音" : "生成配音与字幕"}</button>
              </div>

              <SoundCurrentTask task={latestTask} />
            </div>
          ) : (
            <>
              <div className="soundAsrForm">
                <label className="soundCompactField"><span>ASR 模型</span>
                  <select aria-label="ASR 模型" disabled={loadingModels || !asrModels.length} value={asrModelId} onChange={(event) => setAsrModelId(event.target.value)}>
                    {!asrModels.length && <option value="">暂无启用模型</option>}
                    {asrModels.map((model) => <option value={model.model_id} key={model.model_id}>{model.display_name}</option>)}
                  </select>
                </label>
                <label className="soundCompactField"><span>已有音频素材</span>
                  <select aria-label="已有音频素材" value={sourceAudioId} onChange={(event) => { setSourceAudioId(event.target.value); setInspection(null); setInspectionError(""); }}>
                    <option value="">请选择音频</option>
                    {audioMaterials.map((material) => <option value={material.material_id} key={material.material_id}>{material.file_name}</option>)}
                  </select>
                </label>
              </div>
              <div className="audioInspectionPanel">
                <div>
                  <strong>{selectedAudio?.file_name || "尚未选择音频"}</strong>
                  <span>{inspectionStatusText(inspection)}</span>
                </div>
                <button type="button" disabled={!sourceAudioId || inspectionLoading} onClick={() => void inspectAudio()}>{inspectionLoading ? "检查中" : inspection ? "重新检查" : "检查音频"}</button>
              </div>
              {inspection?.status === "succeeded" && (
                <dl className="inspectionFacts">
                  <div><dt>真实时长</dt><dd>{formatDuration(inspection.duration_ms)}</dd></div>
                  <div><dt>文件大小</dt><dd>{formatBytes(inspection.file_size_bytes)}</dd></div>
                  <div><dt>媒体格式</dt><dd>{inspection.container_format || "-"}</dd></div>
                  <div><dt>音频编码</dt><dd>{inspection.audio_codec || "-"}</dd></div>
                </dl>
              )}
              {inspectionError && <p className="soundInlineError" role="alert">{inspectionError}</p>}
              <div className="soundPrimaryActions">
                <button className="primaryAction" type="button" disabled={writesDisabled || preflighting || !asrModelId || inspection?.status !== "succeeded"} onClick={() => void prepareAsrTask()}>{preflighting ? "正在预检" : "生成字幕"}</button>
              </div>
              <SoundCurrentTask task={latestTask} />
            </>
          )}
        </section>

        <aside className="soundAgentPanel" aria-label="声音 Agent">
          <header className="soundAgentHeader">
            <div className="soundAgentTitleRow"><strong>声音 Agent</strong><span>在线</span></div>
            <div className="soundAgentSessionRow">
              <span>{conversationId ? `会话 #${conversationId.slice(0, 8)}` : "新会话"} · {Math.ceil(agentMessages.length / 2)} 轮</span>
              <select aria-label="声音 Agent 模型" disabled={!textModels.length} value={agentModelId} onChange={(event) => setAgentModelId(event.target.value)}>
                {!textModels.length && <option value="">暂无推理模型</option>}
                {textModels.map((model) => <option value={model.model_id} key={model.model_id}>{model.display_name}</option>)}
              </select>
            </div>
          </header>
          <div className="soundAgentMessages" aria-live="polite">
            {!agentMessages.length && <p className="soundAgentEmpty">等待声音建议</p>}
            {agentMessages.map((message, index) => (
              <div className={`soundAgentMessage ${message.role}`} key={message.message_id || `${message.role}-${index}`}>
                <small>{message.role === "assistant" ? "声音 Agent" : "你"}</small>
                <p>{message.content}</p>
              </div>
            ))}
            {recommendation && <button className="applySuggestionButton" type="button" onClick={applyRecommendation}>应用建议</button>}
            {agentMessages.some((message) => message.metadata.tool_execution === true) && (
              <div className="soundAgentToolStep">
                <strong>工具步骤 · 声音建议</strong>
                <span>模型与声音快照已记录</span>
                <b>成功</b>
              </div>
            )}
            {agentError && <p className="soundInlineError" role="alert">{agentError}</p>}
          </div>
          <div className="soundAgentComposer">
            <textarea aria-label="声音 Agent 输入" rows={3} placeholder="调整语气、节奏或字幕断句" value={agentDraft} onChange={(event) => setAgentDraft(event.target.value)} />
            <button className="primaryAction" type="button" aria-label="发送建议" disabled={writesDisabled || sendingAgent || !agentDraft.trim() || !ttsModelId || !agentModelId || !catalog || voiceInvalid} onClick={() => void sendAgentSuggestion()}>{sendingAgent ? "发送中" : "发送"}</button>
          </div>
          <div className="soundAgentRunStatus">
            <div><strong>{conversationId ? `本轮运行 #${conversationId.slice(0, 8)}` : "本轮尚未运行"}</strong><span>{conversationId ? `${agentMessages.length} 条消息 · 审计已记录` : "等待运行"}</span></div>
            <b>{sendingAgent ? "运行中" : conversationId ? "已完成" : "待命"}</b>
          </div>
        </aside>
      </div>

      {scriptImportOpen && (
        <div className="soundModalBackdrop">
          <section className="soundScriptImportDialog" role="dialog" aria-modal="true" aria-label="从脚本创作导入旁白">
            <header>
              <div>
                <p className="sectionKicker">脚本创作</p>
                <h3>导入已有脚本旁白</h3>
              </div>
              <button type="button" aria-label="关闭导入脚本" onClick={() => setScriptImportOpen(false)}>×</button>
            </header>

            <div className="soundScriptImportBody">
              <aside className="soundScriptImportList" aria-label="可导入脚本">
                <div className="soundScriptImportSearch">
                  <input
                    type="search"
                    aria-label="搜索脚本"
                    placeholder="搜索标题或来源选题"
                    value={importScriptQuery}
                    onChange={(event) => setImportScriptQuery(event.target.value)}
                  />
                </div>
                <div className="soundScriptImportFilters" role="group" aria-label="脚本状态筛选">
                  {(["all", "draft", "approved"] as const).map((status) => (
                    <button
                      type="button"
                      key={status}
                      className={importScriptStatus === status ? "active" : ""}
                      aria-pressed={importScriptStatus === status}
                      onClick={() => setImportScriptStatus(status)}
                    >
                      {status === "all" ? "全部" : status === "draft" ? "草稿" : "已通过"}
                    </button>
                  ))}
                </div>
                <div className="soundScriptImportItems">
                  {importScriptsLoading && <p>正在读取脚本</p>}
                  {!importScriptsLoading && !visibleImportScripts.length && <p>当前筛选下暂无脚本</p>}
                  {visibleImportScripts.map((script) => (
                    <button
                      type="button"
                      className={selectedImportScriptId === script.script_id ? "active" : ""}
                      aria-pressed={selectedImportScriptId === script.script_id}
                      key={script.script_id}
                      onClick={() => setSelectedImportScriptId(script.script_id)}
                    >
                      <strong>{script.title}</strong>
                      <span>{script.source_topic_title || "未关联来源选题"}</span>
                      <small>{script.status === "draft" ? "草稿" : "已通过"} · {formatRelativeDate(script.updated_at)}更新</small>
                    </button>
                  ))}
                </div>
              </aside>

              <section className="soundScriptScenePicker" aria-label="脚本分镜旁白">
                <header>
                  <div>
                    <strong>选择分镜旁白</strong>
                    <span>{selectedImportScript ? `已选择《${selectedImportScript.title}》` : "请选择左侧脚本"}</span>
                  </div>
                  <small>{selectedImportSceneIds.length} / {selectableImportScenes.length} 个分镜</small>
                </header>
                <div className="soundScriptSceneItems">
                  {importScriptLoading && <p>正在读取分镜</p>}
                  {!importScriptLoading && selectedImportScript && !selectableImportScenes.length && <p>该脚本没有可导入的非空旁白</p>}
                  {!importScriptLoading && !selectedImportScript && !importScriptError && <p>选择脚本后查看分镜旁白</p>}
                  {selectableImportScenes.map((scene) => (
                    <label className={selectedImportSceneIds.includes(scene.scene_id) ? "selected" : ""} key={scene.scene_id}>
                      <input
                        type="checkbox"
                        aria-label={`镜头 ${String(scene.sequence).padStart(2, "0")}`}
                        checked={selectedImportSceneIds.includes(scene.scene_id)}
                        onChange={() => toggleImportScene(scene.scene_id)}
                      />
                      <span>镜头 {String(scene.sequence).padStart(2, "0")}</span>
                      <p>{scene.narration}</p>
                    </label>
                  ))}
                </div>
              </section>
            </div>

            <footer>
              <div className="soundScriptImportSummary">
                {ttsText.trim()
                  ? <span>当前旁白已有 {Array.from(ttsText).length} 字，导入后将完整替换。</span>
                  : <span>已选 {selectedImportSceneIds.length} 个分镜，共 {selectedImportCharacterCount} 字。</span>}
                {importScriptError && <strong role="alert">{importScriptError}</strong>}
                {!importScriptError && importExceedsLimit && <strong role="alert">已选旁白 {selectedImportCharacterCount} 字，超过当前模型 {maxInputCharacters} 字上限。</strong>}
              </div>
              <div className="soundScriptImportFooterActions">
                <button type="button" onClick={() => setScriptImportOpen(false)}>取消</button>
                <button
                  className="primaryAction"
                  type="button"
                  disabled={importScriptsLoading || importScriptLoading || !selectedImportScenes.length || importExceedsLimit || Boolean(importScriptError)}
                  onClick={confirmScriptImport}
                >
                  {ttsText.trim() ? "替换并导入" : "导入旁白"}
                </button>
              </div>
            </footer>
          </section>
        </div>
      )}

      {pendingConfirmation && (
        <div className="soundModalBackdrop">
          <section className="soundConfirmDialog" role="dialog" aria-label="确认声音任务">
            <header><div><p className="sectionKicker">资源确认</p><h3>{pendingConfirmation.retryTaskId ? "确认重试" : taskIntentTitle(pendingConfirmation.intent)}</h3></div><button type="button" onClick={() => setPendingConfirmation(null)}>关闭</button></header>
            <dl>
              <div><dt>模型</dt><dd>{pendingConfirmation.preflight.model_display_name}</dd></div>
              {numberValue(pendingConfirmation.preflight.resource_usage.character_count) !== null && <div><dt>文本</dt><dd>{numberValue(pendingConfirmation.preflight.resource_usage.character_count)} 字符</dd></div>}
              {numberValue(pendingConfirmation.preflight.resource_usage.audio_duration_ms) !== null && <div><dt>音频时长</dt><dd>{formatDuration(numberValue(pendingConfirmation.preflight.resource_usage.audio_duration_ms))}</dd></div>}
              <div><dt>执行任务</dt><dd>{numberValue(pendingConfirmation.preflight.resource_usage.task_count) ?? 1} 个任务</dd></div>
              <div><dt>输出产物</dt><dd>{numberValue(pendingConfirmation.preflight.resource_usage.output_count) ?? 1} 个文件</dd></div>
            </dl>
            <footer><button type="button" onClick={() => setPendingConfirmation(null)}>取消</button><button className="primaryAction" type="button" disabled={submittingTask} onClick={() => void confirmTask()}>{submittingTask ? "提交中" : pendingConfirmation.retryTaskId ? "确认重试" : "确认生成"}</button></footer>
          </section>
        </div>
      )}
    </div>
  );
}

function initialSpeechParameters(catalog: VoiceCatalog, current: Record<string, unknown>) {
  const next = { ...current };
  if (!("audio_format" in next) && typeof catalog.model_settings.default_audio_format === "string") {
    next.audio_format = catalog.model_settings.default_audio_format;
  }
  if (!("sample_rate" in next) && typeof catalog.model_settings.default_sample_rate === "number") {
    next.sample_rate = catalog.model_settings.default_sample_rate;
  }
  const definitions = catalog.model_settings.parameters;
  if (definitions && typeof definitions === "object" && !Array.isArray(definitions)) {
    for (const [name, rawDefinition] of Object.entries(definitions)) {
      if (name in next || !rawDefinition || typeof rawDefinition !== "object" || Array.isArray(rawDefinition)) continue;
      const definition = rawDefinition as Record<string, unknown>;
      const minimum = numericValue(definition.minimum ?? definition.min) ?? 0;
      const maximum = numericValue(definition.maximum ?? definition.max) ?? 2;
      next[name] = speechParameterDefault(name, definition, minimum, maximum);
    }
  }
  return next;
}

function parameterLabel(value: string) {
  const labels: Record<string, string> = {
    speed_ratio: "语速",
    volume_ratio: "音量",
    pitch_ratio: "音调",
  };
  return labels[value] || value;
}

function taskIntentTitle(intent: SoundTaskIntent) {
  if (intent.task_type === "tts_preview") return "确认试听";
  if (intent.task_type === "asr") return "确认 ASR 字幕";
  return intent.generate_subtitle ? "确认配音与字幕" : "确认配音";
}

function taskTypeLabel(task: SoundTask) {
  if (task.task_type === "tts_preview") return "TTS 试听";
  if (task.task_type === "asr") return "ASR 字幕";
  return task.generate_subtitle ? "配音与字幕" : "TTS 配音";
}

function taskDisplayTitle(task: SoundTask) {
  const text = (task.text_content ?? "").replace(/\s+/g, " ").trim();
  if (task.task_type === "asr") return "已有音频字幕";
  if (!text) return taskTypeLabel(task);
  const prefix = task.task_type === "tts_preview" ? "试听 · " : "";
  return `${prefix}${Array.from(text).slice(0, 15).join("")}${Array.from(text).length > 15 ? "..." : ""}`;
}

function taskModelVoiceText(task: SoundTask) {
  const model = snapshotLabel(task.model_snapshot, "display_name") || "语音模型";
  const voice = snapshotLabel(task.voice_snapshot, "name") || task.voice_type;
  return voice ? `${model} · ${voice}` : model;
}

function taskCardMetric(task: SoundTask) {
  if (task.status === "queued" || task.status === "running") return resourceUsageText(task.resource_usage ?? {});
  return new Date(task.created_at).toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit", hour12: false });
}

function SoundCurrentTask({ task }: { task: SoundTask | null }) {
  if (task?.status === "failed") {
    const details = task.error_details ?? {};
    const httpStatus = typeof details.http_status === "number" ? `HTTP ${details.http_status}` : "-";
    const providerMessage = stringValue(details.provider_error_message) || "-";
    const protocol = snapshotLabel(task.model_snapshot, "api_protocol") || "-";
    return (
      <section className="soundCurrentTask failed" aria-label="当前失败任务详情">
        <header className="soundFailureHeader">
          <strong>当前任务</strong>
          <b>失败</b>
        </header>
        <p className="soundFailureMessage">{providerMessage}</p>
        <dl className="soundFailureFacts">
          <div><dt>HTTP 状态</dt><dd>{httpStatus}</dd></div>
          <div><dt>内部错误码</dt><dd>{task.error_code || "-"}</dd></div>
          <div><dt>供应商错误码</dt><dd>{stringValue(details.provider_error_code) || "-"}</dd></div>
          <div><dt>模型协议</dt><dd>{protocol}</dd></div>
          <div><dt>尝试次数</dt><dd>{task.attempt_count} / {task.max_attempts}</dd></div>
          <div><dt>完成时间</dt><dd>{formatTaskCompletedAt(task.completed_at)}</dd></div>
          <div className="soundFailureTrace"><dt>request_id</dt><dd>{task.request_id || "-"}</dd></div>
          <div className="soundFailureTrace"><dt>X-Tt-Logid</dt><dd>{task.upstream_log_id || "-"}</dd></div>
        </dl>
      </section>
    );
  }
  return (
    <div className={`soundCurrentTask ${task?.status ?? "idle"}`}>
      <div>
        <strong>当前任务</strong>
        <span>{task ? `${taskStatusLabel(task.status)} · request_id ${task.request_id || "-"}` : "当前没有已提交任务"}</span>
      </div>
      <b>{task ? taskStatusLabel(task.status) : "待提交"}</b>
    </div>
  );
}

function formatTaskCompletedAt(value: string | null) {
  if (!value) return "-";
  const timestamp = new Date(value);
  if (Number.isNaN(timestamp.getTime())) return "-";
  return timestamp.toLocaleString("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  });
}

function taskStatusLabel(status: SoundTask["status"]) {
  return { queued: "排队中", running: "执行中", succeeded: "已完成", failed: "失败", cancelled: "已取消" }[status];
}

function resourceUsageText(usage: Record<string, unknown>) {
  const characterCount = numberValue(usage.character_count);
  if (characterCount !== null) return `${characterCount} 字符`;
  const duration = numberValue(usage.audio_duration_ms);
  if (duration !== null) return formatDuration(duration);
  return `${numberValue(usage.task_count) ?? 1} 个任务`;
}

function inspectionStatusText(inspection: AudioInspection | null) {
  if (!inspection) return "需要先读取真实时长与格式";
  return { queued: "等待检查", running: "正在检查", succeeded: "检查完成", failed: inspection.error_summary || "检查失败" }[inspection.status];
}

function snapshotLabel(snapshot: Record<string, unknown> | null, key: string) {
  return snapshot ? stringValue(snapshot[key]) : "";
}

function stringValue(value: unknown) {
  return typeof value === "string" ? value : "";
}

function numberValue(value: unknown) {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function numericValue(value: unknown) {
  return numberValue(value);
}

function midpoint(minimum: number, maximum: number) {
  return Math.round(((minimum + maximum) / 2) * 10) / 10;
}

function speechParameterDefault(
  name: string,
  definition: Record<string, unknown>,
  minimum: number,
  maximum: number,
) {
  const configuredDefault = numericValue(definition.default);
  if (configuredDefault !== null) return configuredDefault;
  if (name === "speed_ratio") return Math.min(maximum, Math.max(minimum, 1));
  return midpoint(minimum, maximum);
}

function formatDuration(milliseconds: number | null) {
  if (milliseconds === null) return "-";
  const seconds = milliseconds / 1000;
  return `${seconds.toFixed(seconds >= 10 ? 1 : 2)} 秒`;
}

function formatBytes(bytes: number | null) {
  if (bytes === null) return "-";
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

function formatRelativeDate(value: string) {
  const milliseconds = Date.now() - new Date(value).getTime();
  if (!Number.isFinite(milliseconds) || milliseconds < 0) return "刚刚";
  const minutes = Math.floor(milliseconds / 60_000);
  if (minutes < 1) return "刚刚";
  if (minutes < 60) return `${minutes} 分钟前`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} 小时前`;
  return `${Math.floor(hours / 24)} 天前`;
}

function newIdempotencyKey(prefix: string) {
  const randomId = globalThis.crypto?.randomUUID?.()
    ?? `${Date.now()}-${Math.random().toString(16).slice(2)}`;
  return `${prefix}-${randomId}`;
}

function errorMessage(error: unknown) {
  if (error instanceof ApiError || error instanceof Error) return error.message;
  return "请求失败";
}
