import type { WorkspaceMenuNode } from "./api";

export type WorkspaceRouteMatch = {
  menuKey: string;
  subMenuKey: string | null;
  routePath: string;
};

export function normalizeWorkspacePath(path: string) {
  if (!path.startsWith("/")) {
    return "/";
  }
  const normalized = path.replace(/\/+$/, "");
  return normalized || "/";
}

export function findWorkspaceRoute(
  menus: WorkspaceMenuNode[],
  pathname: string,
): WorkspaceRouteMatch | null {
  const normalizedPath = normalizeWorkspacePath(pathname);
  for (const menu of menus) {
    if (!menu.is_enabled) {
      continue;
    }
    const menuPath = routePathOf(menu);
    if (menuPath === normalizedPath) {
      return { menuKey: menu.menu_key, subMenuKey: null, routePath: menuPath };
    }
    for (const child of menu.children) {
      const childPath = child.is_enabled ? routePathOf(child) : null;
      if (childPath === normalizedPath) {
        return {
          menuKey: menu.menu_key,
          subMenuKey: child.menu_key,
          routePath: childPath,
        };
      }
    }
  }
  return null;
}

export function findWorkspaceRouteByMenuKey(
  menus: WorkspaceMenuNode[],
  menuKey: string,
): WorkspaceRouteMatch | null {
  for (const menu of menus) {
    if (!menu.is_enabled) {
      continue;
    }
    if (menu.menu_key === menuKey) {
      const routePath = routePathOf(menu);
      return routePath ? { menuKey, subMenuKey: null, routePath } : null;
    }
    const child = menu.children.find((candidate) => candidate.menu_key === menuKey);
    if (child?.is_enabled) {
      const routePath = routePathOf(child);
      return routePath
        ? { menuKey: menu.menu_key, subMenuKey: child.menu_key, routePath }
        : null;
    }
  }
  return null;
}

export function defaultWorkspaceRoute(menus: WorkspaceMenuNode[]) {
  for (const menu of menus) {
    if (!menu.is_enabled) {
      continue;
    }
    const menuPath = routePathOf(menu);
    if (menuPath) {
      return { menuKey: menu.menu_key, subMenuKey: null, routePath: menuPath };
    }
    for (const child of menu.children) {
      const childPath = child.is_enabled ? routePathOf(child) : null;
      if (childPath) {
        return {
          menuKey: menu.menu_key,
          subMenuKey: child.menu_key,
          routePath: childPath,
        };
      }
    }
  }
  return null;
}

function routePathOf(menu: WorkspaceMenuNode) {
  if (!menu.route_path?.startsWith("/")) {
    return null;
  }
  return normalizeWorkspacePath(menu.route_path);
}
