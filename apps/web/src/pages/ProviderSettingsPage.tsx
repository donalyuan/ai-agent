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
import { zodResolver } from "@hookform/resolvers/zod";
import type { ColumnDef } from "@tanstack/react-table";
import { useMemo, useState } from "react";
import { useForm } from "react-hook-form";
import { toast } from "sonner";
import { z } from "zod";
import { settingsApi, settingsQueryKeys } from "../settings/api";
import { Badge, Button, DataTable, Input, Label } from "../shared/ui";
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
  parameterSchemas?: Record<
    string,
    { properties?: Record<string, { type?: string; minimum?: number }> }
  >;
  quotaSnapshots?: Record<
    string,
    { status: string; remaining?: number | null; source?: string }
  >;
};
type Model = {
  id: string;
  profileId: string;
  modelKey: string;
  enabled: boolean;
  revision: number;
  historicalReferences?: number;
};
type Skill = {
  id: string;
  name: string;
  version: string;
  approval: string;
  enabled: boolean;
  provenance: string;
  sourceType: string;
  revision: number;
  capabilities: string[];
};
type Catalog = {
  schemaVersion?: string;
  providers?: Provider[];
  profiles?: Profile[];
  models?: Model[];
  skills?: Skill[];
  profileParameterSchemas?: Record<
    string,
    NonNullable<Profile["parameterSchemas"]>
  >;
};

