"use client";

import { FormEvent, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { AdminShell } from "../components/AdminShell";
import {
  AiModel,
  AiModelPayload,
  AiModelProtocol,
  AiModelStatus,
  AiModelType,
  ApiClient,
  ApiError,
  TosStagingToolConfig,
  VoiceCatalog,
  changeAiModelStatus,
  createAiModel,
  createApiClient,
  deleteAiModel,
  listAiModels,
  getVoiceCatalog,
  getTosStagingTool,
  requestAdminVoiceCatalogSync,
  setDefaultAiModel,
  updateAiModel,
} from "../lib/api";

const typeLabels: Record<AiModelType, string> = {
  text: "文本模型",
  image: "图片模型",
  video: "视频模型",
  speech: "语音模型",
};
const protocolOptions: Record<AiModelType, { value: AiModelProtocol; label: string }[]> = {
  text: [
    { value: "openai_responses", label: "OpenAI Responses" },
    { value: "openai_chat_completions", label: "OpenAI Chat Completions" },
  ],
  image: [
    { value: "openai_images", label: "OpenAI Images" },
    { value: "volcengine_ark_images", label: "火山方舟图片生成" },
  ],
  video: [
    { value: "runway_api", label: "Runway API" },
    { value: "kling_api", label: "可灵 API" },
  ],
  speech: [
    { value: "volcengine_tts_v3", label: "豆包 TTS V3" },
    { value: "openai_audio_speech", label: "OpenAI Audio Speech（中转）" },
    { value: "volcengine_asr_v3", label: "豆包 ASR V3" },
  ],
};
const ttsTimestampLanguageOptions = [
  { value: "zh-cn", label: "简体中文" },
  { value: "en-us", label: "美式英语" },
] as const;

type FormState = AiModelPayload & { version?: number };
type ConfirmState = { kind: "disable" | "delete"; model: AiModel } | null;

function speechSettings(protocol: AiModelProtocol): Record<string, unknown> {
  const isNativeTts = protocol === "volcengine_tts_v3";
  const isOpenAiTts = protocol === "openai_audio_speech";
  const isTts = isNativeTts || isOpenAiTts;
  return {
    resource_id: isTts ? "seed-tts-2.0" : "volc.seedasr.auc",
    supported_audio_formats: ["mp3", "wav"],
    default_audio_format: "mp3",
    supported_sample_rates: [24000],
    default_sample_rate: 24000,
    max_input_characters: isTts ? 3000 : null,
    max_audio_duration_seconds: isTts ? null : 7200,
    supports_word_timestamps: !isOpenAiTts,
    word_timestamp_languages: isNativeTts ? ["zh-cn", "en-us"] : isOpenAiTts ? [] : ["*"],
    catalog_sync_interval_minutes: isNativeTts ? 1440 : null,
    parameters: isTts
      ? { speed_ratio: { type: "number", minimum: isOpenAiTts ? 0.25 : 0.5, maximum: isOpenAiTts ? 4 : 2 } }
      : {},
  };
}

function getTtsTimestampLanguages(settings: Record<string, unknown>): string[] {
  const configured = Array.isArray(settings.word_timestamp_languages)
    ? settings.word_timestamp_languages.filter((value): value is string => typeof value === "string")
    : [];
  return ttsTimestampLanguageOptions
    .map((option) => option.value)
    .filter((value) => configured.includes(value));
}

function emptyForm(modelType: AiModelType): FormState {
  const protocol = protocolOptions[modelType][0].value;
  const isSpeech = modelType === "speech";
  return {
    display_name: "",
    model_type: modelType,
    provider_name: isSpeech ? "火山引擎" : "",
    api_protocol: protocol,
    protocol_version: isSpeech ? "v3" : "v1",
    auth_scheme: isSpeech ? "api_key" : protocol === "kling_api" ? "access_key_secret" : "bearer",
    request_base_url: isSpeech ? "https://openspeech.bytedance.com/api/v3" : "",
    upstream_model: protocol === "volcengine_tts_v3" ? "doubao-seed-tts-2.0" : "",
    api_key: "",
    api_secret: null,
    catalog_access_key: null,
    catalog_secret_key: null,
    voice_catalog_mode: "official_sync",
    voice_catalog_source_model_id: null,
    timeout_seconds: 120,
    reasoning_effort: modelType === "text" ? "low" : null,
    max_output_tokens: modelType === "text" ? 3000 : null,
    settings: modelType === "image"
      ? { supported_sizes: ["1024x1024"], default_size: "1024x1024", max_images_per_request: 4 }
      : isSpeech ? speechSettings(protocol) : {},
    sort_order: 0,
    remark: "",
    is_default: false,
  };
}

function formFromModel(model: AiModel): FormState {
  return {
    display_name: model.display_name,
    model_type: model.model_type,
    provider_name: model.provider_name,
    api_protocol: model.api_protocol,
    protocol_version: model.protocol_version,
    auth_scheme: model.auth_scheme,
    request_base_url: model.request_base_url,
    upstream_model: model.upstream_model,
    api_key: "",
    api_secret: "",
    catalog_access_key: "",
    catalog_secret_key: "",
    voice_catalog_mode: model.voice_catalog_mode,
    voice_catalog_source_model_id: model.voice_catalog_source_model_id,
    timeout_seconds: model.timeout_seconds,
    reasoning_effort: model.reasoning_effort,
    max_output_tokens: model.max_output_tokens,
    settings: model.settings,
    sort_order: model.sort_order,
    remark: model.remark,
    is_default: model.is_default,
    version: model.version,
  };
}

function normalizedForm(form: FormState): FormState {
  if (form.model_type === "speech") {
    const isNativeTts = form.api_protocol === "volcengine_tts_v3";
    const isOpenAiTts = form.api_protocol === "openai_audio_speech";
    const isTts = isNativeTts || isOpenAiTts;
    return {
      ...form,
      auth_scheme: isOpenAiTts ? "bearer" : "api_key",
      api_secret: null,
      catalog_access_key: isNativeTts ? form.catalog_access_key || null : null,
      catalog_secret_key: isNativeTts ? form.catalog_secret_key || null : null,
      voice_catalog_mode: isOpenAiTts ? "shared" : isNativeTts ? form.voice_catalog_mode : "official_sync",
      voice_catalog_source_model_id: isTts && (isOpenAiTts || form.voice_catalog_mode === "shared")
        ? form.voice_catalog_source_model_id
        : null,
      reasoning_effort: null,
      max_output_tokens: null,
      settings: {
        ...form.settings,
        supports_word_timestamps: isOpenAiTts ? false : true,
        word_timestamp_languages: isNativeTts
          ? getTtsTimestampLanguages(form.settings)
          : isOpenAiTts ? [] : ["*"],
        catalog_sync_interval_minutes: isNativeTts
          ? form.settings.catalog_sync_interval_minutes
          : null,
      },
    };
  }
  if (form.model_type !== "image") {
    return {
      ...form,
      catalog_access_key: null,
      catalog_secret_key: null,
      voice_catalog_mode: "official_sync",
      voice_catalog_source_model_id: null,
    };
  }
  const defaultSize = String(form.settings.default_size ?? "").trim();
  const isArk = form.api_protocol === "volcengine_ark_images";
  return {
    ...form,
    auth_scheme: "bearer",
    api_secret: isArk ? null : form.api_secret,
    catalog_access_key: null,
    catalog_secret_key: null,
    voice_catalog_mode: "official_sync",
    voice_catalog_source_model_id: null,
    settings: {
      ...form.settings,
      supported_sizes: defaultSize ? [defaultSize] : [],
      default_size: defaultSize || null,
      max_images_per_request: isArk
        ? 1
        : Number(form.settings.max_images_per_request ?? 4),
    },
  };
}

export function ModelManagementPage({ client }: { client?: ApiClient }) {
  const apiClient = useMemo(() => client ?? createApiClient(), [client]);
  const [modelType, setModelType] = useState<AiModelType>("text");
  const [status, setStatus] = useState<AiModelStatus | "">("");
  const [provider, setProvider] = useState("");
  const [protocol, setProtocol] = useState<AiModelProtocol | "">("");
  const [search, setSearch] = useState("");
  const [models, setModels] = useState<AiModel[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [voiceCatalogSourceModels, setVoiceCatalogSourceModels] = useState<AiModel[]>([]);
  const [voiceCatalogSourceLoading, setVoiceCatalogSourceLoading] = useState(false);
  const [voiceCatalogSourceError, setVoiceCatalogSourceError] = useState("");
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [editing, setEditing] = useState<AiModel | null>(null);
  const [form, setForm] = useState<FormState>(() => emptyForm("text"));
  const [saving, setSaving] = useState(false);
  const [confirm, setConfirm] = useState<ConfirmState>(null);
  const [replacementId, setReplacementId] = useState("");
  const [catalogs, setCatalogs] = useState<Record<string, VoiceCatalog>>({});
  const [syncingModelId, setSyncingModelId] = useState("");
  const [catalogMessage, setCatalogMessage] = useState("");
  const [tosTool, setTosTool] = useState<TosStagingToolConfig | null>(null);
  const [tosToolLoading, setTosToolLoading] = useState(true);
  const [timestampLanguageOpen, setTimestampLanguageOpen] = useState(false);
  const [timestampLanguageSearch, setTimestampLanguageSearch] = useState("");
  const timestampLanguageDropdownRef = useRef<HTMLDivElement>(null);
  const timestampLanguageTriggerRef = useRef<HTMLButtonElement>(null);
  const voiceCatalogSourceRequestIdRef = useRef(0);
  const selectedTtsTimestampLanguages = getTtsTimestampLanguages(form.settings);
  const selectedTtsTimestampLanguageLabels = ttsTimestampLanguageOptions
    .filter((option) => selectedTtsTimestampLanguages.includes(option.value))
    .map((option) => option.label);
  const normalizedTimestampLanguageSearch = timestampLanguageSearch.trim().toLocaleLowerCase("zh-CN");
  const filteredTtsTimestampLanguageOptions = ttsTimestampLanguageOptions.filter((option) =>
    option.label.toLocaleLowerCase("zh-CN").includes(normalizedTimestampLanguageSearch),
  );
  const apiKeyFieldLabel = form.api_protocol === "volcengine_tts_v3"
    ? "TTS X-Api-Key"
    : form.api_protocol === "openai_audio_speech"
      ? "Bearer API Key"
    : form.api_protocol === "volcengine_asr_v3"
      ? "ASR X-Api-Key"
      : "API Key";
  const isNativeTtsForm = form.api_protocol === "volcengine_tts_v3";
  const isOpenAiTtsForm = form.api_protocol === "openai_audio_speech";
  const isTtsForm = isNativeTtsForm || isOpenAiTtsForm;
  const needsVoiceCatalogSource = isOpenAiTtsForm
    || (isNativeTtsForm && form.voice_catalog_mode === "shared");
  const ttsCatalogSourceModels = voiceCatalogSourceModels.filter((model) =>
    model.model_id !== editing?.model_id
      && model.model_type === "speech"
      && model.api_protocol === "volcengine_tts_v3"
      && model.status === "enabled"
      && model.deleted_at === null
      && model.voice_catalog_mode === "official_sync"
      && model.upstream_model.trim() === form.upstream_model.trim()
      && String(model.settings.resource_id ?? "").trim()
        === String(form.settings.resource_id ?? "").trim(),
  );

  useEffect(() => {
    if (!needsVoiceCatalogSource
      || form.voice_catalog_source_model_id
      || voiceCatalogSourceLoading
      || voiceCatalogSourceError
      || !ttsCatalogSourceModels.length) return;
    setForm((current) => ({
      ...current,
      voice_catalog_mode: "shared",
      voice_catalog_source_model_id: ttsCatalogSourceModels[0].model_id,
    }));
  }, [form.voice_catalog_source_model_id, needsVoiceCatalogSource, ttsCatalogSourceModels, voiceCatalogSourceError, voiceCatalogSourceLoading]);

  useEffect(() => {
    if (!timestampLanguageOpen) return;

    function handlePointerDown(event: PointerEvent) {
      if (!timestampLanguageDropdownRef.current?.contains(event.target as Node | null)) {
        setTimestampLanguageOpen(false);
        setTimestampLanguageSearch("");
      }
    }

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key !== "Escape") return;
      event.preventDefault();
      setTimestampLanguageOpen(false);
      setTimestampLanguageSearch("");
      timestampLanguageTriggerRef.current?.focus();
    }

    document.addEventListener("pointerdown", handlePointerDown);
    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("pointerdown", handlePointerDown);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [timestampLanguageOpen]);

  const loadModels = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      const response = await listAiModels(apiClient, {
        type: modelType,
        status: status || undefined,
        provider: provider || undefined,
        protocol: protocol || undefined,
        q: search || undefined,
      });
      setModels(response.models);
      const ttsModels = response.models.filter(
        (model) => ["volcengine_tts_v3", "openai_audio_speech"].includes(model.api_protocol)
          && model.status !== "deleted",
      );
      const catalogResults = await Promise.allSettled(
        ttsModels.map(async (model) => [model.model_id, await getVoiceCatalog(apiClient, model.model_id)] as const),
      );
      setCatalogs(Object.fromEntries(
        catalogResults
          .filter((result): result is PromiseFulfilledResult<readonly [string, VoiceCatalog]> => result.status === "fulfilled")
          .map((result) => result.value),
      ));
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "模型列表加载失败");
    } finally {
      setLoading(false);
    }
  }, [apiClient, modelType, status, provider, protocol, search]);

  useEffect(() => { void loadModels(); }, [loadModels]);

  const loadVoiceCatalogSourceModels = useCallback(async () => {
    const requestId = voiceCatalogSourceRequestIdRef.current + 1;
    voiceCatalogSourceRequestIdRef.current = requestId;
    setVoiceCatalogSourceLoading(true);
    setVoiceCatalogSourceError("");
    try {
      const response = await listAiModels(apiClient, {
        type: "speech",
        status: "enabled",
      });
      if (voiceCatalogSourceRequestIdRef.current !== requestId) return;
      setVoiceCatalogSourceModels(response.models);
    } catch {
      if (voiceCatalogSourceRequestIdRef.current !== requestId) return;
      setVoiceCatalogSourceError("无法读取全部启用的语音模型，请重试。");
    } finally {
      if (voiceCatalogSourceRequestIdRef.current === requestId) {
        setVoiceCatalogSourceLoading(false);
      }
    }
  }, [apiClient]);

  useEffect(() => {
    if (drawerOpen && needsVoiceCatalogSource) {
      void loadVoiceCatalogSourceModels();
      return;
    }
    voiceCatalogSourceRequestIdRef.current += 1;
    setVoiceCatalogSourceModels([]);
    setVoiceCatalogSourceLoading(false);
    setVoiceCatalogSourceError("");
  }, [drawerOpen, loadVoiceCatalogSourceModels, needsVoiceCatalogSource]);

  const loadTosTool = useCallback(async () => {
    setTosToolLoading(true);
    try {
      setTosTool(await getTosStagingTool(apiClient));
    } catch {
      setTosTool(null);
    } finally {
      setTosToolLoading(false);
    }
  }, [apiClient]);

  useEffect(() => { void loadTosTool(); }, [loadTosTool]);

  useEffect(() => {
    const activeModelIds = Object.entries(catalogs)
      .filter(([, catalog]) => catalog.last_sync && ["queued", "running"].includes(catalog.last_sync.status))
      .map(([modelId]) => modelId);
    if (!activeModelIds.length) return;
    const timer = globalThis.setInterval(() => {
      void Promise.allSettled(
        activeModelIds.map(async (modelId) => [modelId, await getVoiceCatalog(apiClient, modelId)] as const),
      ).then((results) => {
        setCatalogs((current) => ({
          ...current,
          ...Object.fromEntries(
            results
              .filter((result): result is PromiseFulfilledResult<readonly [string, VoiceCatalog]> => result.status === "fulfilled")
              .map((result) => result.value),
          ),
        }));
      });
    }, 3000);
    return () => globalThis.clearInterval(timer);
  }, [apiClient, catalogs]);

  function openCreate() {
    closeTimestampLanguageDropdown();
    setEditing(null);
    setForm(emptyForm(modelType));
    setDrawerOpen(true);
    setError("");
  }

  function openEdit(model: AiModel) {
    closeTimestampLanguageDropdown();
    setEditing(model);
    setForm(formFromModel(model));
    setDrawerOpen(true);
    setError("");
  }

  function changeType(nextType: AiModelType) {
    closeTimestampLanguageDropdown();
    const next = emptyForm(nextType);
    setForm((current) => ({ ...next, display_name: current.display_name, provider_name: current.provider_name }));
  }

  function changeProtocol(nextProtocol: AiModelProtocol) {
    closeTimestampLanguageDropdown();
    setForm((current) => ({
      ...current,
      api_protocol: nextProtocol,
      auth_scheme: current.model_type === "speech"
        ? nextProtocol === "openai_audio_speech" ? "bearer" : "api_key"
        : nextProtocol === "kling_api" ? "access_key_secret" : "bearer",
      api_secret: nextProtocol === "volcengine_ark_images" || current.model_type === "speech"
        ? null
        : current.api_secret,
      upstream_model: nextProtocol === "volcengine_tts_v3"
        ? "doubao-seed-tts-2.0"
        : nextProtocol === "openai_audio_speech"
          ? "doubao-seed-tts-2.0"
        : nextProtocol === "volcengine_asr_v3"
          ? "doubao-seed-asr-2.0"
          : current.upstream_model,
      catalog_access_key: nextProtocol === "volcengine_tts_v3" ? current.catalog_access_key : null,
      catalog_secret_key: nextProtocol === "volcengine_tts_v3" ? current.catalog_secret_key : null,
      voice_catalog_mode: nextProtocol === "openai_audio_speech" ? "shared" : "official_sync",
      voice_catalog_source_model_id: nextProtocol === "openai_audio_speech"
        ? ttsCatalogSourceModels[0]?.model_id ?? null
        : null,
      protocol_version: nextProtocol === "openai_audio_speech" ? "v1" : "v3",
      settings: current.model_type === "speech"
        ? speechSettings(nextProtocol)
        : current.model_type !== "image"
          ? current.settings
        : nextProtocol === "volcengine_ark_images"
          ? { supported_sizes: [], default_size: null, max_images_per_request: 1 }
          : { supported_sizes: ["1024x1024"], default_size: "1024x1024", max_images_per_request: 4 },
    }));
  }

  function updateSpeechSetting(name: string, value: unknown) {
    setForm((current) => ({
      ...current,
      settings: { ...current.settings, [name]: value },
    }));
  }

  function closeTimestampLanguageDropdown() {
    setTimestampLanguageOpen(false);
    setTimestampLanguageSearch("");
  }

  function closeDrawer() {
    closeTimestampLanguageDropdown();
    setDrawerOpen(false);
  }

  function toggleTtsTimestampLanguage(language: string, checked: boolean) {
    setForm((current) => {
      const selected = getTtsTimestampLanguages(current.settings);
      const next = ttsTimestampLanguageOptions
        .map((option) => option.value)
        .filter((value) => value === language ? checked : selected.includes(value));
      if (next.length === 0) return current;
      return {
        ...current,
        settings: { ...current.settings, word_timestamp_languages: next },
      };
    });
  }

  function updateSpeedConstraint(name: "minimum" | "maximum", value: number) {
    setForm((current) => {
      const parameters = (current.settings.parameters ?? {}) as Record<string, unknown>;
      const speedRatio = (parameters.speed_ratio ?? {}) as Record<string, unknown>;
      return {
        ...current,
        settings: {
          ...current.settings,
          parameters: {
            ...parameters,
            speed_ratio: { ...speedRatio, type: "number", [name]: value },
          },
        },
      };
    });
  }

  async function submitForm(event: FormEvent) {
    event.preventDefault();
    setSaving(true);
    setError("");
    try {
      const payload = normalizedForm(form);
      if (editing) {
        await updateAiModel(apiClient, editing.model_id, { ...payload, version: editing.version });
      } else {
        await createAiModel(apiClient, payload);
      }
      closeDrawer();
      await loadModels();
    } catch (reason) {
      handleMutationError(reason);
    } finally {
      setSaving(false);
    }
  }

  function handleMutationError(reason: unknown) {
    const code = reason instanceof ApiError
      && reason.details && typeof reason.details === "object"
      && "error" in reason.details
      ? (reason.details as { error?: { code?: string } }).error?.code
      : undefined;
    if (code === "model_version_conflict") {
      setError("模型已被其他操作更新，列表已刷新，请重新编辑");
      void loadModels();
      return;
    }
    setError(reason instanceof Error ? reason.message : "操作失败");
  }

  async function makeDefault(model: AiModel) {
    try {
      await setDefaultAiModel(apiClient, model.model_id, { version: model.version });
      await loadModels();
    } catch (reason) { handleMutationError(reason); }
  }

  async function syncVoiceCatalog(model: AiModel) {
    setSyncingModelId(model.model_id);
    setCatalogMessage("");
    setError("");
    try {
      const sync = await requestAdminVoiceCatalogSync(apiClient, model.model_id);
      setCatalogs((current) => ({
        ...current,
        [model.model_id]: {
          model_id: model.model_id,
          source_model_id: current[model.model_id]?.source_model_id
            ?? model.voice_catalog_source_model_id
            ?? model.model_id,
          model_settings: model.settings,
          voices: current[model.model_id]?.voices ?? [],
          last_sync: sync,
        },
      }));
      setCatalogMessage(
        sync.status === "succeeded" ? "音色目录同步完成" : "音色目录已进入同步队列",
      );
    } catch (reason) {
      handleMutationError(reason);
    } finally {
      setSyncingModelId("");
    }
  }

  async function toggleStatus(model: AiModel) {
    if (model.status === "enabled" && model.is_default) {
      setReplacementId("");
      setConfirm({ kind: "disable", model });
      return;
    }
    try {
      await changeAiModelStatus(apiClient, model.model_id, {
        version: model.version,
        status: model.status === "enabled" ? "disabled" : "enabled",
      });
      await loadModels();
    } catch (reason) { handleMutationError(reason); }
  }

  async function confirmLifecycle() {
    if (!confirm) return;
    const { model, kind } = confirm;
    const peers = models.filter((item) => item.model_id !== model.model_id
      && item.status === "enabled"
      && (model.model_type !== "speech" || item.api_protocol === model.api_protocol));
    const action = {
      version: model.version,
      replacement_model_id: replacementId || null,
      allow_no_default: peers.length === 0,
    };
    try {
      if (kind === "disable") {
        await changeAiModelStatus(apiClient, model.model_id, { ...action, status: "disabled" });
      } else {
        await deleteAiModel(apiClient, model.model_id, action);
      }
      setConfirm(null);
      await loadModels();
    } catch (reason) { handleMutationError(reason); }
  }

  const replacementModels = confirm
    ? models.filter((item) => item.model_id !== confirm.model.model_id
      && item.status === "enabled"
      && (confirm.model.model_type !== "speech" || item.api_protocol === confirm.model.api_protocol))
    : [];

  return (
    <AdminShell active="模型与路由">
      <div className="modelManagementLayout">
        <header className="modelHeader">
          <div><p className="sectionKicker">模型与路由</p><h1>AI 模型管理</h1></div>
          <button className="primaryButton" onClick={openCreate}>添加模型</button>
        </header>
        <div className="modelTabs" role="tablist">
          {(Object.keys(typeLabels) as AiModelType[]).map((type) => (
            <button className={modelType === type ? "active" : ""} key={type} onClick={() => { setModelType(type); setProtocol(""); }}>{typeLabels[type]}</button>
          ))}
        </div>
        <section className="modelToolbar" aria-label="模型筛选">
          <input aria-label="搜索模型" placeholder="搜索名称、模型标识或供应商" value={search} onChange={(event) => setSearch(event.target.value)} />
          <select aria-label="状态筛选" value={status} onChange={(event) => setStatus(event.target.value as AiModelStatus | "")}><option value="">全部状态</option><option value="enabled">已启用</option><option value="disabled">已停用</option><option value="deleted">已删除</option></select>
          <input aria-label="供应商筛选" placeholder="供应商" value={provider} onChange={(event) => setProvider(event.target.value)} />
          <select aria-label="协议筛选" value={protocol} onChange={(event) => setProtocol(event.target.value as AiModelProtocol | "")}><option value="">全部协议</option>{protocolOptions[modelType].map((item) => <option value={item.value} key={item.value}>{item.label}</option>)}</select>
        </section>
        {error && <div className="inlineError" role="alert">{error}</div>}
        {catalogMessage && <div className="inlineSuccess" role="status">{catalogMessage}</div>}
        <section className="modelTableWrap" aria-busy={loading}>
          <table className="modelTable">
            <thead><tr>{["模型名称", "类型", "供应商 / API 协议", "请求地址", "默认", "状态", "能力目录", "最近调用", "更新时间", "操作"].map((label) => <th key={label}>{label}</th>)}</tr></thead>
            <tbody>
              {!loading && models.map((model) => (
                <tr key={model.model_id}>
                  <td><strong>{model.display_name}</strong><small>{model.upstream_model}</small></td>
                  <td>{typeLabels[model.model_type]}</td>
                  <td>{model.provider_name}<small>{model.api_protocol}</small></td>
                  <td className="urlCell" title={model.request_base_url}>{model.request_base_url}</td>
                  <td>{model.is_default ? <span className="statusTag info">默认</span> : "-"}</td>
                  <td><span className={`statusTag ${model.status}`}>{model.status === "enabled" ? "已启用" : model.status === "disabled" ? "已停用" : "已删除"}</span></td>
                  <td>
                    {["volcengine_tts_v3", "openai_audio_speech"].includes(model.api_protocol) ? (
                      <span className="catalogStatus">
                        {catalogs[model.model_id]?.last_sync?.status === "succeeded"
                          ? `${catalogs[model.model_id].voices.filter((voice) => voice.is_available).length} 个可用音色`
                          : catalogs[model.model_id]?.last_sync?.status === "failed"
                            ? "同步失败"
                            : catalogs[model.model_id]?.last_sync
                              ? "同步中"
                              : "尚未同步"}
                        <small>{model.voice_catalog_mode === "shared"
                          ? `复用：${model.voice_catalog_source_display_name ?? "来源模型不可用"}`
                          : "官方同步"}</small>
                        {catalogs[model.model_id]?.last_sync?.completed_at && (
                          <small>{new Date(catalogs[model.model_id].last_sync!.completed_at!).toLocaleString("zh-CN")}</small>
                        )}
                      </span>
                    ) : "-"}
                  </td>
                  <td>{model.last_call_status === "never" ? "未调用" : model.last_call_status === "success" ? "成功" : "失败"}</td>
                  <td>{new Date(model.updated_at).toLocaleString("zh-CN")}</td>
                  <td><div className="rowActions">{["volcengine_tts_v3", "openai_audio_speech"].includes(model.api_protocol) && model.status === "enabled" && <button disabled={syncingModelId === model.model_id} onClick={() => void syncVoiceCatalog(model)}>{syncingModelId === model.model_id ? "同步中" : "同步音色"}</button>}<button onClick={() => openEdit(model)}>编辑</button>{!model.is_default && model.status === "enabled" && <button onClick={() => void makeDefault(model)}>设为默认</button>}<button onClick={() => void toggleStatus(model)}>{model.status === "enabled" ? "停用" : "启用"}</button><button className="dangerText" onClick={() => { setReplacementId(""); setConfirm({ kind: "delete", model }); }}>删除</button></div></td>
                </tr>
              ))}
            </tbody>
          </table>
          {loading && <div className="tableState">正在加载模型...</div>}
          {!loading && models.length === 0 && <div className="tableState"><strong>当前类型暂无模型</strong><span>使用“添加模型”创建第一条配置</span></div>}
        </section>
      </div>

      {drawerOpen && (
        <div className="drawerBackdrop" role="presentation">
          <aside className="modelDrawer" role="dialog" aria-label={editing ? "编辑 AI 模型" : "添加 AI 模型"}>
            <header><div><p className="sectionKicker">模型配置</p><h2>{editing ? "编辑 AI 模型" : "添加 AI 模型"}</h2></div><button onClick={closeDrawer}>关闭</button></header>
            <form className="modelDrawerForm" onSubmit={submitForm}>
              <div className="modelDrawerFormScroll">
              <fieldset>
                <legend>基础信息</legend>
                <label>显示名称<input required value={form.display_name} onChange={(event) => setForm({ ...form, display_name: event.target.value })} /></label>
                <label>模型类型<select aria-label="模型类型" value={form.model_type} onChange={(event) => changeType(event.target.value as AiModelType)}>{(Object.keys(typeLabels) as AiModelType[]).map((type) => <option value={type} key={type}>{typeLabels[type]}</option>)}</select></label>
                <label>供应商<input required value={form.provider_name} onChange={(event) => setForm({ ...form, provider_name: event.target.value })} /></label>
                <label>上游模型标识<input required value={form.upstream_model} onChange={(event) => setForm({ ...form, upstream_model: event.target.value })} /></label>
              </fieldset>
              <fieldset>
                <legend>API 调用协议与凭据</legend>
                <label>API 调用协议<select aria-label="API 调用协议" value={form.api_protocol} onChange={(event) => changeProtocol(event.target.value as AiModelProtocol)}>{protocolOptions[form.model_type].map((item) => <option value={item.value} key={item.value}>{item.label}</option>)}</select></label>
                <label>协议版本<input value={form.protocol_version} onChange={(event) => setForm({ ...form, protocol_version: event.target.value })} /></label>
                <label>请求地址<input aria-label="请求地址" required type="url" value={form.request_base_url} onChange={(event) => setForm({ ...form, request_base_url: event.target.value })} /></label>
                <label>{apiKeyFieldLabel}<input aria-label={apiKeyFieldLabel} required={!editing} type="password" autoComplete="new-password" value={form.api_key} onChange={(event) => setForm({ ...form, api_key: event.target.value })} />{editing?.api_key_configured && <small>已配置：{editing.api_key_masked}</small>}</label>
                {form.auth_scheme === "access_key_secret" && <label>API Secret<input aria-label="API Secret" required={!editing} type="password" autoComplete="new-password" value={form.api_secret ?? ""} onChange={(event) => setForm({ ...form, api_secret: event.target.value })} />{editing?.api_secret_configured && <small>已配置：{editing.api_secret_masked}</small>}</label>}
              </fieldset>
              {isTtsForm && (
                <fieldset>
                  <legend>音色目录来源</legend>
                  {isOpenAiTtsForm ? (
                    <>
                      <p className="modelFieldsetHelp">OpenAI Audio Speech 中转必须复用相同上游模型和资源 ID 的官方目录。</p>
                      <label className="voiceCatalogSourceField">目录来源模型
                        <select aria-label="目录来源模型" required disabled={voiceCatalogSourceLoading || Boolean(voiceCatalogSourceError)} value={form.voice_catalog_source_model_id ?? ""} onChange={(event) => setForm({ ...form, voice_catalog_source_model_id: event.target.value || null })}>
                          <option value="">{voiceCatalogSourceLoading ? "加载中..." : "请选择官方目录模型"}</option>
                          {ttsCatalogSourceModels.map((model) => <option value={model.model_id} key={model.model_id}>{model.display_name}</option>)}
                        </select>
                      </label>
                      {voiceCatalogSourceLoading && <p className="modelFieldsetHelp voiceCatalogSourceStatus" role="status">正在加载目录来源模型...</p>}
                      {voiceCatalogSourceError && <div className="inlineError voiceCatalogSourceStatus" role="alert"><strong>目录来源模型加载失败</strong><span>{voiceCatalogSourceError}</span><button type="button" aria-label="重试加载目录来源" onClick={() => void loadVoiceCatalogSourceModels()}>重试</button></div>}
                      {!voiceCatalogSourceLoading && !voiceCatalogSourceError && ttsCatalogSourceModels.length === 0 && <p className="inlineWarning voiceCatalogSourceWarning">没有匹配当前上游模型和资源 ID 的官方目录模型。</p>}
                    </>
                  ) : (
                    <>
                      <div className="voiceCatalogModeControl" role="radiogroup" aria-label="音色目录来源模式">
                        <label className={form.voice_catalog_mode === "official_sync" ? "active" : ""}>
                          <input type="radio" name="voice-catalog-mode" value="official_sync" checked={form.voice_catalog_mode === "official_sync"} onChange={() => setForm({ ...form, voice_catalog_mode: "official_sync", voice_catalog_source_model_id: null })} />
                          官方同步
                        </label>
                        <label className={form.voice_catalog_mode === "shared" ? "active" : ""}>
                          <input type="radio" name="voice-catalog-mode" value="shared" checked={form.voice_catalog_mode === "shared"} onChange={() => setForm({ ...form, voice_catalog_mode: "shared", voice_catalog_source_model_id: ttsCatalogSourceModels[0]?.model_id ?? null, catalog_access_key: null, catalog_secret_key: null })} />
                          复用已有目录
                        </label>
                      </div>
                      {form.voice_catalog_mode === "official_sync" ? (
                        <>
                          <p className="modelFieldsetHelp">
                            OpenAPI AK/SK 仅用于 ListSpeakers HMAC 签名，不会进入请求体；与 TTS X-Api-Key 不同。
                          </p>
                          <label>OpenAPI Access Key（AK）<input aria-label="OpenAPI Access Key（AK）" required={!editing || !editing.catalog_access_key_configured || editing.voice_catalog_mode === "shared"} type="password" autoComplete="new-password" value={form.catalog_access_key ?? ""} onChange={(event) => setForm({ ...form, catalog_access_key: event.target.value })} />{editing?.catalog_access_key_configured && <small>已配置：{editing.catalog_access_key_masked}</small>}</label>
                          <label>OpenAPI Secret Key（SK）<input aria-label="OpenAPI Secret Key（SK）" required={!editing || !editing.catalog_secret_key_configured || editing.voice_catalog_mode === "shared"} type="password" autoComplete="new-password" value={form.catalog_secret_key ?? ""} onChange={(event) => setForm({ ...form, catalog_secret_key: event.target.value })} />{editing?.catalog_secret_key_configured && <small>已配置：{editing.catalog_secret_key_masked}</small>}</label>
                        </>
                      ) : (
                        <>
                          <p className="modelFieldsetHelp">复用相同上游模型和资源 ID 的官方音色目录；当前中转模型不保存 OpenAPI AK/SK。</p>
                          <label className="voiceCatalogSourceField">目录来源模型
                            <select aria-label="目录来源模型" required disabled={voiceCatalogSourceLoading || Boolean(voiceCatalogSourceError)} value={form.voice_catalog_source_model_id ?? ""} onChange={(event) => setForm({ ...form, voice_catalog_source_model_id: event.target.value || null })}>
                              <option value="">{voiceCatalogSourceLoading ? "加载中..." : "请选择官方目录模型"}</option>
                              {ttsCatalogSourceModels.map((model) => <option value={model.model_id} key={model.model_id}>{model.display_name}</option>)}
                            </select>
                          </label>
                          {voiceCatalogSourceLoading && <p className="modelFieldsetHelp voiceCatalogSourceStatus" role="status">正在加载目录来源模型...</p>}
                          {voiceCatalogSourceError && <div className="inlineError voiceCatalogSourceStatus" role="alert"><strong>目录来源模型加载失败</strong><span>{voiceCatalogSourceError}</span><button type="button" aria-label="重试加载目录来源" onClick={() => void loadVoiceCatalogSourceModels()}>重试</button></div>}
                          {!voiceCatalogSourceLoading && !voiceCatalogSourceError && ttsCatalogSourceModels.length === 0 && <p className="inlineWarning voiceCatalogSourceWarning">没有匹配当前上游模型和资源 ID 的官方目录模型。</p>}
                        </>
                      )}
                    </>
                  )}
                </fieldset>
              )}
              <fieldset><legend>运行配置</legend><label>超时时间（秒）<input type="number" min="1" max="3600" value={form.timeout_seconds} onChange={(event) => setForm({ ...form, timeout_seconds: Number(event.target.value) })} /></label><label>排序值<input type="number" value={form.sort_order} onChange={(event) => setForm({ ...form, sort_order: Number(event.target.value) })} /></label><label className="checkboxLabel"><input type="checkbox" checked={form.is_default} onChange={(event) => setForm({ ...form, is_default: event.target.checked })} />设为该类型默认模型</label></fieldset>
              {form.model_type === "text" && <fieldset><legend>文本推理配置</legend><label>推理等级<select aria-label="推理等级" value={form.reasoning_effort ?? ""} onChange={(event) => setForm({ ...form, reasoning_effort: event.target.value || null })}><option value="">不设置</option>{["low", "medium", "high", "xhigh"].map((value) => <option key={value}>{value}</option>)}</select></label><label>最大输出 Token<input type="number" min="1" value={form.max_output_tokens ?? ""} onChange={(event) => setForm({ ...form, max_output_tokens: event.target.value ? Number(event.target.value) : null })} /></label></fieldset>}
              {form.model_type === "image" && <fieldset><legend>图片配置</legend><label>默认图片尺寸<input aria-label="默认图片尺寸" value={String(form.settings.default_size ?? "")} onChange={(event) => { const value = event.target.value; setForm({ ...form, settings: { ...form.settings, default_size: value, supported_sizes: value.trim() ? [value.trim()] : [] } }); }} /></label><label>单次最大图片数<input aria-label="单次最大图片数" type="number" min="1" max="48" disabled={form.api_protocol === "volcengine_ark_images"} value={form.api_protocol === "volcengine_ark_images" ? 1 : Number(form.settings.max_images_per_request ?? 4)} onChange={(event) => setForm({ ...form, settings: { ...form.settings, max_images_per_request: Number(event.target.value) } })} /></label></fieldset>}
              {form.model_type === "video" && <fieldset><legend>视频配置</legend><label>支持分辨率<input placeholder="1080p, 720p" value={String((form.settings.resolutions as string[] | undefined)?.join(", ") ?? "")} onChange={(event) => setForm({ ...form, settings: { ...form.settings, resolutions: event.target.value.split(",").map((item) => item.trim()).filter(Boolean) } })} /></label><div className="fieldRow"><label>最短时长<input type="number" min="1" value={Number(form.settings.min_duration_seconds ?? 5)} onChange={(event) => setForm({ ...form, settings: { ...form.settings, min_duration_seconds: Number(event.target.value) } })} /></label><label>最长时长<input type="number" min="1" value={Number(form.settings.max_duration_seconds ?? 10)} onChange={(event) => setForm({ ...form, settings: { ...form.settings, max_duration_seconds: Number(event.target.value) } })} /></label></div></fieldset>}
              {form.model_type === "speech" && (
                <fieldset>
                  <legend>声音能力</legend>
                  <label>资源 ID<input aria-label="资源 ID" readOnly value={String(form.settings.resource_id ?? "")} /></label>
                  <label>支持音频格式<input aria-label="支持音频格式" value={String((form.settings.supported_audio_formats as string[] | undefined)?.join(", ") ?? "")} onChange={(event) => updateSpeechSetting("supported_audio_formats", event.target.value.split(",").map((item) => item.trim().toLowerCase()).filter(Boolean))} /></label>
                  <label>默认音频格式<input aria-label="默认音频格式" value={String(form.settings.default_audio_format ?? "")} onChange={(event) => updateSpeechSetting("default_audio_format", event.target.value.trim().toLowerCase())} /></label>
                  <label>支持采样率<input aria-label="支持采样率" value={String((form.settings.supported_sample_rates as number[] | undefined)?.join(", ") ?? "")} onChange={(event) => updateSpeechSetting("supported_sample_rates", event.target.value.split(",").map((item) => Number(item.trim())).filter((item) => Number.isFinite(item) && item > 0))} /></label>
                  <label>默认采样率<input aria-label="默认采样率" type="number" min="1" value={Number(form.settings.default_sample_rate ?? 24000)} onChange={(event) => updateSpeechSetting("default_sample_rate", Number(event.target.value))} /></label>
                  <div className="speechTimestampLanguages" ref={timestampLanguageDropdownRef}>
                    <span className="speechFieldLabel">时间戳语言</span>
                    {isNativeTtsForm ? (
                      <>
                        <button
                          ref={timestampLanguageTriggerRef}
                          type="button"
                          className={`speechTimestampLanguageTrigger${timestampLanguageOpen ? " isOpen" : ""}`}
                          aria-label="时间戳语言"
                          aria-expanded={timestampLanguageOpen}
                          aria-haspopup="dialog"
                          aria-controls="tts-timestamp-language-menu"
                          onClick={() => {
                            if (timestampLanguageOpen) setTimestampLanguageSearch("");
                            setTimestampLanguageOpen(!timestampLanguageOpen);
                          }}
                        >
                          <span>{selectedTtsTimestampLanguageLabels.join("、")}</span>
                          <span className="speechTimestampLanguageChevron" aria-hidden="true" />
                        </button>
                        {timestampLanguageOpen && (
                          <div
                            id="tts-timestamp-language-menu"
                            className="speechTimestampLanguageMenu"
                            role="dialog"
                            aria-label="时间戳语言选项"
                          >
                            <input
                              className="speechTimestampLanguageSearch"
                              type="search"
                              aria-label="搜索时间戳语言"
                              placeholder="搜索语言"
                              autoFocus
                              value={timestampLanguageSearch}
                              onChange={(event) => setTimestampLanguageSearch(event.target.value)}
                            />
                            <div className="speechTimestampLanguageOptions">
                              {filteredTtsTimestampLanguageOptions.map((option) => {
                                const checked = selectedTtsTimestampLanguages.includes(option.value);
                                const locked = checked && selectedTtsTimestampLanguages.length === 1;
                                return (
                                  <label
                                    className={`speechTimestampLanguageOption${locked ? " isLocked" : ""}`}
                                    key={option.value}
                                  >
                                    <input
                                      type="checkbox"
                                      aria-label={option.label}
                                      checked={checked}
                                      disabled={locked}
                                      onChange={(event) => toggleTtsTimestampLanguage(option.value, event.target.checked)}
                                    />
                                    <span>{option.label}</span>
                                  </label>
                                );
                              })}
                              {filteredTtsTimestampLanguageOptions.length === 0 && (
                                <p className="speechTimestampLanguageEmpty" role="status">未找到匹配语言</p>
                              )}
                            </div>
                          </div>
                        )}
                      </>
                    ) : isOpenAiTtsForm ? (
                      <output className="speechReadOnlyValue" aria-label="时间戳语言">
                        不支持（仅生成配音）
                      </output>
                    ) : (
                      <output className="speechReadOnlyValue" aria-label="时间戳语言">
                        自动识别（全部语言）
                      </output>
                    )}
                  </div>
                  <label className="checkboxLabel"><input type="checkbox" checked={Boolean(form.settings.supports_word_timestamps)} disabled />支持字词时间戳</label>
                  {isTtsForm ? (
                    <>
                      <label>最大输入字符数<input aria-label="最大输入字符数" type="number" min="1" value={Number(form.settings.max_input_characters ?? 3000)} onChange={(event) => updateSpeechSetting("max_input_characters", Number(event.target.value))} /></label>
                      {isNativeTtsForm && <label>定期同步间隔（分钟）<input aria-label="定期同步间隔" type="number" min="1" value={Number(form.settings.catalog_sync_interval_minutes ?? 1440)} onChange={(event) => updateSpeechSetting("catalog_sync_interval_minutes", Number(event.target.value))} /></label>}
                      <label>语速下限<input aria-label="语速下限" type="number" min="0.1" step={isOpenAiTtsForm ? "0.01" : "0.1"} value={Number((((form.settings.parameters as Record<string, unknown>)?.speed_ratio as Record<string, unknown>)?.minimum) ?? 0.5)} onChange={(event) => updateSpeedConstraint("minimum", Number(event.target.value))} /></label>
                      <label>语速上限<input aria-label="语速上限" type="number" min="0.1" step={isOpenAiTtsForm ? "0.01" : "0.1"} value={Number((((form.settings.parameters as Record<string, unknown>)?.speed_ratio as Record<string, unknown>)?.maximum) ?? 2)} onChange={(event) => updateSpeedConstraint("maximum", Number(event.target.value))} /></label>
                    </>
                  ) : (
                    <label>最大音频时长（秒）<input aria-label="最大音频时长" type="number" min="1" value={Number(form.settings.max_audio_duration_seconds ?? 7200)} onChange={(event) => updateSpeechSetting("max_audio_duration_seconds", Number(event.target.value))} /></label>
                  )}
                </fieldset>
              )}
              {form.api_protocol === "volcengine_asr_v3" && (
                <fieldset className="systemToolReference">
                  <legend>系统私有 TOS</legend>
                  <p className="modelFieldsetHelp">全部 ASR 模型共用“工具与 MCP”中的系统配置，模型保存不修改 TOS。</p>
                  <div className="systemToolStatus" aria-label="系统私有 TOS 状态">
                    <span className={`statusTag ${tosTool?.configured && tosTool.enabled ? "enabled" : "disabled"}`}>
                      {tosToolLoading
                        ? "读取中"
                        : tosTool?.configured
                          ? tosTool.enabled ? "已配置并启用" : "已配置但未启用"
                          : "尚未配置"}
                    </span>
                    {tosTool?.configured && <span>版本 {tosTool.version}</span>}
                    {tosTool?.pending_cleanup_count ? <span>待清理对象 {tosTool.pending_cleanup_count}</span> : null}
                  </div>
                  <a className="inlineLink" href="/tools">前往工具与 MCP 配置</a>
                </fieldset>
              )}
              <label>备注<textarea rows={3} value={form.remark} onChange={(event) => setForm({ ...form, remark: event.target.value })} /></label>
              </div>
              <footer><button type="button" onClick={closeDrawer}>取消</button><button className="primaryButton" disabled={saving || (isNativeTtsForm && selectedTtsTimestampLanguages.length === 0) || (isTtsForm && (isOpenAiTtsForm || form.voice_catalog_mode === "shared") && (voiceCatalogSourceLoading || Boolean(voiceCatalogSourceError) || !form.voice_catalog_source_model_id || !ttsCatalogSourceModels.some((model) => model.model_id === form.voice_catalog_source_model_id)))} type="submit">{saving ? "保存中..." : "保存模型"}</button></footer>
            </form>
          </aside>
        </div>
      )}

      {confirm && (
        <div className="modalBackdrop">
          <section className="confirmModal" role="dialog" aria-label={confirm.kind === "disable" ? "停用默认模型" : "删除模型"}>
            <h2>{confirm.kind === "disable" ? "停用默认模型" : "删除模型"}</h2>
            <p>{confirm.kind === "disable" ? `停用后${confirm.model.model_type === "speech" ? "同一语音协议" : "该类型"}需要新的默认模型。` : "已有运行引用时将逻辑删除；未引用时将物理删除。"}</p>
            {confirm.model.is_default && replacementModels.length > 0 && <label>替代默认模型<select value={replacementId} onChange={(event) => setReplacementId(event.target.value)}><option value="">请选择</option>{replacementModels.map((model) => <option value={model.model_id} key={model.model_id}>{model.display_name}</option>)}</select></label>}
            {confirm.model.is_default && replacementModels.length === 0 && <p className="inlineWarning">当前没有可替代模型，确认后该类型将暂无默认模型。</p>}
            <footer><button onClick={() => setConfirm(null)}>取消</button><button className={confirm.kind === "delete" ? "dangerButton" : "primaryButton"} disabled={confirm.model.is_default && replacementModels.length > 0 && !replacementId} onClick={() => void confirmLifecycle()}>确认{confirm.kind === "disable" ? "停用" : "删除"}</button></footer>
          </section>
        </div>
      )}
    </AdminShell>
  );
}
