import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { CircleAlert, LockKeyhole, PlugZap, Save } from "lucide-react";
import { useState } from "react";
import { useParams } from "react-router";
import { settingsApi, settingsQueryKeys } from "../settings/api";
import { ErrorNotice, PageIntro, QueryNotice } from "../ui";

type Profile = {
  storageProfileId: string;
  revision: number;
  name: string;
  endpoint: string;
  bucket: string;
  region: string;
  adapterKey: string;
  privateBucket: boolean;
  bucketBindingId: string;
  credentialRef?: string | null;
  credentialStatus: string;
  credentialSummary?: string | null;
  enabled: boolean;
  projectScope: string[];
  connectTimeoutMs: number;
  readTimeoutMs: number;
  writeTimeoutMs: number;
  presignMaxTtlSeconds: number;
};

export function StorageProfilePage() {
  const { projectId = "", storageProfileId = "" } = useParams();
  const client = useQueryClient();
  const [draft, setDraft] = useState<Partial<Profile>>({});
  const [diagnostic, setDiagnostic] = useState("");
  const profile = useQuery({
    queryKey: settingsQueryKeys.storageProfile(storageProfileId),
    queryFn: () => settingsApi.storage(storageProfileId),
    enabled: Boolean(storageProfileId),
  });
  const update = useMutation({
    mutationFn: (body: Record<string, unknown>) =>
      settingsApi.updateStorage(
        storageProfileId,
        (profile.data as Profile).revision,
        body,
      ),
    onSuccess: async () => {
      setDiagnostic("StorageProfile 已更新");
      await client.invalidateQueries({
        queryKey: settingsQueryKeys.storageProfile(storageProfileId),
      });
    },
    onError: (error) =>
      setDiagnostic(error instanceof Error ? error.message : "更新失败"),
  });
  const toggle = useMutation({
    mutationFn: (enabled: boolean) =>
      enabled
        ? settingsApi.enableStorage(
            storageProfileId,
            (profile.data as Profile).revision,
          )
        : settingsApi.disableStorage(
            storageProfileId,
            (profile.data as Profile).revision,
          ),
    onSuccess: () =>
      client.invalidateQueries({
        queryKey: settingsQueryKeys.storageProfile(storageProfileId),
      }),
    onError: (error) =>
      setDiagnostic(error instanceof Error ? error.message : "启停失败"),
  });
  const test = useMutation({
    mutationFn: () =>
      settingsApi.testStorage(
        storageProfileId,
        (profile.data as Profile).revision,
        crypto.randomUUID(),
      ),
    onSuccess: (value) =>
      setDiagnostic(`connection-test: ${JSON.stringify(value)}`),
    onError: (error) =>
      setDiagnostic(
        error instanceof Error ? error.message : "connection-test failed",
      ),
  });
  if (!storageProfileId)
    return (
      <section className="mx-auto flex w-full max-w-screen-2xl flex-col gap-6 p-4 sm:p-6 lg:p-8">
        <PageIntro
          eyebrow="STORAGE PROFILE"
          title="需要显式选择 profile"
          detail="不会从项目中推断或创建 StorageProfile。"
        />
      </section>
    );
  return (
    <section className="mx-auto flex w-full max-w-screen-2xl flex-col gap-6 p-4 sm:p-6 lg:p-8">
      <PageIntro
        eyebrow="STORAGE PROFILE / OWNER"
        title="存储连接"
        detail="TOS/private bucket 仅显式测试；缺少凭据或主密钥时保留 unconfigured/503，不 fallback Local。"
      />
      {profile.isPending && <QueryNotice isPending error={null} empty="" />}
      {profile.error && <ErrorNotice error={profile.error} />}
      {Boolean(profile.data) &&
        (() => {
          const value = profile.data as Profile;
          const get = <K extends keyof Profile>(key: K) =>
            draft[key] ?? value[key];
          return (
            <section className="rounded-lg border border-border bg-card p-5 shadow-sm rounded-lg border border-border bg-card p-5 shadow-sm">
              <div className="rounded-md border border-border bg-muted p-3 text-sm text-muted-foreground">
                <LockKeyhole size={15} /> credential:{" "}
                <strong>{value.credentialStatus}</strong>{" "}
                {value.credentialSummary ?? ""}
              </div>
              <label className="grid gap-1 text-sm">
                <span>名称</span>
                <input
                  value={String(get("name"))}
                  onChange={(event) =>
                    setDraft({ ...draft, name: event.target.value })
                  }
                />
              </label>
              <label className="grid gap-1 text-sm">
                <span>Endpoint</span>
                <input
                  value={String(get("endpoint"))}
                  onChange={(event) =>
                    setDraft({ ...draft, endpoint: event.target.value })
                  }
                />
              </label>
              <label className="grid gap-1 text-sm">
                <span>Bucket</span>
                <input
                  value={String(get("bucket"))}
                  onChange={(event) =>
                    setDraft({ ...draft, bucket: event.target.value })
                  }
                />
              </label>
              <label className="grid gap-1 text-sm">
                <span>Region</span>
                <input
                  value={String(get("region"))}
                  onChange={(event) =>
                    setDraft({ ...draft, region: event.target.value })
                  }
                />
              </label>
              <label className="grid gap-1 text-sm">
                <span>Presign TTL</span>
                <input
                  type="number"
                  min="1"
                  max="300"
                  value={Number(get("presignMaxTtlSeconds"))}
                  onChange={(event) =>
                    setDraft({
                      ...draft,
                      presignMaxTtlSeconds: Number(event.target.value),
                    })
                  }
                />
              </label>
              <div className="flex flex-wrap gap-2">
                <button
                  className="inline-flex h-10 items-center justify-center gap-2 rounded-md bg-primary px-4 text-sm font-semibold text-primary-foreground hover:bg-primary/90 disabled:pointer-events-none disabled:opacity-50"
                  onClick={() =>
                    update.mutate({
                      name: get("name"),
                      endpoint: get("endpoint"),
                      bucket: get("bucket"),
                      region: get("region"),
                      adapterKey: value.adapterKey,
                      privateBucket: value.privateBucket,
                      bucketBindingId: value.bucketBindingId,
                      credentialRef: value.credentialRef ?? null,
                      enabled: value.enabled,
                      connectTimeoutMs: value.connectTimeoutMs,
                      readTimeoutMs: value.readTimeoutMs,
                      writeTimeoutMs: value.writeTimeoutMs,
                      presignMaxTtlSeconds: get("presignMaxTtlSeconds"),
                      projectScope: value.projectScope,
                    })
                  }
                >
                  <Save size={15} /> 保存
                </button>
                <button
                  className="inline-flex h-10 items-center justify-center gap-2 rounded-md border border-border bg-background px-4 text-sm font-semibold text-foreground hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                  onClick={() => toggle.mutate(!value.enabled)}
                >
                  {value.enabled ? "停用" : "启用"}
                </button>
                <button
                  className="inline-flex h-10 items-center justify-center gap-2 rounded-md border border-border bg-background px-4 text-sm font-semibold text-foreground hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                  onClick={() => test.mutate()}
                >
                  <PlugZap size={15} /> connection-test
                </button>
              </div>
              {diagnostic && (
                <div className="flex items-start gap-2 rounded-md border border-destructive/30 bg-destructive/10 p-3 text-sm text-destructive">
                  <CircleAlert size={14} /> {diagnostic}
                </div>
              )}
              <small className="font-mono text-xs text-muted-foreground">
                {value.storageProfileId} / rev {value.revision} / project{" "}
                {projectId}
              </small>
            </section>
          );
        })()}
    </section>
  );
}
