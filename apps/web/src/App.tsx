import {
  Activity,
  Boxes,
  CircleAlert,
  CircleCheck,
  Clapperboard,
  FolderKanban,
  Gauge,
  Layers3,
  ListChecks,
  Settings2,
  Workflow,
} from "lucide-react";
import { useEffect, useState } from "react";
import { BrowserRouter, NavLink } from "react-router";

type Readiness =
  | { state: "checking" }
  | { state: "ready" }
  | { state: "unavailable"; detail: string };

const readinessEndpoint = "/api/v1/health/ready";

const navigation = [
  { label: "项目", icon: FolderKanban, href: "/projects" },
  { label: "工作流", icon: Workflow, href: "/workflows" },
  { label: "故事板", icon: Layers3, href: "/storyboard" },
  { label: "资产", icon: Boxes, href: "/assets" },
  { label: "运行", icon: Activity, href: "/runs" },
];

const phases = ["工程", "契约", "接口", "运行"];

function useApiReadiness() {
  const [readiness, setReadiness] = useState<Readiness>({ state: "checking" });

  useEffect(() => {
    const controller = new AbortController();

    async function probeReadiness() {
      try {
        const response = await fetch(readinessEndpoint, {
          signal: controller.signal,
        });
        const payload: unknown = await response.json();
        const isReady =
          response.ok &&
          typeof payload === "object" &&
          payload !== null &&
          "status" in payload &&
          payload.status === "ready";

        if (!isReady) {
          setReadiness({
            state: "unavailable",
            detail: `端点返回 ${response.status}，尚未报告 ready`,
          });
          return;
        }

        setReadiness({ state: "ready" });
      } catch (error) {
        if (controller.signal.aborted) {
          return;
        }

        const detail = error instanceof Error ? error.message : "未知网络错误";
        setReadiness({ state: "unavailable", detail });
      }
    }

    void probeReadiness();
    return () => controller.abort();
  }, []);

  return readiness;
}

function ReadinessBadge({ readiness }: { readiness: Readiness }) {
  if (readiness.state === "checking") {
    return (
      <span className="status-badge status-checking" role="status">
        <span className="status-dot" />
        检查 API
      </span>
    );
  }

  if (readiness.state === "ready") {
    return (
      <span className="status-badge status-ready" role="status">
        <CircleCheck aria-hidden="true" size={15} />
        API 已就绪
      </span>
    );
  }

  return (
    <span className="status-badge status-unavailable" role="status">
      <CircleAlert aria-hidden="true" size={15} />
      API 不可用
    </span>
  );
}

function WorkspaceShell() {
  const readiness = useApiReadiness();

  return (
    <div className="workbench-shell">
      <aside className="sidebar" aria-label="主侧栏">
        <div className="brand">
          <span className="brand-mark" aria-hidden="true">
            <Clapperboard size={20} strokeWidth={1.8} />
          </span>
          <span>视频 Agent</span>
        </div>

        <nav aria-label="工作台导航" className="primary-nav">
          {navigation.map(({ label, icon: Icon, href }) => (
            <NavLink className="nav-item" key={href} to={href}>
              <Icon aria-hidden="true" size={18} strokeWidth={1.8} />
              <span>{label}</span>
            </NavLink>
          ))}
        </nav>

        <div className="sidebar-footer">
          <NavLink className="nav-item" to="/settings">
            <Settings2 aria-hidden="true" size={18} strokeWidth={1.8} />
            <span>设置</span>
          </NavLink>
          <p>本地工程</p>
        </div>
      </aside>

      <main className="workspace-main">
        <header className="topbar">
          <div>
            <p className="eyebrow">制片控制台</p>
            <h1>阶段 0 / 工程基线</h1>
          </div>
          <ReadinessBadge readiness={readiness} />
        </header>

        <section className="workspace-content" aria-label="阶段 0 状态">
          <div className="phase-rail" aria-label="阶段 0 状态轨">
            {phases.map((phase, index) => (
              <div
                className={index === 0 ? "phase current" : "phase"}
                key={phase}
              >
                <span className="phase-reel" aria-hidden="true">
                  {String(index + 1).padStart(2, "0")}
                </span>
                <span>{phase}</span>
              </div>
            ))}
          </div>

          <div className="status-grid">
            <section
              className="status-panel readiness-panel"
              aria-labelledby="readiness-title"
            >
              <div className="panel-heading">
                <div>
                  <p className="eyebrow">运行信号</p>
                  <h2 id="readiness-title">API readiness</h2>
                </div>
                <Gauge aria-hidden="true" size={22} strokeWidth={1.6} />
              </div>
              {readiness.state === "ready" ? (
                <p className="signal-copy">
                  `/api/v1/health/ready` 已报告 ready。
                </p>
              ) : readiness.state === "unavailable" ? (
                <div className="diagnostic">
                  <p>无法连接 /api/v1/health/ready</p>
                  <code>{readiness.detail}</code>
                </div>
              ) : (
                <p className="signal-copy">正在探测 `/api/v1/health/ready`。</p>
              )}
            </section>

            <section className="status-panel" aria-labelledby="scope-title">
              <div className="panel-heading">
                <div>
                  <p className="eyebrow">交付边界</p>
                  <h2 id="scope-title">工作台壳层</h2>
                </div>
                <ListChecks aria-hidden="true" size={22} strokeWidth={1.6} />
              </div>
              <dl className="scope-list">
                <div>
                  <dt>运行入口</dt>
                  <dd>React SPA</dd>
                </div>
                <div>
                  <dt>契约来源</dt>
                  <dd>共享 contracts</dd>
                </div>
                <div>
                  <dt>当前范围</dt>
                  <dd>工程状态</dd>
                </div>
              </dl>
            </section>
          </div>
        </section>
      </main>
    </div>
  );
}

export function App() {
  return (
    <BrowserRouter>
      <WorkspaceShell />
    </BrowserRouter>
  );
}
