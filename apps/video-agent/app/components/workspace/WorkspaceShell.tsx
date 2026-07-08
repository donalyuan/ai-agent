import type { ReactNode } from "react";
import type { Project, WorkspaceMenuNode } from "../../lib/api";

type WorkspaceShellProps = {
  apiAvailable: boolean | null;
  children: ReactNode;
  loadingMenus: boolean;
  menuError: string;
  overlay?: ReactNode;
  projects: Project[];
  selectedMenuKey: string;
  selectedSubMenuKey: string | null;
  selectedProjectId: string;
  workspaceMenus: WorkspaceMenuNode[];
  onSelectMenu: (menuKey: string) => void;
  onSelectSubMenu: (menuKey: string) => void;
  onSelectProject: (projectId: string) => void;
};

export function WorkspaceShell({
  apiAvailable,
  children,
  loadingMenus,
  menuError,
  overlay,
  projects,
  selectedMenuKey,
  selectedSubMenuKey,
  selectedProjectId,
  workspaceMenus,
  onSelectMenu,
  onSelectSubMenu,
  onSelectProject,
}: WorkspaceShellProps) {
  return (
    <main className="workspaceShell">
      <aside className="agentRail">
        <div className="brandBlock">
          <div className="brandMark">VD</div>
          <div>
            <p>VEDIO-AGENT</p>
            <span>视频工作台</span>
          </div>
        </div>

        <nav aria-label="视频工作台菜单" className="agentMenu">
          {loadingMenus ? <p className="railStateText">正在加载菜单</p> : null}
          {menuError ? <p className="railErrorText">{menuError}</p> : null}
          {!loadingMenus && !menuError
            ? workspaceMenus.map((menu) => (
                <MenuButton
                  key={menu.menu_id}
                  menu={menu}
                  selectedMenuKey={selectedMenuKey}
                  selectedSubMenuKey={selectedSubMenuKey}
                  onSelect={onSelectMenu}
                  onSelectSubMenu={onSelectSubMenu}
                />
              ))
            : null}
        </nav>
      </aside>

      <section className="workbench">
        <header className="topbar">
          <div>
            <p className="sectionKicker">VEDIO-AGENT</p>
            <h1>视频工作台</h1>
          </div>
          <div className="topbarActions">
            <span className={apiAvailable === false ? "healthBadge down" : "healthBadge"}>
              {apiAvailable === null ? "服务检测中" : apiAvailable ? "API 正常" : "API 不可用"}
            </span>
            <label className="projectSelectLabel">
              当前账号
              <select
                aria-label="当前账号"
                disabled={!projects.length}
                onChange={(event) => onSelectProject(event.target.value)}
                value={selectedProjectId}
              >
                {projects.length ? null : <option value="">暂无账号</option>}
                {projects.map((project) => (
                  <option key={project.project_id} value={project.project_id}>
                    {project.name}
                  </option>
                ))}
              </select>
            </label>
          </div>
        </header>

        {children}
      </section>
      {overlay}
    </main>
  );
}

function MenuButton({
  menu,
  selectedMenuKey,
  selectedSubMenuKey,
  onSelect,
  onSelectSubMenu,
}: {
  menu: WorkspaceMenuNode;
  selectedMenuKey: string;
  selectedSubMenuKey: string | null;
  onSelect: (menuKey: string) => void;
  onSelectSubMenu: (menuKey: string) => void;
}) {
  const active = menu.menu_key === selectedMenuKey;
  return (
    <div className="agentMenuGroup">
      <button
        className={active ? "agentItem active" : "agentItem"}
        disabled={!menu.is_enabled}
        onClick={() => onSelect(menu.menu_key)}
        title={menu.description}
        type="button"
      >
        <span>{menu.label}</span>
        <small>{menuStatusLabel(menu)}</small>
      </button>
      {active && menu.children.length ? (
        <div aria-label={`${menu.label}二级菜单`} className="agentSubMenu">
          {menu.children.map((child) => (
            <button
              className={child.menu_key === selectedSubMenuKey ? "agentSubItem active" : "agentSubItem"}
              disabled={!child.is_enabled}
              key={child.menu_id}
              onClick={() => onSelectSubMenu(child.menu_key)}
              title={child.description}
              type="button"
            >
              {child.label}
            </button>
          ))}
        </div>
      ) : null}
    </div>
  );
}

function menuStatusLabel(menu: WorkspaceMenuNode) {
  if (menu.status === "active") {
    return "当前";
  }
  if (menu.status === "disabled") {
    return "禁用";
  }
  const phase = typeof menu.metadata.phase === "number" ? `P${menu.metadata.phase}` : "计划";
  return phase;
}
