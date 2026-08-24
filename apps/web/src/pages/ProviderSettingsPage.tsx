import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  CircleAlert,
  KeyRound,
  RefreshCw,
  RotateCcw,
  ShieldCheck,
  ToggleLeft,
  Workflow,
} from "lucide-react";
import { useState } from "react";
import { settingsApi, settingsQueryKeys } from "../settings/api";
import { ErrorNotice, PageIntro, QueryNotice, SurfaceHeading } from "../ui";

type Provider = {
  id: string;
  name: string;
  adapterKey: string;
  enabled: boolean;
  revision: number;
  approval: string;
};
type Profile = {
  id: string;
  providerId: string;
  name: string;
  adapterIdentity: string;
  enabled: boolean;
  revision: number;
  credentialStatus: string;
  operationPolicies?: Record<string, Record<string, unknown>>;
  quotaSnapshots?: Record<
    string,
    { status: string; remaining?: number | null; source?: string }
  >;
};
type Catalog = {
  providers?: Provider[];
  profiles?: Profile[];
  models?: Array<{
    id: string;
    profileId: string;
    modelKey: string;
    enabled: boolean;
    revision: number;
    historicalReferences?: number;
  }>;
  skills?: Array<{
    id: string;
    name: string;
    version: string;
    approval: string;
    enabled: boolean;
    provenance: string;
    sourceType: string;
    revision: number;
    capabilities: string[];
  }>;
};