function ProfilePolicyForm({
  profile,
  onSave,
}: {
  profile: Profile;
  onSave: (changes: Record<string, unknown>) => void;
}) {
  const operations = Object.entries(profile.parameterSchemas ?? {});
  const [operation, schema] = operations[0] ?? [];
  const properties = schema?.properties ?? {};
  const policySchema = z.object(
    Object.fromEntries(
      Object.entries(properties).map(([key, definition]) => [
        key,
        definition.type === "number" || definition.type === "integer"
          ? z.coerce
              .number()
              .int()
              .min(definition.minimum ?? 0)
          : z.string().min(1),
      ]),
    ),
  );
  const form = useForm<Record<string, unknown>>({
    resolver: zodResolver(policySchema),
    defaultValues: {
      ...(operation ? profile.operationPolicies?.[operation] : {}),
    },
  });
  const submit = form.handleSubmit((value) => {
    if (!operation) return;
    onSave({
      operationPolicies: {
        ...(profile.operationPolicies ?? {}),
        [operation]: value,
      },
    });
  });

  if (!operation || Object.keys(properties).length === 0) {
    return (
      <div className="mt-3 border-t border-border pt-3 text-sm text-muted-foreground">
        owner parameter schema unavailable；不会猜测字段或提交 policy。
      </div>
    );
  }

  return (
    <form
      className="mt-3 grid gap-2 border-t border-border pt-3"
      onSubmit={submit}
    >
      <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
        OWNER POLICY PARAMETERS
      </span>
      <div className="grid gap-2 sm:grid-cols-3">
        {Object.entries(properties).map(([key, definition]) => (
          <label className="grid gap-1" key={key}>
            <Label htmlFor={`${profile.id}-${key}`}>{key}</Label>
            <Input
              id={`${profile.id}-${key}`}
              type={
                definition.type === "number" || definition.type === "integer"
                  ? "number"
                  : "text"
              }
              {...form.register(key)}
            />
          </label>
        ))}
      </div>
      {Object.entries(form.formState.errors).map(([key, error]) =>
        error?.message ? (
          <small className="text-sm text-destructive" key={key}>
            {error.message}
          </small>
        ) : null,
      )}
      <div>
        <Button size="sm" type="submit">
          保存 owner policy
        </Button>
      </div>
    </form>
  );
}

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
    source?: string;
    discovery?: string;
  } | null>(null);
  const [remoteModels, setRemoteModels] = useState("");
  const [diagnostic, setDiagnostic] = useState("");
  const data = (catalog.data ?? {}) as Catalog;
  const providerColumns = useMemo<ColumnDef<Provider, unknown>[]>(
    () => [
      { accessorKey: "name", header: "Provider" },
      { accessorKey: "adapterKey", header: "Adapter" },
      { accessorKey: "revision", header: "Revision" },
      { accessorKey: "approval", header: "Approval" },
      {
        accessorKey: "enabled",
        header: "状态",
        cell: ({ getValue }) => (
          <Badge variant={getValue<boolean>() ? "success" : "secondary"}>
            {getValue<boolean>() ? "enabled" : "disabled"}
          </Badge>
        ),
      },
    ],
    [],
  );
  const skillColumns = useMemo<
    ColumnDef<NonNullable<Catalog["skills"]>[number], unknown>[]
  >(
    () => [
      { accessorKey: "name", header: "Skill" },
      { accessorKey: "version", header: "Version" },
      { accessorKey: "sourceType", header: "Source" },
      { accessorKey: "revision", header: "Revision" },
      { accessorKey: "approval", header: "Approval" },
    ],
    [],
  );
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
    onSuccess: () => {
      toast.success("Catalog owner 已保存 revision");
      return client.invalidateQueries({ queryKey: settingsQueryKeys.catalog });
    },
    onError: async (error) => {
      setDiagnostic(error instanceof Error ? error.message : "更新失败");
      if (error instanceof Error && "status" in error && error.status === 409)
        await client.refetchQueries({ queryKey: settingsQueryKeys.catalog });
    },
  });
  const lifecycle = useMutation({
    mutationFn: ({
      id,
      revision,
      enabled,
      kind,
    }: {
      id: string;
      revision: number;
      enabled: boolean;
      kind: "provider" | "profile" | "model" | "skill";
    }) => {
      if (kind === "provider")
        return settingsApi.setProviderEnabled(id, revision, enabled);
      if (kind === "profile")
        return settingsApi.setProfileEnabled(id, revision, enabled);
      if (kind === "model")
        return settingsApi.setModelEnabled(id, revision, enabled);
      return settingsApi.setSkillEnabled(id, revision, enabled);
    },
    onSuccess: async () => {
      toast.success("Catalog owner lifecycle 已保存 revision");
      await client.invalidateQueries({ queryKey: settingsQueryKeys.catalog });
    },
    onError: async (error) => {
      setDiagnostic(
        error instanceof Error ? error.message : "lifecycle 更新失败",
      );
      if (error instanceof Error && "status" in error && error.status === 409)
        await client.refetchQueries({ queryKey: settingsQueryKeys.catalog });
    },
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
      settingsApi.probe(
        profile.id,
        profile.revision,
        Object.keys(profile.operationPolicies ?? {})[0] ?? "image.generate",
      ),
    onSuccess: (value) => setDiagnostic(`probe: ${JSON.stringify(value)}`),
    onError: (error) =>
      setDiagnostic(error instanceof Error ? error.message : "probe failed"),
  });
  const sync = useMutation({
    mutationFn: (profile: Profile) => {
      const candidates = remoteModels
        .split(",")
        .map((item) => item.trim())
        .filter(Boolean);
      if (candidates.length === 0)
        throw new Error("请输入显式候选模型 key（不执行远程 discovery）");
      return settingsApi.syncModels(profile.id, profile.revision, candidates);
    },
    onSuccess: (value) => setSyncCandidate(value as typeof syncCandidate),
    onError: (error) =>
      setDiagnostic(error instanceof Error ? error.message : "sync failed"),
  });
  const decideSync = useMutation({
    mutationFn: ({ decision }: { decision: "accept" | "reject" }) => {
      if (!syncCandidate) throw new Error("没有待确认的 model sync proposal");
      return settingsApi.decideSync(
        syncCandidate.id,
        syncCandidate.revision,
        decision,
      );
    },
    onSuccess: async () => {
      setSyncCandidate(null);
      await client.invalidateQueries({ queryKey: settingsQueryKeys.catalog });
    },
    onError: async (error) => {
      setDiagnostic(
        error instanceof Error ? error.message : "sync decision failed",
      );
      if (error instanceof Error && "status" in error && error.status === 409)
        await client.refetchQueries({ queryKey: settingsQueryKeys.catalog });
    },
  });
  const modelColumns = useMemo<ColumnDef<Model, unknown>[]>(
    () => [
      { accessorKey: "modelKey", header: "Model" },
      { accessorKey: "profileId", header: "Profile" },
      { accessorKey: "revision", header: "Revision" },
      {
        accessorKey: "enabled",
        header: "状态",
        cell: ({ row, getValue }) => {
          const enabled = getValue<boolean>();
          return (
            <Button
              size="sm"
              variant={enabled ? "secondary" : "outline"}
              onClick={() =>
                lifecycle.mutate({
                  id: row.original.id,
                  revision: row.original.revision,
                  enabled: !enabled,
                  kind: "model",
                })
              }
            >
              <ToggleLeft size={15} /> {enabled ? "停用" : "启用"}
            </Button>
          );
        },
      },
    ],
    [lifecycle],
  );
  if (catalog.isPending)
    return (
      <section className="mx-auto flex w-full max-w-screen-2xl flex-col gap-6 p-4 sm:p-6 lg:p-8">
        <PageIntro
          eyebrow="PROJECT SETTINGS / CATALOG"
          title="模型与能力"
          detail="只读取 owner catalog，不隐式启用真实调用。"
        />
        <QueryNotice isPending error={null} empty="" />
      </section>
    );
  return (
    <section className="mx-auto flex w-full max-w-screen-2xl flex-col gap-6 p-4 sm:p-6 lg:p-8 gap-5">
      <PageIntro
        eyebrow="PROJECT SETTINGS / CATALOG"
        title="模型与能力"
        detail="Provider、Model、Skill 和 StorageProfile 都以 owner revision/CAS 管理；密钥只显示 masked status。"
      />
      <div className="grid gap-6 xl:grid-cols-2">
        <section className="rounded-lg border border-border bg-card p-5 shadow-sm rounded-lg border border-border bg-card p-5 shadow-sm">
          <SurfaceHeading label="PROVIDER / PROFILE" title="运行选择" />
          {data.providers?.map((provider) => (
            <div
              className="rounded-md border border-border bg-muted p-3"
              key={provider.id}
            >
              <div className="flex flex-wrap items-center justify-between gap-3 border-t border-border py-3 text-sm">
                <strong>{provider.name}</strong>
                <span
                  className={`inline-flex rounded-full px-2 py-1 text-xs font-semibold ${provider.enabled ? "bg-success/10 text-success" : "bg-muted text-muted-foreground"}`}
                >
                  {provider.enabled ? "enabled" : "disabled"}
                </span>
              </div>
              <small className="font-mono text-xs text-muted-foreground">
                {provider.id} / rev {provider.revision} / {provider.adapterKey}
              </small>
              <button
                className="inline-flex h-10 items-center justify-center gap-2 rounded-md border border-border bg-background px-4 text-sm font-semibold text-foreground hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                onClick={() =>
                  lifecycle.mutate({
                    id: provider.id,
                    revision: provider.revision,
                    kind: "provider",
                    enabled: !provider.enabled,
                  })
                }
              >
                <ToggleLeft size={15} /> {provider.enabled ? "停用" : "启用"}
              </button>
            </div>
          ))}
          <DataTable
            columns={providerColumns}
            data={data.providers ?? []}
            emptyLabel="没有 Provider projection"
            filterPlaceholder="筛选 Provider / adapter"
            getRowId={(row) => row.id}
          />
          {data.profiles?.map((profile) => (
            <div
              className="rounded-md border border-border bg-muted p-3"
              key={profile.id}
            >
              <div className="flex flex-wrap items-center justify-between gap-3 border-t border-border py-3 text-sm">
                <strong>{profile.name}</strong>
                <span className="inline-flex rounded-full bg-muted px-2 py-1 text-xs font-semibold text-muted-foreground">
                  {profile.credentialStatus}
                </span>
              </div>
              <small className="font-mono text-xs text-muted-foreground">
                {profile.id} / rev {profile.revision} / adapter{" "}
                {profile.adapterIdentity}
              </small>
              <div className="flex flex-wrap items-center gap-2">
                <button
                  className="inline-flex h-10 items-center justify-center gap-2 rounded-md border border-border bg-background px-4 text-sm font-semibold text-foreground hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                  onClick={() =>
                    lifecycle.mutate({
                      id: profile.id,
                      revision: profile.revision,
                      kind: "profile",
                      enabled: !profile.enabled,
                    })
                  }
                >
                  <ToggleLeft size={15} /> {profile.enabled ? "停用" : "启用"}
                </button>
                <button
                  className="inline-flex h-10 items-center justify-center gap-2 rounded-md border border-border bg-background px-4 text-sm font-semibold text-foreground hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                  onClick={() => probe.mutate(profile)}
                >
                  <RefreshCw size={15} /> 显式 probe
                </button>
                <button
                  className="inline-flex h-10 items-center justify-center gap-2 rounded-md border border-border bg-background px-4 text-sm font-semibold text-foreground hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                  onClick={() => sync.mutate(profile)}
                >
                  <RotateCcw size={15} /> model sync
                </button>
              </div>
              <label className="grid gap-1 text-sm">
                <span>显式候选模型输入（不执行远程 discovery）</span>
                <Input
                  aria-label="Remote model candidates"
                  value={remoteModels}
                  onChange={(event) => setRemoteModels(event.target.value)}
                  placeholder="model-a, model-b（仅用于 owner diff）"
                />
              </label>
              <div className="grid gap-3 rounded-md border border-border p-4">
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
                  className="inline-flex h-10 items-center justify-center gap-2 rounded-md bg-primary px-4 text-sm font-semibold text-primary-foreground hover:bg-primary/90 disabled:pointer-events-none disabled:opacity-50"
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
              <div className="mt-2 text-sm text-muted-foreground">
                <ShieldCheck size={14} /> credential value 不进入 UI state
                之外的持久化；真实主密钥缺失会返回 `503`。
              </div>
              <ProfilePolicyForm
                profile={{
                  ...profile,
                  parameterSchemas: data.profileParameterSchemas?.[profile.id],
                }}
                onSave={(changes) =>
                  update.mutate({
                    id: profile.id,
                    revision: profile.revision,
                    kind: "profile",
                    changes,
                  })
                }
              />
            </div>
          ))}
        </section>
        <section className="rounded-lg border border-border bg-card p-5 shadow-sm rounded-lg border border-border bg-card p-5 shadow-sm">
          <SurfaceHeading label="MODEL CATALOG" title="模型生命周期" />
          <DataTable
            columns={modelColumns}
            data={data.models ?? []}
            emptyLabel="没有 Model projection"
            filterPlaceholder="筛选 Model / profile"
            getRowId={(row) => row.id}
          />
        </section>
        <section className="rounded-lg border border-border bg-card p-5 shadow-sm rounded-lg border border-border bg-card p-5 shadow-sm">
          <SurfaceHeading
            label="SKILL REGISTRY"
            title="候选与批准"
            trailing={<Workflow size={19} />}
          />
          {data.skills?.map((skill) => (
            <div
              className="flex flex-wrap items-center justify-between gap-3 rounded-md border border-border p-3"
              key={skill.id}
            >
              <span className="size-2 rounded-full bg-primary" />
              <strong>{skill.name}</strong>
              <small className="font-mono text-xs text-muted-foreground">
                {skill.version} / rev {skill.revision} / {skill.sourceType}
              </small>
              <span
                className={`inline-flex rounded-full px-2 py-1 text-xs font-semibold ${skill.approval === "approved" && skill.enabled ? "bg-success/10 text-success" : "bg-muted text-muted-foreground"}`}
              >
                {skill.approval}
                {skill.enabled ? " / enabled" : " / disabled"}
              </span>
              <button
                className="inline-flex h-10 items-center justify-center gap-2 rounded-md border border-border bg-background px-4 text-sm font-semibold text-foreground hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                onClick={() =>
                  lifecycle.mutate({
                    id: skill.id,
                    revision: skill.revision,
                    enabled: !skill.enabled,
                    kind: "skill",
                  })
                }
              >
                <ToggleLeft size={15} /> {skill.enabled ? "停用" : "启用"}
              </button>
            </div>
          ))}
          <DataTable
            columns={skillColumns}
            data={data.skills ?? []}
            emptyLabel="没有 Skill projection"
            filterPlaceholder="筛选 Skill / provenance"
            getRowId={(row) => row.id}
          />
          {syncCandidate && (
            <div className="mt-3 rounded-md border border-border bg-muted p-3 text-sm">
              <strong>Model sync candidate</strong>
              <span className="font-mono text-xs text-muted-foreground">
                {syncCandidate.id} / rev {syncCandidate.revision}
              </span>
              <span>
                source: {syncCandidate.source ?? "explicit_input"} / discovery:{" "}
                {syncCandidate.discovery ?? "not_performed"}
              </span>
              <span>
                added: {(syncCandidate.added ?? []).join(", ") || "none"}
              </span>
              <span>
                removed: {(syncCandidate.removed ?? []).join(", ") || "none"}
              </span>
              <button
                className="inline-flex h-10 items-center justify-center gap-2 rounded-md border border-border bg-background px-4 text-sm font-semibold text-foreground hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                disabled={decideSync.isPending}
                onClick={() => decideSync.mutate({ decision: "reject" })}
              >
                拒绝 proposal
              </button>
              <button
                className="inline-flex h-10 items-center justify-center gap-2 rounded-md bg-primary px-4 text-sm font-semibold text-primary-foreground hover:bg-primary/90 disabled:pointer-events-none disabled:opacity-50"
                disabled={decideSync.isPending}
                onClick={() => decideSync.mutate({ decision: "accept" })}
              >
                接受 proposal
              </button>
            </div>
          )}
          {diagnostic && (
            <div className="flex items-start gap-2 rounded-md border border-warning/30 bg-warning/10 p-3 text-sm text-warning-foreground">
              <CircleAlert size={14} /> {diagnostic}
            </div>
          )}
          {catalog.error && <ErrorNotice error={catalog.error} />}
        </section>
      </div>
      <span className="rounded-md bg-muted px-3 py-2 font-mono text-xs text-muted-foreground">
        project scope: {projectId}
      </span>
    </section>
  );
}
