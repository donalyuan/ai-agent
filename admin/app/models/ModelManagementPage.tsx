"use client";

import { FormEvent, useCallback, useEffect, useMemo, useState } from "react";
import { AdminShell } from "../components/AdminShell";
import {
  AiModel,
  AiModelPayload,
  AiModelProtocol,
  AiModelStatus,
  AiModelType,
  ApiClient,
  ApiError,
  changeAiModelStatus,
  createAiModel,
  createApiClient,
  deleteAiModel,
  listAiModels,
  setDefaultAiModel,
  updateAiModel,
} from "../lib/api";

const typeLabels: Record<AiModelType, string> = { text: "文本模型", image: "图片模型", video: "视频模型" };
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
};

type FormState = AiModelPayload & { version?: number };
type ConfirmState = { kind: "disable" | "delete"; model: AiModel } | null;

function emptyForm(modelType: AiModelType): FormState {
  const protocol = protocolOptions[modelType][0].value;
  return {
    display_name: "",
    model_type: modelType,
    provider_name: "",
    api_protocol: protocol,
    protocol_version: "v1",
    auth_scheme: protocol === "kling_api" ? "access_key_secret" : "bearer",
    request_base_url: "",
    upstream_model: "",
    api_key: "",
    api_secret: null,
    timeout_seconds: 120,
    reasoning_effort: modelType === "text" ? "low" : null,
    max_output_tokens: modelType === "text" ? 3000 : null,
    settings: modelType === "image"
      ? { supported_sizes: ["1024x1024"], default_size: "1024x1024", max_images_per_request: 4 }
      : {},
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
  if (form.model_type !== "image") return form;
  const defaultSize = String(form.settings.default_size ?? "").trim();
  const isArk = form.api_protocol === "volcengine_ark_images";
  return {
    ...form,
    auth_scheme: "bearer",
    api_secret: isArk ? null : form.api_secret,
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
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [editing, setEditing] = useState<AiModel | null>(null);
  const [form, setForm] = useState<FormState>(() => emptyForm("text"));
  const [saving, setSaving] = useState(false);
  const [confirm, setConfirm] = useState<ConfirmState>(null);
  const [replacementId, setReplacementId] = useState("");

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
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "模型列表加载失败");
    } finally {
      setLoading(false);
    }
  }, [apiClient, modelType, status, provider, protocol, search]);

  useEffect(() => { void loadModels(); }, [loadModels]);

  function openCreate() {
    setEditing(null);
    setForm(emptyForm(modelType));
    setDrawerOpen(true);
    setError("");
  }

  function openEdit(model: AiModel) {
    setEditing(model);
    setForm(formFromModel(model));
    setDrawerOpen(true);
    setError("");
  }

  function changeType(nextType: AiModelType) {
    const next = emptyForm(nextType);
    setForm((current) => ({ ...next, display_name: current.display_name, provider_name: current.provider_name }));
  }

  function changeProtocol(nextProtocol: AiModelProtocol) {
    setForm((current) => ({
      ...current,
      api_protocol: nextProtocol,
      auth_scheme: nextProtocol === "kling_api" ? "access_key_secret" : "bearer",
      api_secret: nextProtocol === "volcengine_ark_images" ? null : current.api_secret,
      settings: current.model_type !== "image"
        ? current.settings
        : nextProtocol === "volcengine_ark_images"
          ? { supported_sizes: [], default_size: null, max_images_per_request: 1 }
          : { supported_sizes: ["1024x1024"], default_size: "1024x1024", max_images_per_request: 4 },
    }));
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
      setDrawerOpen(false);
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
    const peers = models.filter((item) => item.model_id !== model.model_id && item.status === "enabled");
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
    ? models.filter((item) => item.model_id !== confirm.model.model_id && item.status === "enabled")
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
        <section className="modelTableWrap" aria-busy={loading}>
          <table className="modelTable">
            <thead><tr>{["模型名称", "类型", "供应商 / API 协议", "请求地址", "默认", "状态", "最近调用", "更新时间", "操作"].map((label) => <th key={label}>{label}</th>)}</tr></thead>
            <tbody>
              {!loading && models.map((model) => (
                <tr key={model.model_id}>
                  <td><strong>{model.display_name}</strong><small>{model.upstream_model}</small></td>
                  <td>{typeLabels[model.model_type]}</td>
                  <td>{model.provider_name}<small>{model.api_protocol}</small></td>
                  <td className="urlCell" title={model.request_base_url}>{model.request_base_url}</td>
                  <td>{model.is_default ? <span className="statusTag info">默认</span> : "-"}</td>
                  <td><span className={`statusTag ${model.status}`}>{model.status === "enabled" ? "已启用" : model.status === "disabled" ? "已停用" : "已删除"}</span></td>
                  <td>{model.last_call_status === "never" ? "未调用" : model.last_call_status === "success" ? "成功" : "失败"}</td>
                  <td>{new Date(model.updated_at).toLocaleString("zh-CN")}</td>
                  <td><div className="rowActions"><button onClick={() => openEdit(model)}>编辑</button>{!model.is_default && model.status === "enabled" && <button onClick={() => void makeDefault(model)}>设为默认</button>}<button onClick={() => void toggleStatus(model)}>{model.status === "enabled" ? "停用" : "启用"}</button><button className="dangerText" onClick={() => { setReplacementId(""); setConfirm({ kind: "delete", model }); }}>删除</button></div></td>
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
            <header><div><p className="sectionKicker">模型配置</p><h2>{editing ? "编辑 AI 模型" : "添加 AI 模型"}</h2></div><button onClick={() => setDrawerOpen(false)}>关闭</button></header>
            <form onSubmit={submitForm}>
              <fieldset><legend>基础信息</legend><label>显示名称<input required value={form.display_name} onChange={(event) => setForm({ ...form, display_name: event.target.value })} /></label><label>模型类型<select aria-label="模型类型" value={form.model_type} onChange={(event) => changeType(event.target.value as AiModelType)}>{(Object.keys(typeLabels) as AiModelType[]).map((type) => <option value={type} key={type}>{typeLabels[type]}</option>)}</select></label><label>供应商<input required value={form.provider_name} onChange={(event) => setForm({ ...form, provider_name: event.target.value })} /></label><label>上游模型标识<input required value={form.upstream_model} onChange={(event) => setForm({ ...form, upstream_model: event.target.value })} /></label></fieldset>
              <fieldset><legend>API 调用协议与凭据</legend><label>API 调用协议<select value={form.api_protocol} onChange={(event) => changeProtocol(event.target.value as AiModelProtocol)}>{protocolOptions[form.model_type].map((item) => <option value={item.value} key={item.value}>{item.label}</option>)}</select></label><label>协议版本<input value={form.protocol_version} onChange={(event) => setForm({ ...form, protocol_version: event.target.value })} /></label><label>请求地址<input required type="url" value={form.request_base_url} onChange={(event) => setForm({ ...form, request_base_url: event.target.value })} /></label><label>API Key<input aria-label="API Key" required={!editing} type="password" autoComplete="new-password" value={form.api_key} onChange={(event) => setForm({ ...form, api_key: event.target.value })} />{editing?.api_key_configured && <small>已配置：{editing.api_key_masked}</small>}</label>{form.auth_scheme === "access_key_secret" && <label>API Secret<input aria-label="API Secret" required={!editing} type="password" autoComplete="new-password" value={form.api_secret ?? ""} onChange={(event) => setForm({ ...form, api_secret: event.target.value })} />{editing?.api_secret_configured && <small>已配置：{editing.api_secret_masked}</small>}</label>}</fieldset>
              <fieldset><legend>运行配置</legend><label>超时时间（秒）<input type="number" min="1" max="3600" value={form.timeout_seconds} onChange={(event) => setForm({ ...form, timeout_seconds: Number(event.target.value) })} /></label><label>排序值<input type="number" value={form.sort_order} onChange={(event) => setForm({ ...form, sort_order: Number(event.target.value) })} /></label><label className="checkboxLabel"><input type="checkbox" checked={form.is_default} onChange={(event) => setForm({ ...form, is_default: event.target.checked })} />设为该类型默认模型</label></fieldset>
              {form.model_type === "text" && <fieldset><legend>文本推理配置</legend><label>推理等级<select aria-label="推理等级" value={form.reasoning_effort ?? ""} onChange={(event) => setForm({ ...form, reasoning_effort: event.target.value || null })}><option value="">不设置</option>{["low", "medium", "high", "xhigh"].map((value) => <option key={value}>{value}</option>)}</select></label><label>最大输出 Token<input type="number" min="1" value={form.max_output_tokens ?? ""} onChange={(event) => setForm({ ...form, max_output_tokens: event.target.value ? Number(event.target.value) : null })} /></label></fieldset>}
              {form.model_type === "image" && <fieldset><legend>图片配置</legend><label>默认图片尺寸<input aria-label="默认图片尺寸" value={String(form.settings.default_size ?? "")} onChange={(event) => { const value = event.target.value; setForm({ ...form, settings: { ...form.settings, default_size: value, supported_sizes: value.trim() ? [value.trim()] : [] } }); }} /></label><label>单次最大图片数<input aria-label="单次最大图片数" type="number" min="1" max="48" disabled={form.api_protocol === "volcengine_ark_images"} value={form.api_protocol === "volcengine_ark_images" ? 1 : Number(form.settings.max_images_per_request ?? 4)} onChange={(event) => setForm({ ...form, settings: { ...form.settings, max_images_per_request: Number(event.target.value) } })} /></label></fieldset>}
              {form.model_type === "video" && <fieldset><legend>视频配置</legend><label>支持分辨率<input placeholder="1080p, 720p" value={String((form.settings.resolutions as string[] | undefined)?.join(", ") ?? "")} onChange={(event) => setForm({ ...form, settings: { ...form.settings, resolutions: event.target.value.split(",").map((item) => item.trim()).filter(Boolean) } })} /></label><div className="fieldRow"><label>最短时长<input type="number" min="1" value={Number(form.settings.min_duration_seconds ?? 5)} onChange={(event) => setForm({ ...form, settings: { ...form.settings, min_duration_seconds: Number(event.target.value) } })} /></label><label>最长时长<input type="number" min="1" value={Number(form.settings.max_duration_seconds ?? 10)} onChange={(event) => setForm({ ...form, settings: { ...form.settings, max_duration_seconds: Number(event.target.value) } })} /></label></div></fieldset>}
              <label>备注<textarea rows={3} value={form.remark} onChange={(event) => setForm({ ...form, remark: event.target.value })} /></label>
              <footer><button type="button" onClick={() => setDrawerOpen(false)}>取消</button><button className="primaryButton" disabled={saving} type="submit">{saving ? "保存中..." : "保存模型"}</button></footer>
            </form>
          </aside>
        </div>
      )}

      {confirm && (
        <div className="modalBackdrop">
          <section className="confirmModal" role="dialog" aria-label={confirm.kind === "disable" ? "停用默认模型" : "删除模型"}>
            <h2>{confirm.kind === "disable" ? "停用默认模型" : "删除模型"}</h2>
            <p>{confirm.kind === "disable" ? "停用后该类型需要新的默认模型。" : "已有运行引用时将逻辑删除；未引用时将物理删除。"}</p>
            {confirm.model.is_default && replacementModels.length > 0 && <label>替代默认模型<select value={replacementId} onChange={(event) => setReplacementId(event.target.value)}><option value="">请选择</option>{replacementModels.map((model) => <option value={model.model_id} key={model.model_id}>{model.display_name}</option>)}</select></label>}
            {confirm.model.is_default && replacementModels.length === 0 && <p className="inlineWarning">当前没有可替代模型，确认后该类型将暂无默认模型。</p>}
            <footer><button onClick={() => setConfirm(null)}>取消</button><button className={confirm.kind === "delete" ? "dangerButton" : "primaryButton"} disabled={confirm.model.is_default && replacementModels.length > 0 && !replacementId} onClick={() => void confirmLifecycle()}>确认{confirm.kind === "disable" ? "停用" : "删除"}</button></footer>
          </section>
        </div>
      )}
    </AdminShell>
  );
}
