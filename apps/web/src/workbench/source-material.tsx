import { FileText, ArrowRight, CircleCheck } from "lucide-react";
import { useState } from "react";
import { useMutation } from "@tanstack/react-query";
import { Button, Input, Textarea } from "../shared/ui";
import { workbenchApi } from "./api";
import { inputModeLabel, materialTypeLabel } from "./display";
import { WorkbenchNotice, WorkbenchPanel } from "./ui";

type MaterialType = "novel" | "synopsis" | "existing_script";
type InputMode = "inline_text" | "uploaded_file";

export function SourceMaterialPanel({ projectId }: { projectId: string }) {
  const [materialType, setMaterialType] = useState<MaterialType>("novel");
  const [inputMode, setInputMode] = useState<InputMode>("inline_text");
  const [content, setContent] = useState("");
  const [assetVersionId, setAssetVersionId] = useState("");
  const [projection, setProjection] = useState<Record<string, unknown> | null>(
    null,
  );
  const create = useMutation({
    mutationFn: async () => {
      if (inputMode === "inline_text" && !content.trim())
        throw new Error("请先粘贴需要解析的文本。");
      if (inputMode === "uploaded_file" && !assetVersionId.trim())
        throw new Error("请先填写已验证文件的版本编号。");
      const source = (await workbenchApi.createSourceMaterial(
        projectId,
        materialType,
        inputMode,
      )) as {
        id?: string;
        revision?: number;
      };
      if (!source.id || !source.revision)
        throw new Error("来源材料服务返回的数据缺少版本信息。");
      return workbenchApi.appendSourceMaterial(
        source.id,
        source.revision,
        inputMode,
        inputMode === "inline_text" ? content : null,
        inputMode === "uploaded_file" ? assetVersionId.trim() : null,
      );
    },
    onSuccess: (value) => setProjection(value as Record<string, unknown>),
  });

  return (
    <WorkbenchPanel className="border-dashed bg-muted/30 shadow-none">
      <div className="flex items-center gap-2 px-4 pt-4 text-sm font-semibold">
        <FileText aria-hidden="true" className="size-4 text-primary" />
        <span>来源材料</span>
        <span className="text-xs font-normal text-muted-foreground">
          需要明确导入
        </span>
      </div>
      <div className="grid gap-4 p-4">
        <div className="grid gap-3 sm:grid-cols-2">
          <label className="grid gap-1.5 text-sm font-medium">
            材料类型
            <select
              className="h-10 rounded-md border border-input bg-background px-3 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
              value={materialType}
              onChange={(event) =>
                setMaterialType(event.target.value as MaterialType)
              }
              aria-label="材料类型"
            >
              <option value="novel">{materialTypeLabel("novel")}</option>
              <option value="synopsis">{materialTypeLabel("synopsis")}</option>
              <option value="existing_script">
                {materialTypeLabel("existing_script")}
              </option>
            </select>
          </label>
          <label className="grid gap-1.5 text-sm font-medium">
            输入方式
            <select
              className="h-10 rounded-md border border-input bg-background px-3 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
              value={inputMode}
              onChange={(event) =>
                setInputMode(event.target.value as InputMode)
              }
              aria-label="输入方式"
            >
              <option value="inline_text">
                {inputModeLabel("inline_text")}
              </option>
              <option value="uploaded_file">
                {inputModeLabel("uploaded_file")}
              </option>
            </select>
          </label>
        </div>
        {inputMode === "inline_text" ? (
          <Textarea
            value={content}
            onChange={(event) => setContent(event.target.value)}
            placeholder="粘贴待解析文本；不会创建文件上传会话"
            aria-label="来源材料文本"
          />
        ) : (
          <Input
            value={assetVersionId}
            onChange={(event) => setAssetVersionId(event.target.value)}
            placeholder="填写已验证文件的版本编号"
            aria-label="来源文件版本编号"
          />
        )}
        <p className="text-xs leading-5 text-muted-foreground">
          来源材料先由服务创建不可变版本，再执行解析与校验。文件本体必须先由资产服务完成上传。
        </p>
        <Button
          className="w-fit"
          variant="outline"
          disabled={create.isPending}
          onClick={() => create.mutate()}
        >
          {create.isPending ? "正在导入…" : "导入并校验"}{" "}
          <ArrowRight aria-hidden="true" />
        </Button>
        {projection && (
          <WorkbenchNotice tone="success">
            <span className="inline-flex items-center gap-2">
              <CircleCheck aria-hidden="true" className="size-4" />{" "}
              已返回来源材料版本，仍需项目服务明确绑定。
            </span>
          </WorkbenchNotice>
        )}
        {create.error && (
          <WorkbenchNotice tone="danger">
            {create.error instanceof Error
              ? create.error.message
              : "导入失败，请重试。"}
          </WorkbenchNotice>
        )}
      </div>
    </WorkbenchPanel>
  );
}
