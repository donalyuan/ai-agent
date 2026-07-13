## 1. 建立重构基线

- [x] 1.1 在 Compose API 容器运行 `cargo test --workspace`，确认重构前测试基线并记录失败项
- [x] 1.2 汇总当前 Axum 路由、HTTP 方法和对应 handler，建立迁移核对清单
- [x] 1.3 全仓库统计旧 `agents::models`、`conversational_runtime`、`AppState` 和 `build_app_with_state` 调用方

## 2. 迁移领域模型

- [x] 2.1 创建 `backend/src/domain` 模块并迁移脚本实体、分镜和状态规则
- [x] 2.2 迁移选题、生成批次、质量评估和主题组评审领域类型
- [x] 2.3 迁移 Agent 会话、消息、run、step 及其状态类型
- [x] 2.4 更新 Repository、Agent 服务和相关测试使用新的 Domain 模块路径
- [x] 2.5 运行脚本、选题和对话 Domain/Repository 测试，确认领域迁移无行为变化

## 3. 拆分应用组装层

- [x] 3.1 创建 `backend/src/bootstrap/config.rs` 并迁移 `AppConfig` 与环境配置读取
- [x] 3.2 创建 `backend/src/bootstrap/state.rs` 并迁移 `AppState`、依赖访问和 Service 组装
- [x] 3.3 创建 `backend/src/bootstrap/runtime.rs` 并迁移 PostgreSQL、Redis 和运行时状态构建
- [x] 3.4 更新 `main.rs`、测试构造器和应用入口使用新的 bootstrap 路径
- [x] 3.5 运行健康检查、CORS、数据库连接和配置相关测试

## 4. 拆分公共 API 基础设施

- [x] 4.1 创建 `backend/src/api/error.rs`，迁移公共 JSON rejection 与通用错误响应结构
- [x] 4.2 创建 `backend/src/api/router.rs`，集中组合业务 Router、CORS 和静态素材服务
- [x] 4.3 创建 health 与 workspace API 模块，并创建对应 Application Service
- [x] 4.4 拆分 AI 模型管理 API DTO、handler、错误映射和 Application Service
- [x] 4.5 运行 health、CORS、workspace menu 和 AI model 路由测试

## 5. 按业务拆分 API 与 Application Service

- [x] 5.1 拆分 projects API DTO、handler、路由和 Application Service，保留策略草稿重试语义
- [x] 5.2 拆分 topics API DTO、handler、路由和 Application Service，保留批次、评审、质量和状态规则
- [x] 5.3 拆分 materials API DTO、handler、路由和 Application Service，保留筛选和状态语义
- [x] 5.4 拆分 scripts API DTO、handler、路由和 Application Service，保留生成、查询和状态更新语义
- [x] 5.5 拆分 asset generation API DTO、handler、路由和 Application Service，保留幂等、候选和任务确认语义
- [x] 5.6 逐模块运行 project、topic、material、script 和 asset generation 路由测试

## 6. 拆分 Agent Runtime 与对话 API

- [x] 6.1 创建 `application/agents/runtime` 统一入口、共享类型和分层错误
- [x] 6.2 迁移脚本生成、意图识别和分镜修改能力到独立 script 模块
- [x] 6.3 迁移普通选题与补充批次生成能力到独立 topic generation 模块
- [x] 6.4 迁移质量闸门、最多一次重写和质量输出解析到独立 topic quality 模块
- [x] 6.5 迁移主题组评审、评审输出解析和上下文构建到独立 topic review 模块
- [x] 6.6 拆分 Prompt 构建与共享上下文格式化，并保留模型选择、快照和同模型重试规则
- [x] 6.7 拆分 conversations API DTO、handler、路由和 Application Service，接入新的统一 Runtime
- [x] 6.8 运行脚本 Agent、选题 Agent、质量闸门、主题评审和连续对话相关测试

## 7. 清理旧实现并补充注释

- [x] 7.1 删除旧 `agents/models/request.rs`，确认全部 DTO 已归入对应 API 模块
- [x] 7.2 删除旧 `agents/conversational_runtime.rs`，确认全部能力已迁入 Runtime 子模块
- [x] 7.3 收敛 `lib.rs` 为模块声明和应用构建入口，删除业务 handler 与错误映射
- [x] 7.4 搜索并删除旧公共模块路径引用和 `pub use` 兼容导出
- [x] 7.5 为模块职责、公共 Service、Agent 分派、重试、幂等、质量闸门、主题组归一和事务顺序补充必要注释
- [x] 7.6 检查新文件职责和体量，拆除任何新产生的千行聚合文件

## 8. 全量验证与规格核对

- [x] 8.1 在 Compose API 容器运行 `cargo fmt --all -- --check`
- [x] 8.2 在 Compose API 容器运行 `cargo test --workspace`
- [x] 8.3 在 Compose API 容器运行 `cargo clippy --workspace --all-targets -- -D warnings`
- [x] 8.4 对照基线路由清单确认 URL、HTTP 方法、状态码、JSON 字段和错误协议未变化
- [x] 8.5 对照本 change 的 requirements 和 tasks 核查实现覆盖，并运行 `openspec validate refactor-rust-api-agent-runtime-layers`
- [x] 8.6 运行 `openspec instructions apply --change "refactor-rust-api-agent-runtime-layers" --json`，确认状态与实际任务进度一致
