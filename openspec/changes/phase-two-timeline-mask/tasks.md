## 1. Schema and Owner

- [ ] 1.1 定义 mask、关键帧路径、羽化、反转、track matte 和 capability snapshot Schema
- [ ] 1.2 设计 Timeline owner 表、项目归属、revision、hash、track lock 和 no-GC 约束
- [ ] 1.3 建立合法/非法路径、跨项目引用、越界和容量 fixture

## 2. Commands and Rendering

- [ ] 2.1 实现 mask/track matte typed command、CAS、权限和幂等
- [ ] 2.2 实现路径插值、时间边界、点数/关键帧上限和诊断
- [ ] 2.3 扩展 canonical mask RenderPlan 和 plan hash
- [ ] 2.4 实现 PixiJS preview 与 FFmpeg filter graph 编译
- [ ] 2.5 实现缩略图/代理降级、renderer gate 和重启恢复

## 3. Integration and Verification

- [ ] 3.1 将 mask provenance 接入 TimelineVersion、ExportArtifact 和 portable manifest
- [ ] 3.2 增加 parity、权限、容量、锁、冲突、unknown 和零副作用测试
- [ ] 3.3 用真实素材 benchmark 冻结性能上限和并发
- [ ] 3.4 完成 mask 编辑到预览/导出的 Playwright 闭环
- [ ] 3.5 完成 OpenSpec strict、文档和历史 Timeline 兼容验证
