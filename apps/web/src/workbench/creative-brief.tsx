import { Check, FileText } from "lucide-react";
import { useState } from "react";
import { useMutation } from "@tanstack/react-query";
import { Button, CardContent, Input } from "../shared/ui";
import { OwnerApiError, queryKeys, workbenchApi } from "./api";
import { creativeBriefCommandSchema } from "./contracts";
import type { CreativeProjection } from "./contracts";
import { queryClient } from "../app/query-client";
import { formatRevision } from "./display";
import { SourceMaterialPanel } from "./source-material";
import {
  WorkbenchNotice,
  WorkbenchPanel,
  WorkbenchPanelHeader,
  WorkbenchQueryNotice,
} from "./ui";

export type BriefDraft = {
  subject: string;
  genre: string;
  audience: string;
  characterPremise: string;
  style: string;
  episodeDurationSeconds: number;
  episodeCount: number;
  scenesPerEpisode: number;
  shotsPerScene: number;
};

export const emptyBrief: BriefDraft = {
  subject: "",
  genre: "",
  audience: "",
  characterPremise: "",
  style: "",
  episodeDurationSeconds: 60,
  episodeCount: 1,
  scenesPerEpisode: 1,
  shotsPerScene: 1,
};

function briefToDraft(
  creativeBrief: CreativeProjection["creativeBrief"] | undefined,
): BriefDraft {
  if (!creativeBrief) return emptyBrief;
  return {
    subject: creativeBrief.subject,
    genre: creativeBrief.genre,
    audience: creativeBrief.audience,
    characterPremise: creativeBrief.characterPremise,
    style: creativeBrief.style,
    episodeDurationSeconds: creativeBrief.episodeDurationSeconds,
    episodeCount: creativeBrief.episodeCount,
    scenesPerEpisode: creativeBrief.scenesPerEpisode,
    shotsPerScene: creativeBrief.shotsPerScene,
  };
}

export function CreativeBriefPanel({
  projectId,
  creative,
}: {
  projectId: string;
  creative: { data?: CreativeProjection; isPending: boolean; error: unknown };
}) {
  const [modeOverride, setModeOverride] = useState<
    "original" | "adaptation" | null
  >(null);
  const [draftOverride, setDraftOverride] = useState<BriefDraft | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const currentBrief = creative.data?.creativeBrief;
  const mode = modeOverride ?? creative.data?.creationMode ?? "original";
  const draft = draftOverride ?? briefToDraft(currentBrief);
  const save = useMutation({
    mutationFn: () => {
      const expectedRevision = creative.data?.projectRevision ?? 0;
      const parsed = creativeBriefCommandSchema.safeParse({
        creationMode: mode,
        ...draft,
        schemaVersion: "1.0.0" as const,
        expectedRevision,
        expectedBriefRevision: currentBrief?.revision ?? null,
      });
      if (!parsed.success)
        throw new OwnerApiError(
          422,
          "creative_brief_contract_invalid",
          "创作简报字段不完整或无效。",
        );
      return workbenchApi.saveBrief(projectId, parsed.data, expectedRevision);
    },
    onSuccess: () => {
      setModeOverride(null);
      setDraftOverride(null);
      setMessage("创作简报已保存。");
      void queryClient.invalidateQueries({
        queryKey: queryKeys.creative(projectId),
      });
    },
    onError: (error) => {
      if (error instanceof OwnerApiError && error.status === 409) {
        setModeOverride(null);
        setDraftOverride(null);
        void queryClient.invalidateQueries({
          queryKey: queryKeys.creative(projectId),
        });
      }
      setMessage(error instanceof Error ? error.message : "保存失败，请重试。");
    },
  });
  const updateDraft = <K extends keyof BriefDraft>(
    field: K,
    value: BriefDraft[K],
  ) => setDraftOverride({ ...draft, [field]: value });

  return (
    <WorkbenchPanel>
      <WorkbenchPanelHeader
        icon={<FileText aria-hidden="true" className="size-4" />}
        label="创作设定"
        title={currentBrief?.subject ?? "尚未保存创作简报"}
        detail="项目数据会按当前版本提交，版本冲突会保留并重新读取。"
        trailing={
          <span className="text-xs text-muted-foreground">
            {formatRevision(creative.data?.projectRevision)}
          </span>
        }
      />
      <CardContent className="grid gap-5">
        <WorkbenchQueryNotice
          isPending={creative.isPending}
          error={creative.error}
        />
        <div
          className="grid w-fit grid-cols-2 rounded-md border border-border bg-muted p-1"
          role="tablist"
          aria-label="创作方式"
        >
          <Button
            size="sm"
            variant={mode === "original" ? "default" : "ghost"}
            onClick={() => setModeOverride("original")}
            role="tab"
            aria-selected={mode === "original"}
          >
            原创
          </Button>
          <Button
            size="sm"
            variant={mode === "adaptation" ? "default" : "ghost"}
            onClick={() => setModeOverride("adaptation")}
            role="tab"
            aria-selected={mode === "adaptation"}
          >
            改编
          </Button>
        </div>
        <div className="grid gap-4 sm:grid-cols-2">
          {(
            [
              ["主题", "subject"],
              ["题材", "genre"],
              ["受众", "audience"],
              ["人物设想", "characterPremise"],
              ["风格", "style"],
            ] as const
          ).map(([label, field]) => (
            <label className="grid gap-1.5 text-sm font-medium" key={field}>
              {label}
              <Input
                value={draft[field]}
                onChange={(event) => updateDraft(field, event.target.value)}
                placeholder={`填写${label}`}
              />
            </label>
          ))}
        </div>
        <div className="grid gap-4 sm:grid-cols-4">
          {(
            [
              ["每集时长（秒）", "episodeDurationSeconds"],
              ["集数", "episodeCount"],
              ["每集场数", "scenesPerEpisode"],
              ["每场镜头数", "shotsPerScene"],
            ] as const
          ).map(([label, field]) => (
            <label className="grid gap-1.5 text-sm font-medium" key={field}>
              {label}
              <Input
                type="number"
                min="1"
                value={draft[field]}
                onChange={(event) =>
                  updateDraft(field, Number(event.target.value))
                }
              />
            </label>
          ))}
        </div>
        {mode === "adaptation" && <SourceMaterialPanel projectId={projectId} />}
        <div className="flex flex-wrap items-center justify-between gap-3 border-t border-border pt-4">
          <span className="text-xs text-muted-foreground">
            数据格式 1.0 · 当前简报 {formatRevision(currentBrief?.revision)}
          </span>
          <Button
            variant="outline"
            disabled={!creative.data || save.isPending || !draft.subject.trim()}
            onClick={() => save.mutate()}
          >
            {save.isPending ? "保存中…" : "保存创作简报"}{" "}
            <Check aria-hidden="true" />
          </Button>
        </div>
        {message && (
          <WorkbenchNotice tone={save.error ? "danger" : "success"}>
            {message}
          </WorkbenchNotice>
        )}
      </CardContent>
    </WorkbenchPanel>
  );
}
