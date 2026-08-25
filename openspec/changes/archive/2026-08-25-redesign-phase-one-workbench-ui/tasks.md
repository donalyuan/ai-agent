## 1. 原型契约

- [x] 1.1 记录共享组件库、静态数据、零副作用和正式迁移确认门。
- [x] 1.2 为 `/prototype` 写入失败测试：生产状态可见、静态交互可用、`fetch` 零调用。

## 2. 静态原型

- [x] 2.1 使用 `shared/ui`、Lucide、既有 token 和固定演示数据实现可运行的阶段一工作台原型。
- [x] 2.2 添加独立 `/prototype` route，不改变任何正式 owner API route 或默认业务导航。
- [x] 2.3 验证桌面与移动布局、键盘焦点、无横向溢出和业务副作用为零。

## 3. 用户确认门

- [x] 3.1 启动原型并向用户提供本地 URL，等待明确确认视觉方向。
- [x] 3.2 收到确认后，按 Workbench、Review、Assets、Timeline、Exports、Settings 的顺序为正式迁移补充页面级任务与测试；确认前不得执行。
- [x] 3.3 拆分共享工作台壳层、应用路由和页面级通用展示组件；左栏只保留产品标识与导航，项目名和集上下文位于右侧工作台顶部。
- [x] 3.4 迁移 Workbench：以共享组件替换历史页面 class，并保留 CreativeBrief、Skill route、Run、Storyboard、AssetBible 的 scope/CAS/显式 mutation 语义与测试。
- [x] 3.5 迁移 Review：以共享组件替换历史页面 class，并保留 TextReview 与 AssetEditReview 的 owner session、显式确认和 revision 语义与测试。
- [x] 3.6 迁移 Assets：以共享组件替换资产中心的页面结构，并保留上传 reservation、媒体状态、usage 和 Timeline handoff 语义与测试。
- [x] 3.7 迁移 Timeline：以共享组件替换 Episode selector 和编辑器页面结构，并保留 current Cut、revision command、精确 AssetVersion handoff 语义与测试。
- [x] 3.8 迁移 Exports：以共享组件替换导出页面结构，并保留显式成员选择、失败成员 retry 和 opaque download grant 语义与测试。
- [x] 3.9 迁移 Settings：以共享组件替换 Provider/Model/Skill 和 StorageProfile 页面结构，并保留 credential masking、普通字段保留 binding 与显式 probe 语义与测试。
- [x] 3.10 清理已迁移页面的历史样式 class，复核 `App.tsx` 只保留 Provider 与路由组合。

## 4. 验证

- [x] 4.1 运行原型相关单元测试、Web typecheck、lint 和 format check。
- [x] 4.2 使用浏览器验证 `/prototype` 的桌面/移动路由、可访问控件和零网络请求。

## 5. 项目索引收口

- [x] 5.1 将 `/projects` 与项目工作台深链的壳层布局分离：索引页保留全局导航、按内容高度结束，不保留无意义的卡片或视口空白；补充回归测试并验证桌面页面。
- [x] 5.2 移除工作台滚动画布的通用阅读宽度上限，使宽屏内容使用完整项目工作区；补充回归测试并验证桌面页面。
- [x] 5.3 将当前剧集选择合并至工作台标题上下文区，移除单独状态栏并保留现有选择语义；补充回归测试并验证桌面页面。
