const services = [
  { name: "API", url: "http://localhost:18180/health" },
  { name: "Worker", url: "http://localhost:18181/health" },
];

export default function Home() {
  return (
    <main className="shell">
      <section className="panel">
        <p className="eyebrow">video-agent</p>
        <h1>AI 视频生成 Agent 环境</h1>
        <p className="summary">
          当前页面用于确认 Next.js 前端容器已启动；业务功能将在后续 OpenSpec
          change 中按选题、脚本、素材、生成、发布和优化链路逐步实现。
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
