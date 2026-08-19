# @video-agent/contracts

阶段 0 的跨层文档契约。`schemas/` 中的九份 Draft 2020-12 JSON Schema 是唯一权威来源；`src/generated/` 和 `src/index.ts` 由它们生成，不能手动编辑。

## Schema

- `Project`、`Episode`、`Scene`、`Shot`
- `Asset`、`AssetVersion`
- `WorkflowDraft`、`WorkflowVersion`
- `TimelineDocument`

所有文档都要求稳定 UUID、`schema_version`、非负 `revision` 和受限状态。业务属性使用 camelCase；数据库 adapter 负责映射到 snake_case。

`AssetVersion` 只保存抽象 `storageObject` 元数据，不能保存媒体二进制、base64、blob 或宿主绝对路径。`WorkflowVersion` 与 `TimelineDocument` 的不可变性由持久化边界以追加版本实现，Schema 固化所需的版本引用和内容哈希字段。

## Commands

```sh
npm test
npm run generate:types
npm run check:types
npm run format:check
```