export function ProviderSettingsPage({ projectId }: { projectId: string }) {
  const client = useQueryClient();
  const catalog = useQuery({
    queryKey: settingsQueryKeys.catalog,
    queryFn: settingsApi.catalog,
  });
  const [credential, setCredential] = useState({ id: "", value: "" });
  const [syncCandidate, setSyncCandidate] = useState<{
    id: string;
    revision: number;
    added?: string[];
    removed?: string[];
  } | null>(null);
  const [diagnostic, setDiagnostic] = useState("");
  const data = (catalog.data ?? {}) as Catalog;
  const update = useMutation({
    mutationFn: ({
      id,
      revision,
      changes,
      kind,
    }: {
      id: string;
      revision: number;
      changes: Record<string, unknown>;
      kind: "provider" | "profile";
    }) =>
      kind === "provider"
        ? settingsApi.updateProvider(id, revision, changes)
        : settingsApi.updateProfile(id, revision, changes),
    onSuccess: () =>
      client.invalidateQueries({ queryKey: settingsQueryKeys.catalog }),
    onError: (error) =>
      setDiagnostic(error instanceof Error ? error.message : "更新失败"),
  });
  const replace = useMutation({
    mutationFn: (profile: Profile) =>
      settingsApi.replaceCredential(
        profile.id,
        profile.revision,
        credential.id.trim(),
        credential.value,
      ),
    onSuccess: async () => {
      setCredential({ id: "", value: "" });
      setDiagnostic("credential 已提交；只保留 masked status");
      await client.invalidateQueries({ queryKey: settingsQueryKeys.catalog });
    },
    onError: (error) =>
      setDiagnostic(
        error instanceof Error ? error.message : "credential 更新失败",
      ),
  });
  const probe = useMutation({
    mutationFn: (profile: Profile) =>
      settingsApi.probe(profile.id, "image.generate"),
    onSuccess: (value) => setDiagnostic(`probe: ${JSON.stringify(value)}`),
    onError: (error) =>
      setDiagnostic(error instanceof Error ? error.message : "probe failed"),
  });
  const sync = useMutation({
    mutationFn: (profile: Profile) =>
      settingsApi.syncModels(profile.id, ["mock-image-v1", "mock-video-v1"]),
    onSuccess: (value) => setSyncCandidate(value as typeof syncCandidate),
    onError: (error) =>
      setDiagnostic(error instanceof Error ? error.message : "sync failed"),
  });
  if (catalog.isPending)
    return (
      <section className="page-body">
        <PageIntro
          eyebrow="PROJECT SETTINGS / CATALOG"
          title="模型与能力"
          detail="只读取 owner catalog，不隐式启用真实调用。"
        />
        <QueryNotice isPending error={null} empty="" />
      </section>
    );
  return (
    <section className="page-body settings-page">
      <PageIntro
        eyebrow="PROJECT SETTINGS / CATALOG"
        title="模型与能力"
        detail="Provider、Model、Skill 和 StorageProfile 都以 owner revision/CAS 管理；密钥只显示 masked status。"
      />
      <div className="settings-grid">
        <section className="surface settings-section">
          <SurfaceHeading label="PROVIDER / PROFILE" title="运行选择" />
          {data.providers?.map((provider) => (
            <div className="setting-card" key={provider.id}>
              <div className="setting-line">
                <strong>{provider.name}</strong>
                <span
                  className={`status-tag ${provider.enabled ? "ready" : "neutral"}`}
                >
                  {provider.enabled ? "enabled" : "disabled"}
                </span>
              </div>
              <small className="mono">
                {provider.id} / rev {provider.revision} / {provider.adapterKey}
              </small>
              <button
                className="secondary-button"
                onClick={() =>
                  update.mutate({
                    id: provider.id,
                    revision: provider.revision,
                    kind: "provider",
                    changes: { enabled: !provider.enabled },
                  })
                }
              >
                <ToggleLeft size={15} /> {provider.enabled ? "停用" : "启用"}
              </button>
            </div>
          ))}
          {data.profiles?.map((profile) => (
            <div className="setting-card" key={profile.id}>
              <div className="setting-line">
                <strong>{profile.name}</strong>
                <span className="status-tag neutral">
                  {profile.credentialStatus}
                </span>
              </div>
              <small className="mono">
                {profile.id} / rev {profile.revision} / adapter{" "}
                {profile.adapterIdentity}
              </small>
              <div className="setting-actions">
                <button
                  className="secondary-button"
                  onClick={() =>
                    update.mutate({
                      id: profile.id,
                      revision: profile.revision,
                      kind: "profile",
                      changes: { enabled: !profile.enabled },
                    })
                  }
                >
                  <ToggleLeft size={15} /> {profile.enabled ? "停用" : "启用"}
                </button>
                <button
                  className="secondary-button"
                  onClick={() => probe.mutate(profile)}
                >
                  <RefreshCw size={15} /> 显式 probe
                </button>
                <button
                  className="secondary-button"
                  onClick={() => sync.mutate(profile)}
                >
                  <RotateCcw size={15} /> model sync
                </button>
              </div>
              <div className="credential-form">
                <KeyRound size={15} />
                <input
                  placeholder="credentialRef / ID"
                  value={credential.id}
                  onChange={(event) =>
                    setCredential((value) => ({
                      ...value,
                      id: event.target.value,
                    }))
                  }
                  autoComplete="off"
                />
                <input
                  placeholder="一次性输入 credential"
                  type="password"
                  value={credential.value}
                  onChange={(event) =>
                    setCredential((value) => ({
                      ...value,
                      value: event.target.value,
                    }))
                  }
                  autoComplete="new-password"
                />
                <button
                  className="primary-button"
                  disabled={
                    !credential.id.trim() ||
                    !credential.value ||
                    replace.isPending
                  }
                  onClick={() => replace.mutate(profile)}
                >
                  替换
                </button>
              </div>
              <div className="setting-note">
                <ShieldCheck size={14} /> credential value 不进入 UI state
                之外的持久化；真实主密钥缺失会返回 `503`。
              </div>
            </div>
          ))}
        </section>
        <section className="surface settings-section">
          <SurfaceHeading
            label="SKILL REGISTRY"
            title="候选与批准"
            trailing={<Workflow size={19} />}
          />
          {data.skills?.map((skill) => (
            <div className="skill-line" key={skill.id}>
              <span className="skill-dot" />
              <strong>{skill.name}</strong>
              <small className="mono">
                {skill.version} / rev {skill.revision} / {skill.sourceType}
              </small>
              <span
                className={`status-tag ${skill.approval === "approved" && skill.enabled ? "ready" : "neutral"}`}
              >
                {skill.approval}
                {skill.enabled ? " / enabled" : " / disabled"}
              </span>
            </div>
          ))}
          {syncCandidate && (
            <div className="sync-diff">
              <strong>Model sync candidate</strong>
              <span className="mono">
                {syncCandidate.id} / rev {syncCandidate.revision}
              </span>
              <span>
                added: {(syncCandidate.added ?? []).join(", ") || "none"}
              </span>
              <span>
                removed: {(syncCandidate.removed ?? []).join(", ") || "none"}
              </span>
              <button
                className="secondary-button"
                onClick={() => setSyncCandidate(null)}
              >
                取消人工同步
              </button>
            </div>
          )}
          {diagnostic && (
            <div className="warning-line">
              <CircleAlert size={14} /> {diagnostic}
            </div>
          )}
          {catalog.error && <ErrorNotice error={catalog.error} />}
        </section>
      </div>
      <span className="settings-scope mono">project scope: {projectId}</span>
    </section>
  );
}
