import type { ReactNode } from "react";

const navItems = [
  ["用户与权限", "/#用户与权限"],
  ["模型与路由", "/models"],
  ["工具与 MCP", "/#工具与 MCP"],
  ["任务与日志", "/#任务与日志"],
  ["成本与限额", "/#成本与限额"],
  ["环境健康", "/#环境健康"],
];

export function AdminShell({ children, active }: { children: ReactNode; active?: string }) {
  return (
    <main className="adminShell">
      <aside className="adminRail" aria-label="管理后台导航">
        <a className="brandBlock" href="/">
          <div className="brandMark">NX</div>
          <div><p>NOVEX ADMIN</p><span>控制面</span></div>
        </a>
        <nav aria-label="管理后台导航菜单" className="adminNav">
          {navItems.map(([label, href]) => (
            <a className={active === label ? "active" : ""} href={href} key={label}>{label}</a>
          ))}
        </nav>
      </aside>
      <section className="adminWorkbench">{children}</section>
    </main>
  );
}
