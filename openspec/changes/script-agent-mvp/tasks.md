# 任务清单

## Phase 1: 数据库与基础设施

- [x] **T1.1 创建数据库migration**
  - [x] 编写`migrations/20260701000000_initial_schema.sql`
  - [x] 定义MVP基础业务表与`scripts`表结构
  - [x] 定义`scenes`表结构
  - [x] 创建索引和约束
  - [x] 创建`updated_at`触发器
  - [x] 新增迁移验证测试并在容器内通过
  - [x] 在运行中的`video_agent`数据库执行SQLx migration并验证成功

- [x] **T1.2 创建领域模型**
  - [x] 定义`Script`结构体（`backend/src/agents/models/script.rs`）
  - [x] 定义`Scene`结构体
  - [x] 定义`ScriptStatus`枚举
  - [x] 实现From/Into traits用于数据库映射
  - [x] 验证：编译通过

- [x] **T1.3 创建请求/响应模型**
  - [x] 定义`GenerateScriptRequest`（`backend/src/agents/models/request.rs`）
  - [x] 定义`ScriptResponse`
  - [x] 定义`ScriptListFilter`
  - [x] 添加serde序列化/反序列化
  - [x] 添加validation（validator crate）
  - [x] 验证：单元测试通过

---

## Phase 2: 数据访问层

- [x] **T2.1 实现ScriptRepository trait**
- [x] **T2.2 实现PostgreSQL Repository**
- [ ] **T2.3 ProjectRepository存在性检查**

---

## Phase 3: LLM抽象层

- [ ] **T3.1 定义LLMClient trait**
- [ ] **T3.2 实现Prompt Builder**
- [ ] **T3.3 实现OpenAI Client**
- [ ] **T3.4 实现JSON解析与验证**

---

## Phase 4: 业务服务层

- [ ] **T4.1 创建ScriptAgentService**
- [ ] **T4.2 实现generate_script方法**
- [ ] **T4.3 实现重试机制**
- [ ] **T4.4 实现其他方法**

---

## Phase 5: HTTP路由层

- [ ] **T5.1 创建scripts路由**
- [ ] **T5.2 实现POST /api/scripts/generate**
- [ ] **T5.3 实现GET /api/scripts/:script_id**
- [ ] **T5.4 实现GET /api/projects/:project_id/scripts**
- [ ] **T5.5 实现PUT /api/scripts/:script_id/status**
- [ ] **T5.6 注册路由到main.rs**

---

## 里程碑

### M1: 数据层就绪（T1-T2）
**目标**：数据库表创建成功，Repository可用

### M2: LLM层就绪（T3）
**目标**：可调用OpenAI生成结构化JSON

### M3: 业务层就绪（T4）
**目标**：Service层完整实现

### M4: API就绪（T5）
**目标**：HTTP接口可调用

---

## 下一步行动

1. **T1.1**: 创建数据库migration（最基础）
2. **T1.2**: 创建领域模型（并行可做）
3. **T3.2**: 实现Prompt Builder（可先验证prompt效果）
