import {
  Activity,
  ArrowLeft,
  ArrowRight,
  Boxes,
  CheckCircle2,
  Clapperboard,
  FileCheck2,
  FolderKanban,
  LoaderCircle,
  Settings2,
} from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { Link, NavLink, useLocation, useNavigate } from "react-router";
import { Badge, Button } from "../shared/ui";

type Readiness = "checking" | "ready" | "unavailable";

type ProjectShellProps = {
  children: ReactNode;
};

const navigationFor = (projectId: string) =>
  projectId
    ? [
        {
          label: "项目工作台",
          icon: FolderKanban,
          to: `/projects/${projectId}/workbench`,
        },
        {
          label: "候选审核",
          icon: FileCheck2,
          to: `/projects/${projectId}/review`,
        },
        { label: "项目资产", icon: Boxes, to: `/projects/${projectId}/assets` },
        {
          label: "集时间线",
          icon: Activity,
          to: `/projects/${projectId}/episodes/select/timeline`,
        },
        {
          label: "项目导出",
          icon: ArrowRight,
          to: `/projects/${projectId}/exports`,
        },
        {
          label: "模型设置",
          icon: Settings2,
          to: `/projects/${projectId}/settings`,
        },
      ]
    : [{ label: "项目入口", icon: FolderKanban, to: "/projects" }];

function useReadiness() {
  const [state, setState] = useState<Readiness>("checking");

  useEffect(() => {
    const controller = new AbortController();
    fetch("/api/v1/health/ready", { signal: controller.signal })
      .then((response) => setState(response.ok ? "ready" : "unavailable"))
      .catch(() => {
        if (!controller.signal.aborted) setState("unavailable");
      });
    return () => controller.abort();
  }, []);

  return state;
}

function ReadinessBadge({ state }: { state: Readiness }) {
  if (state === "ready")
    return (
      <Badge variant="success">
        <CheckCircle2 aria-hidden="true" className="size-3.5" /> API 已就绪
      </Badge>
    );
  if (state === "unavailable")
    return (
      <Badge
        className="border-destructive/30 bg-destructive/10 text-destructive"
        variant="outline"
      >
        API 不可用
      </Badge>
    );
  return (
    <Badge variant="secondary">
      <LoaderCircle aria-hidden="true" className="size-3.5 animate-spin" />
      检查 API
    </Badge>
  );
}

export function ProjectShell({ children }: ProjectShellProps) {
  const readiness = useReadiness();
  const location = useLocation();
  const navigate = useNavigate();
  const projectId = location.pathname.match(/^\/projects\/([^/]+)/)?.[1] ?? "";
  const hasProjectContext = Boolean(projectId);
  const links = navigationFor(projectId);
  const projectsHome = "/projects";

  return (
    <div
      className={
        hasProjectContext
          ? "min-h-screen bg-muted/40 text-foreground lg:flex lg:h-dvh lg:overflow-hidden"
          : "min-h-screen bg-muted/40 text-foreground lg:flex"
      }
      data-testid="project-shell"
    >
      <aside
        aria-label="应用菜单"
        className="hidden shrink-0 flex-col border-r border-border bg-background px-4 py-5 lg:flex lg:w-56"
      >
        <Button
          className="justify-start px-0 text-base"
          variant="ghost"
          onClick={() => navigate(projectsHome)}
        >
          <span className="grid size-10 place-items-center rounded-md bg-primary text-primary-foreground">
            <Clapperboard aria-hidden="true" className="size-5" />
          </span>
          <span>帧间制片</span>
        </Button>
        <nav aria-label="工作台导航" className="mt-7 grid content-start gap-1">
          {links.map(({ label, icon: Icon, to }) => (
            <NavLink
              className={({ isActive }) =>
                isActive
                  ? "flex items-center gap-3 rounded-md bg-secondary px-3 py-2.5 text-sm font-semibold text-secondary-foreground"
                  : "flex items-center gap-3 rounded-md px-3 py-2.5 text-sm font-medium text-muted-foreground hover:bg-accent hover:text-foreground"
              }
              key={label}
              to={to}
            >
              <Icon aria-hidden="true" className="size-4" />
              <span className="truncate">{label}</span>
            </NavLink>
          ))}
        </nav>
      </aside>

      <div
        className={
          hasProjectContext
            ? "min-w-0 lg:flex lg:min-h-0 lg:flex-1 lg:flex-col"
            : "min-w-0 lg:flex lg:flex-1 lg:flex-col"
        }
      >
        <header className="shrink-0 border-b border-border bg-background">
          <div className="flex items-center justify-between gap-4 px-4 py-4 sm:px-6 lg:px-8">
            <div className="min-w-0">
              <Link
                className="inline-flex items-center gap-1 text-sm text-muted-foreground hover:text-foreground"
                to={projectsHome}
              >
                <ArrowLeft aria-hidden="true" className="size-4" /> 项目入口
              </Link>
              <div className="mt-2 flex flex-wrap items-center gap-2">
                <h1 className="truncate text-xl font-semibold">
                  {projectId ? "项目工作台" : "项目索引"}
                </h1>
              </div>
            </div>
            <div className="flex shrink-0 items-center gap-2">
              <ReadinessBadge state={readiness} />
              {projectId && (
                <Button
                  aria-label="项目设置"
                  onClick={() => navigate(`/projects/${projectId}/settings`)}
                  size="icon"
                  title="项目设置"
                  variant="outline"
                >
                  <Settings2 aria-hidden="true" className="size-4" />
                </Button>
              )}
            </div>
          </div>
          <nav
            aria-label="移动工作台导航"
            className="flex gap-1 overflow-x-auto border-t border-border px-3 py-2 lg:hidden"
          >
            {links.map(({ label, icon: Icon, to }) => (
              <NavLink
                className={({ isActive }) =>
                  isActive
                    ? "inline-flex shrink-0 items-center gap-2 rounded-md bg-secondary px-3 py-2 text-sm font-semibold text-secondary-foreground"
                    : "inline-flex shrink-0 items-center gap-2 rounded-md px-3 py-2 text-sm font-medium text-muted-foreground hover:bg-accent hover:text-foreground"
                }
                key={label}
                to={to}
              >
                <Icon aria-hidden="true" className="size-4" />
                {label}
              </NavLink>
            ))}
          </nav>
        </header>
        {readiness === "unavailable" && (
          <div
            className="mx-4 mt-4 flex items-center gap-2 rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-sm text-destructive sm:mx-6 lg:mx-8"
            role="status"
          >
            <span>无法连接 /api/v1/health/ready</span>
            <span>；保留 owner 原始不可用状态。</span>
          </div>
        )}
        <main
          className={
            hasProjectContext ? "min-w-0 lg:min-h-0 lg:flex-1" : "min-w-0"
          }
        >
          {children}
        </main>
      </div>
    </div>
  );
}
