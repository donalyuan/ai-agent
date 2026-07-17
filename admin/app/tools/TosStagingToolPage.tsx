"use client";

import { FormEvent, useCallback, useEffect, useMemo, useState } from "react";
import { AdminShell } from "../components/AdminShell";
import {
  ApiClient,
  ApiError,
  SaveTosStagingToolPayload,
  TosStagingCheckStatus,
  TosStagingToolConfig,
  checkTosStagingTool,
  createApiClient,
  getTosStagingTool,
  saveTosStagingTool,
} from "../lib/api";

type TosForm = Omit<SaveTosStagingToolPayload, "version">;

const emptyForm: TosForm = {
  enabled: false,
  storage_provider: "volcengine_tos",
  endpoint: "https://tos-cn-beijing.volces.com",
  region: "cn-beijing",
  bucket: "",
  object_prefix: "novex/asr",
  access_key: "",
  secret_key: "",
  signed_url_ttl_seconds: 600,
  max_file_bytes: 104857600,
  max_audio_duration_seconds: 7200,
};

const checkStatusLabels: Record<TosStagingCheckStatus, string> = {
  never: "未检查",
  queued: "待检查",
  running: "检查中",
  succeeded: "连接正常",
  failed: "连接失败",
};

function formFromConfig(config: TosStagingToolConfig): TosForm {
  if (!config.configured) return { ...emptyForm };
  return {
    enabled: config.enabled,
    storage_provider: "volcengine_tos",
    endpoint: config.endpoint ?? emptyForm.endpoint,
    region: config.region ?? emptyForm.region,
    bucket: config.bucket ?? "",
    object_prefix: config.object_prefix ?? emptyForm.object_prefix,
    access_key: "",
    secret_key: "",
    signed_url_ttl_seconds: config.signed_url_ttl_seconds ?? emptyForm.signed_url_ttl_seconds,
    max_file_bytes: config.max_file_bytes ?? emptyForm.max_file_bytes,
    max_audio_duration_seconds:
      config.max_audio_duration_seconds ?? emptyForm.max_audio_duration_seconds,
  };
}

function errorCode(reason: unknown): string | undefined {
  if (!(reason instanceof ApiError) || !reason.details || typeof reason.details !== "object") {
    return undefined;
  }
  const details = reason.details as { error?: { code?: string } };
  return details.error?.code;
}

