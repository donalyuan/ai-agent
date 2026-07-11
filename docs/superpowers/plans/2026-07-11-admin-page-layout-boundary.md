# Admin Page Layout Boundary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 修复 Admin 模型管理页被公共 Grid 轨道拉散的问题，同时保持首页布局稳定。

**Architecture:** `AdminShell` 仅提供侧栏与内容区边界；首页和模型管理页各自拥有内部布局容器。使用 Vitest 验证结构类名，使用 Playwright 在 `1920x948` 验证相邻区域几何关系。

**Tech Stack:** Next.js 14、React、TypeScript、CSS、Vitest、Testing Library、Playwright。

---

### Task 1: 建立页面布局边界

**Files:**
- Modify: `admin/app/page.tsx`
- Modify: `admin/app/models/ModelManagementPage.tsx`
- Modify: `admin/app/styles.css`
- Test: `admin/app/page.test.tsx`
- Test: `admin/app/models/page.test.tsx`

- [ ] **Step 1: 写失败结构测试**

断言首页内容位于 `.adminOverviewPage`，模型页 Header、Tab、筛选栏和表格共同位于 `.modelManagementLayout`。

- [ ] **Step 2: 运行结构测试并确认失败**

Run: `npm test -- --run app/page.test.tsx app/models/page.test.tsx`

Expected: FAIL，缺少页面级布局容器。

- [ ] **Step 3: 实现页面级布局容器**

`AdminShell` 内容区移除业务 Grid；首页使用两行 Grid，模型页使用纵向 Flex，并让表格区域保持紧凑且可滚动。

- [ ] **Step 4: 运行结构测试并确认通过**

Run: `npm test -- --run app/page.test.tsx app/models/page.test.tsx`

Expected: PASS。

### Task 2: 增加桌面布局回归

**Files:**
- Modify: `admin/e2e/workspace.spec.ts`

- [ ] **Step 1: 写 `1920x948` 几何断言**

读取 `.modelHeader`、`.modelTabs`、`.modelToolbar`、`.modelTableWrap` 的 `getBoundingClientRect()`，断言相邻区域垂直间距不超过 2px，模型页主内容顶部不出现额外空白。

- [ ] **Step 2: 运行 Playwright 并确认旧布局失败**

Run: `npm run test:e2e`

Expected: 旧布局的 Header 到 Tab 间距断言失败。

- [ ] **Step 3: 运行全量验证**

Run: `npm test -- --run && npm run lint && npm run build && npm run test:e2e`

Expected: 全部 PASS。

- [ ] **Step 4: 重启并验证真实容器**

重启 `ai-agent-admin`，在 `http://localhost:18182/models` 检查页面 HTTP 200、区域几何关系和菜单点击，无浏览器 `pageerror`。
