const services = [
  { name: "API", url: "http://localhost:18180/health" },
  { name: "Worker", url: "http://localhost:18181/health" },
];

export default function Home() {
  return (
    <main className="shell">
      <section className="panel">
        <p className="eyebrow">Novex</p>
        <h1>AI Agent 基座环境</h1>
        <p className="summary">
          当前页面用于确认 Novex 管理后台容器已启动；video-agent
          作为首个业务应用迁入 apps/video-agent 后继续迭代。
        </p>
        <div className="serviceList" aria-label="基础服务端点">
          {services.map((service) => (
            <a key={service.name} href={service.url}>
              <span>{service.name}</span>
              <code>{service.url}</code>
            </a>
          ))}
        </div>
      </section>
    </main>
  );
}
