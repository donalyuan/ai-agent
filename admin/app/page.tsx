const managementAreas = [
  {
    title: "用户与权限",
    description: "管理用户、角色、权限策略和审计范围。",
  },
  {
    title: "模型与路由",
    description: "配置模型供应商、模型路由、API Key 和健康状态。",
    status: "已接入",
  },
  {
    title: "工具与 MCP",
    description: "维护私有 TOS、Agent 工具、MCP 连接和执行权限。",
    status: "已接入",
  },
  {
    title: "任务与日志",
    description: "查看 Worker、任务队列、运行日志、错误和审计记录。",
  },
  {
    title: "成本与限额",
    description: "跟踪调用成本、组织限额、速率限制和用量趋势。",
  },
  {
    title: "环境健康",
    description: "检查 API、数据库、Redis、Worker 和外部依赖状态。",
  },
];

export default function Home() {
  return (
    <AdminShell>
      <div className="adminOverviewPage">
        <header className="adminTopbar">
          <div>
            <p className="sectionKicker">NOVEX ADMIN</p>
            <h1>平台管理后台</h1>
            <p>
              管理用户、权限、模型、工具、任务和运行状态。视频内容生产工作台已迁移到 apps/video-agent。
            </p>
          </div>
          <span className="boundaryBadge">管理控制面</span>
        </header>

        <section className="managementGrid" aria-label="平台管理能力">
          {managementAreas.map((area) => (
            <article className="managementCard" id={area.title} key={area.title}>
              <h2>{area.title}</h2>
              <p>{area.description}</p>
              <span>{area.status ?? "待接入"}</span>
            </article>
          ))}
        </section>
      </div>
    </AdminShell>
  );
}
import { AdminShell } from "./components/AdminShell";