export function TosStagingToolPage({ client }: { client?: ApiClient }) {
  const apiClient = useMemo(() => client ?? createApiClient(), [client]);
  const [config, setConfig] = useState<TosStagingToolConfig | null>(null);
  const [form, setForm] = useState<TosForm>({ ...emptyForm });
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [checking, setChecking] = useState(false);
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");

  const load = useCallback(async (replaceForm = true) => {
    try {
      const current = await getTosStagingTool(apiClient);
      setConfig(current);
      if (replaceForm) setForm(formFromConfig(current));
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "TOS 配置加载失败");
    } finally {
      setLoading(false);
    }
  }, [apiClient]);

  useEffect(() => { void load(); }, [load]);

  useEffect(() => {
    if (!config || !["queued", "running"].includes(config.last_check_status)) return;
    const timer = globalThis.setInterval(() => { void load(false); }, 3000);
    return () => globalThis.clearInterval(timer);
  }, [config, load]);

  function update<K extends keyof TosForm>(field: K, value: TosForm[K]) {
    setForm((current) => ({
      ...current,
      [field]: value,
      ...(field !== "enabled" && config?.configured ? { enabled: false } : {}),
    }));
  }

  function handleError(reason: unknown) {
    const code = errorCode(reason);
    if (code === "tos_staging_version_conflict") {
      setError("配置已被其他操作更新，已重新加载，请确认后再保存");
      void load();
      return;
    }
    if (code === "tos_staging_cleanup_pending") {
      setError("存在待清理的临时对象，完成清理前不能修改或停用系统 TOS");
      void load(false);
      return;
    }
    if (code === "tos_staging_check_required") {
      setError("当前版本尚未通过真实 Bucket 连接检查，不能启用系统 TOS");
      return;
    }
    setError(reason instanceof Error ? reason.message : "操作失败");
  }

  async function submit(event: FormEvent) {
    event.preventDefault();
    setError("");
    setMessage("");
    const hasAccessKey = form.access_key.trim().length > 0;
    const hasSecretKey = form.secret_key.trim().length > 0;
    if (hasAccessKey !== hasSecretKey) {
      setError("Access Key 和 Secret Key 必须同时填写或同时留空");
      return;
    }
    setSaving(true);
    try {
      const saved = await saveTosStagingTool(apiClient, {
        ...form,
        version: config?.version ?? null,
      });
      setConfig(saved);
      setForm(formFromConfig(saved));
      setMessage(`系统 TOS 已保存为版本 ${saved.version}`);
    } catch (reason) {
      handleError(reason);
    } finally {
      setSaving(false);
    }
  }

  async function checkConnection() {
    if (config?.version == null) return;
    setChecking(true);
    setError("");
    setMessage("");
    try {
      const queued = await checkTosStagingTool(apiClient, config.version);
      setConfig(queued);
      setMessage("TOS Bucket 连接检查已进入队列");
    } catch (reason) {
      handleError(reason);
    } finally {
      setChecking(false);
    }
  }

  const statusClass = config?.last_check_status === "succeeded"
    ? "enabled"
    : config?.last_check_status === "queued" || config?.last_check_status === "running"
      ? "info"
      : "disabled";

  return (
    <AdminShell active="工具与 MCP">
      <div className="toolManagementPage">
        <header className="modelHeader">
          <div>
            <p className="sectionKicker">工具与 MCP</p>
            <h1>私有 TOS</h1>
          </div>
          <div className="toolHeaderStatus" aria-label="系统 TOS 总体状态">
            <span className={`statusTag ${config?.configured && config.enabled ? "enabled" : "disabled"}`}>
              {loading ? "读取中" : config?.configured ? config.enabled ? "已启用" : "未启用" : "未配置"}
            </span>
            {config?.version != null && <span>版本 {config.version}</span>}
          </div>
        </header>

        {error && <p className="inlineError" role="alert">{error}</p>}
        {message && <p className="inlineSuccess" role="status">{message}</p>}
        {Boolean(config?.pending_cleanup_count) && (
          <p className="inlineWarning toolWarning" role="alert">
            当前有 {config?.pending_cleanup_count} 个临时对象待清理，配置修改与停用已锁定。
          </p>
        )}

        <section className="toolStatusBand" aria-label="连接与运行状态">
          <div><span>连接检查</span><strong className={`statusTag ${statusClass}`}>{checkStatusLabels[config?.last_check_status ?? "never"]}</strong></div>
          <div><span>当前 Bucket</span><strong>{config?.bucket || "未配置"}</strong></div>
          <div><span>对象前缀</span><strong>{config?.object_prefix || "未配置"}</strong></div>
          <div><span>待清理对象</span><strong>{config?.pending_cleanup_count ?? 0}</strong></div>
          <button
            type="button"
            disabled={!config?.configured || checking || ["queued", "running"].includes(config.last_check_status)}
            onClick={() => void checkConnection()}
          >
            {checking ? "提交中..." : "检查连接"}
          </button>
        </section>
        {config?.last_check_error_summary && (
          <p className="toolCheckError" role="alert">{config.last_check_error_summary}</p>
        )}

        <form className="toolConfigForm" onSubmit={submit}>
          <fieldset disabled={loading || saving}>
            <legend>存储位置</legend>
            <label>存储服务<input aria-label="存储服务" readOnly value="火山引擎 TOS" /></label>
            <label>Endpoint<input aria-label="Endpoint" required type="url" value={form.endpoint} onChange={(event) => update("endpoint", event.target.value)} /></label>
            <label>Region<input aria-label="Region" required value={form.region} onChange={(event) => update("region", event.target.value)} /></label>
            <label>Bucket<input aria-label="Bucket" required value={form.bucket} onChange={(event) => update("bucket", event.target.value)} /></label>
            <label>对象前缀<input aria-label="对象前缀" required value={form.object_prefix} onChange={(event) => update("object_prefix", event.target.value)} /></label>
          </fieldset>

          <fieldset disabled={loading || saving}>
            <legend>访问凭据</legend>
            <label>Access Key<input aria-label="Access Key" required={!config?.access_key_configured} type="password" autoComplete="new-password" value={form.access_key} onChange={(event) => update("access_key", event.target.value)} />{config?.access_key_configured && <small>已配置：{config.access_key_masked}</small>}</label>
            <label>Secret Key<input aria-label="Secret Key" required={!config?.secret_key_configured} type="password" autoComplete="new-password" value={form.secret_key} onChange={(event) => update("secret_key", event.target.value)} />{config?.secret_key_configured && <small>已配置：{config.secret_key_masked}</small>}</label>
          </fieldset>

          <fieldset disabled={loading || saving}>
            <legend>暂存限制</legend>
            <label>签名有效期（秒）<input aria-label="签名有效期" required type="number" min="60" max="3600" value={form.signed_url_ttl_seconds} onChange={(event) => update("signed_url_ttl_seconds", Number(event.target.value))} /></label>
            <label>最大文件大小（字节）<input aria-label="最大文件大小" required type="number" min="1" value={form.max_file_bytes} onChange={(event) => update("max_file_bytes", Number(event.target.value))} /></label>
            <label>最大音频时长（秒）<input aria-label="最大音频时长" required type="number" min="1" value={form.max_audio_duration_seconds} onChange={(event) => update("max_audio_duration_seconds", Number(event.target.value))} /></label>
            <label className="checkboxLabel"><input aria-label="启用系统 TOS" type="checkbox" disabled={!form.enabled && config?.last_check_status !== "succeeded"} checked={form.enabled} onChange={(event) => update("enabled", event.target.checked)} />启用系统 TOS</label>
          </fieldset>

          <footer>
            <button type="submit" className="primaryButton" disabled={saving || loading || Boolean(config?.pending_cleanup_count)}>
              {saving ? "保存中..." : "保存配置"}
            </button>
          </footer>
        </form>
      </div>
    </AdminShell>
  );
}
