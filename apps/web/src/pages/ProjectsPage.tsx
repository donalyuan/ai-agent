import { useMutation, useQuery } from "@tanstack/react-query";
import {
  ArrowRight,
  Check,
  CircleCheck,
  FolderKanban,
  Plus,
  Settings2,
} from "lucide-react";
import { useState } from "react";
import { useNavigate } from "react-router";
import { queryClient } from "../app/query-client";
import { Button, Input } from "../shared/ui";
import { ErrorNotice, PageIntro, QueryNotice, SurfaceHeading } from "../ui";
import { OwnerApiError, queryKeys, workbenchApi } from "../workbench/api";

export function ProjectsPage() {
  const navigate = useNavigate();
  const projects = useQuery({
    queryKey: queryKeys.projects,
    queryFn: workbenchApi.listProjects,
  });
  const [name, setName] = useState("");
  const [editing, setEditing] = useState<{
    id: string;
    name: string;
    revision: number;
  } | null>(null);
  const create = useMutation({
    mutationFn: workbenchApi.createProject,
    onSuccess: (project) => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.projects });
      navigate(`/projects/${project.id}/workbench`);
    },
  });
  const update = useMutation({
    mutationFn: () =>
      editing
        ? workbenchApi.updateProject(
            editing.id,
            editing.name.trim(),
            editing.revision,
          )
        : Promise.reject(new Error("未选择项目")),
    onSuccess: () => {
      setEditing(null);
      void queryClient.invalidateQueries({ queryKey: queryKeys.projects });
    },
    onError: (error) => {
      if (error instanceof OwnerApiError && error.status === 409) {
        void queryClient.invalidateQueries({ queryKey: queryKeys.projects });
      }
    },
  });

  return (
    <section className="mx-auto flex w-full max-w-screen-2xl flex-col gap-6 p-4 sm:p-6 lg:p-8">
      <PageIntro
        eyebrow="项目"
        title="选择一个项目"
        detail="创建项目后，需显式进入工作台；不会自动建立剧集或工作流。"
      />
      <div
        className="grid items-start gap-6 lg:grid-cols-[minmax(18rem,0.75fr)_minmax(0,1.25fr)]"
        data-testid="project-index-grid"
      >
        <section className="grid content-start gap-4 rounded-lg border border-border bg-card p-5 shadow-sm">
          <SurfaceHeading
            label="新建项目"
            title="建立创作上下文"
            trailing={<Plus aria-hidden="true" className="size-5" />}
          />
          <label className="grid gap-1 text-sm font-medium text-foreground">
            项目名称
            <Input
              onChange={(event) => setName(event.target.value)}
              placeholder="例如：雾港来信"
              value={name}
            />
          </label>
          <Button
            className="w-full"
            disabled={!name.trim() || create.isPending}
            onClick={() => create.mutate(name.trim())}
          >
            {create.isPending ? "创建中..." : "创建并进入工作台"}
            <ArrowRight aria-hidden="true" />
          </Button>
          {create.error && <ErrorNotice error={create.error} />}
          <div className="flex items-center gap-2 rounded-md bg-muted px-3 py-2 text-sm text-muted-foreground">
            <CircleCheck aria-hidden="true" className="size-4" />
            默认测试组合：Mock Provider + Local test/offline
          </div>
        </section>
        <section className="grid content-start gap-4 rounded-lg border border-border bg-card p-5 shadow-sm">
          <SurfaceHeading
            label="项目"
            title="已有项目"
            trailing={
              <span className="rounded-md bg-secondary px-2 py-1 text-xs font-semibold text-secondary-foreground">
                {projects.data?.length ?? 0}
              </span>
            }
          />
          {projects.isPending && (
            <QueryNotice isPending error={null} empty="" />
          )}
          {projects.error && <ErrorNotice error={projects.error} />}
          {!projects.isPending &&
            !projects.error &&
            projects.data?.length === 0 && (
              <QueryNotice
                isPending={false}
                error={null}
                empty="尚无项目；创建第一个项目后即可进入工作台。"
              />
            )}
          <div className="grid gap-2">
            {projects.data?.map((project) => (
              <div
                className="flex items-stretch gap-2 rounded-md border border-border bg-muted/30 p-2"
                key={project.id}
              >
                <Button
                  className="h-auto min-h-14 flex-1 justify-start text-left"
                  onClick={() => navigate(`/projects/${project.id}/workbench`)}
                  variant="ghost"
                >
                  <span className="grid size-8 shrink-0 place-items-center rounded-md bg-primary text-primary-foreground">
                    <FolderKanban aria-hidden="true" className="size-4" />
                  </span>
                  <span className="grid min-w-0 gap-1">
                    <strong>{project.name}</strong>
                    <small className="font-mono text-xs text-muted-foreground">
                      {project.id} / rev {project.revision}
                    </small>
                  </span>
                  <ArrowRight aria-hidden="true" className="size-4" />
                </Button>
                <Button
                  aria-label="编辑项目名称"
                  onClick={() =>
                    setEditing({
                      id: project.id,
                      name: project.name,
                      revision: project.revision,
                    })
                  }
                  size="icon"
                  title="编辑项目名称"
                  variant="outline"
                >
                  <Settings2 aria-hidden="true" className="size-4" />
                </Button>
              </div>
            ))}
          </div>
          {editing && (
            <div className="grid gap-3 rounded-md border border-border bg-muted p-3 sm:grid-cols-[minmax(0,1fr)_auto_auto] sm:items-end">
              <label className="grid gap-1 text-sm font-medium text-foreground">
                项目名称
                <Input
                  onChange={(event) =>
                    setEditing({ ...editing, name: event.target.value })
                  }
                  value={editing.name}
                />
              </label>
              <Button onClick={() => setEditing(null)} variant="outline">
                取消
              </Button>
              <Button
                disabled={!editing.name.trim() || update.isPending}
                onClick={() => update.mutate()}
              >
                {update.isPending ? "保存中..." : "保存名称"}
                <Check aria-hidden="true" />
              </Button>
              {update.error && <ErrorNotice error={update.error} />}
            </div>
          )}
        </section>
      </div>
    </section>
  );
}
