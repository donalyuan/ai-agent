# 设计文档

## 概述

脚本Agent是video-agent系统的第二个核心Agent，负责将选题（Topic）转化为结构化的视频脚本（Script）。

## 架构位置

### 系统分层中的位置

```
用户层
  ↓
业务层（API）
  ↓
Agent层（Script Agent）
  ↓
模型层（OpenAI/Claude）
  ↓
数据层（PostgreSQL）
```

**职责边界**：
- 输入：选题文本、风格偏好、分镜数量
- 核心处理：Prompt构造 → LLM调用 → JSON解析 → 数据验证 → 数据持久化
- 输出：结构化脚本（标题、hook、分镜列表）

**不负责**：
- 素材匹配（交给素材Agent）
- 视频生成（交给视频Agent）
- 发布分发（交给发布Agent）

---

## 核心设计

### 1. 领域模型

#### Script聚合根

```rust
pub struct Script {
    pub id: Uuid,
    pub project_id: Uuid,
    pub title: String,
    pub hook: String,
    pub content: serde_json::Value,
    pub status: ScriptStatus,
    pub parent_id: Option<Uuid>,
    pub scenes: Vec<Scene>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

**设计理由**：
- `Script`作为聚合根，统一管理其下所有`Scene`
- `content`使用JSONB保存元数据，便于未来扩展
- `scenes`内嵌在领域模型中，符合业务聚合逻辑

#### Scene值对象

```rust
pub struct Scene {
    pub id: Uuid,
    pub sequence: i32,
    pub narration: String,
    pub visual_description: String,
    pub emotion: String,
    pub duration_sec: i32,
}
```

**设计理由**：
- `Scene`只在`Script`上下文中存在，没有独立生命周期
- sequence作为业务关键字段，必须在内存中保证有序

#### ScriptStatus枚举

```rust
pub enum ScriptStatus {
    Draft,
    Approved,
    Archived,
}
```

**设计理由**：
- 使用强类型枚举，避免魔法字符串
- 状态流转明确：draft → approved → archived

---

### 2. 应用服务层

#### ScriptAgentService

```rust
pub struct ScriptAgentService {
    llm_client: Arc<dyn LLMClient>,
    script_repository: Arc<dyn ScriptRepository>,
    project_repository: Arc<dyn ProjectRepository>,
}

impl ScriptAgentService {
    pub async fn generate_script(
        &self,
        request: GenerateScriptRequest,
    ) -> Result<Script, ScriptError>;
    
    pub async fn get_script(&self, script_id: Uuid) -> Result<Script, ScriptError>;
    
    pub async fn list_scripts(
        &self,
        project_id: Uuid,
        filter: ScriptListFilter,
    ) -> Result<Vec<Script>, ScriptError>;
    
    pub async fn update_status(
        &self,
        script_id: Uuid,
        status: ScriptStatus,
    ) -> Result<Script, ScriptError>;
}
```

**设计理由**：
- Service层负责协调LLM调用、数据校验和持久化
- 使用依赖注入，便于测试和未来替换实现
- 分离业务逻辑（Service）和数据访问（Repository）

---

### 3. LLM抽象层

#### LLMClient trait

```rust
#[async_trait]
pub trait LLMClient: Send + Sync {
    async fn generate_script(
        &self,
        prompt: ScriptPrompt,
    ) -> Result<String, LLMError>;
}
```

**实现策略**：
- 定义统一trait，屏蔽OpenAI/Claude差异
- MVP先实现一个provider（推荐OpenAI claude-4-turbo）
- 后续可无缝切换或支持多provider

#### Prompt Builder

```rust
pub struct ScriptPromptBuilder;

impl ScriptPromptBuilder {
    pub fn build(request: &GenerateScriptRequest) -> ScriptPrompt {
        // 构造system prompt + user prompt
    }
}
```

**设计理由**：
- Prompt构造逻辑独立封装，便于测试和优化
- 后续可根据平台/风格切换不同prompt模板

---

## 数据流设计

### 生成脚本主流程

```mermaid
sequenceDiagram
    participant U as User/API
    participant S as ScriptAgentService
    participant P as ProjectRepo
    participant L as LLMClient
    participant R as ScriptRepo
    participant D as PostgreSQL

    U->>S: generate_script(request)
    S->>P: check_project_exists(project_id)
    P->>D: SELECT projects WHERE id = ?
    D-->>P: project exists
    P-->>S: OK
    
    S->>S: validate_request()
    S->>S: build_prompt()
    
    loop max 3 retries
        S->>L: generate_script(prompt)
        L-->>S: JSON response
        S->>S: parse_json()
        alt parse success
            break
        else parse failed
            S->>S: log warning
        end
    end
    
    S->>S: validate_script_structure()
    
    S->>R: save_script(script)
    R->>D: BEGIN TRANSACTION
    R->>D: INSERT INTO scripts
    R->>D: INSERT INTO scenes (batch)
    R->>D: COMMIT
    D-->>R: saved
    R-->>S: script with ids
    
    S-->>U: ScriptResponse
```

---

### 关键节点说明

#### 节点1：项目存在性校验
**为什么**：
- 避免生成孤立脚本，保证数据一致性
- project_id是外键，提前校验可返回更友好的404错误

#### 节点2：Prompt构造
**为什么**：
- Prompt质量直接决定脚本质量
- 需要根据style、scene_count动态调整内容

#### 节点3：JSON解析与重试
**为什么**：
- LLM输出不稳定，可能返回非标准JSON
- 3次重试是成本和成功率的平衡点

#### 节点4：事务保存
**为什么**：
- script和scenes必须原子性保存
- 避免只有主表无分镜的脏数据

---

## 模块划分

### backend/src/ 目录结构建议

```
backend/src/
├── agents/
│   ├── mod.rs
│   ├── script_agent.rs          # 核心业务服务
│   ├── llm/
│   │   ├── mod.rs
│   │   ├── client.rs            # LLMClient trait
│   │   ├── openai.rs            # OpenAI实现
│   │   └── prompts/
│   │       ├── mod.rs
│   │       └── script_prompt.rs # Prompt模板
│   └── models/
│       ├── mod.rs
│       ├── script.rs            # Script/Scene领域模型
│       └── request.rs           # GenerateScriptRequest等
│
├── repositories/
│   ├── mod.rs
│   ├── script_repository.rs     # 数据访问层
│   └── project_repository.rs    # 项目存在性检查
│
├── routes/
│   ├── mod.rs
│   └── scripts.rs               # HTTP处理器
│
├── errors/
│   ├── mod.rs
│   └── script_error.rs          # 业务错误定义
│
└── main.rs
```

**设计理由**：
- `agents/`：业务编排逻辑
- `repositories/`：数据访问隔离
- `routes/`：HTTP协议层
- `errors/`：统一错误处理

---

## 错误处理设计

### ScriptError枚举

```rust
pub enum ScriptError {
    ValidationError(String),
    ProjectNotFound(Uuid),
    ScriptNotFound(Uuid),
    LLMError(String),
    ParseError(String),
    DatabaseError(sqlx::Error),
    Timeout,
}
```

**错误映射**：

| ScriptError | HTTP状态码 | 说明 |
|------------|-----------|------|
| ValidationError | 400 | 请求参数错误 |
| ProjectNotFound | 404 | 项目不存在 |
| ScriptNotFound | 404 | 脚本不存在 |
| Timeout | 503 | LLM超时 |
| ParseError | 500 | LLM返回格式异常 |
| DatabaseError | 500 | 持久化失败 |
| LLMError | 500 | LLM服务异常 |

**设计理由**：
- 明确区分业务错误和系统错误
- 对外返回清晰的HTTP语义
- 内部保留详细错误上下文供日志使用

---

## 并发与性能

### MVP并发策略

**约束**：
- 目标：支持20个并发生成请求
- 单次LLM调用超时：30秒
- 重试最多3次

**实现建议**：
- HTTP层使用Axum async handler
- LLM调用使用`tokio::time::timeout`
- 数据库连接池使用SQLx Pool，大小建议10-20

**为什么这样设计**：
- LLM调用是IO密集型，async足够支撑MVP规模
- 20并发下，不需要额外队列或后台任务系统
- 如果后续LLM延迟过高，再改为异步任务表模式

### 性能热点

#### 热点1：LLM调用延迟
**风险**：30秒响应对用户体验较差
**MVP处理**：
- 同步返回结果，简单直接
- 后续可改造为异步任务模式（generation_tasks）

#### 热点2：批量插入分镜
**风险**：逐条插入性能差
**处理**：
- 使用单条SQL批量插入6-8个scene
- 在事务中一次提交

---

## 测试策略

### 1. 单元测试

**测试目标**：
- Prompt Builder输出正确
- JSON解析函数健壮
- 请求校验逻辑正确
- 领域模型状态流转正确

**Mock策略**：
- Mock `LLMClient`：返回固定JSON
- Mock `ScriptRepository`：内存实现

### 2. 集成测试

**测试目标**：
- API路由工作正常
- 数据库读写正确
- 事务完整性

**环境**：
- 在Docker容器内运行cargo test
- 使用测试数据库或事务回滚

### 3. 端到端测试

**测试目标**：
- 从HTTP请求到数据库持久化的完整链路
- 错误场景（无效project_id、空topic、LLM超时）

---

## 可观测性

### 日志

**关键日志点**：
1. 开始生成脚本（包含project_id、topic摘要）
2. LLM调用开始/结束（包含耗时）
3. 重试发生（包含第几次）
4. 数据库保存成功（包含script_id）
5. 错误发生（包含错误类型和上下文）

**日志示例**：
```rust
info!("script generation started: project_id={}, style={}, scene_count={}", ...);
warn!("llm parse failed on attempt {}: {}", attempt, err);
info!("script saved successfully: script_id={}, duration_ms={}", script_id, elapsed);
```

### 指标（MVP可选）
- script_generation_total
- script_generation_success_total
- script_generation_failure_total
- script_generation_duration_ms
- llm_retry_count

---

## 安全与成本控制

### 输入限制
- `topic`长度限制10-200字符
- `scene_count`限制5-8
- `style`必须在枚举中

### 成本控制
**为什么需要**：CLAUDE.md要求涉及外部费用的操作必须有成本控制。

**实现建议**：
- 限制单次生成的max_tokens
- 记录每次调用的model和耗时到`content.metadata`
- MVP阶段先不做精确token计费，但保留扩展字段

### 重试控制
- 只对可恢复错误重试（超时、解析失败）
- 不对参数错误、项目不存在等业务错误重试
- 最多3次，避免成本失控

---

## 扩展演进路线

### Phase 1（当前MVP）
- 同步生成脚本
- 单一LLM provider
- 结构化JSON输出
- PostgreSQL持久化

### Phase 2
- 支持多个LLM provider切换
- Prompt版本管理
- A/B测试结果统计
- 脚本优化接口

### Phase 3
- 异步任务队列化
- 批量生成多版本
- 基于历史爆款数据优化prompt
- 脚本质量评分

---

## 设计决策总结

### 决策1：同步API而非异步任务
**结论**：MVP先做同步API
**理由**：
- 逻辑简单，联调成本低
- 当前仓库仍是骨架阶段，先打通最短闭环
- 30秒以内可接受，后续再演进

### 决策2：Scene单独成表而不是只放JSONB
**结论**：使用`scripts` + `scenes`双表
**理由**：
- 分镜是后续素材匹配、视频生成的核心输入
- 单独成表便于按顺序查询和关联其他实体
- 与已有 [`video-agent-database-schema`](../../../docs/requirements/video-agent-database-schema.md) 设计一致

### 决策3：LLM输出JSON而非Markdown
**结论**：强制JSON输出
**理由**：
- 下游系统需要结构化数据
- 便于自动解析和验证
- 降低人工干预成本

### 决策4：重试3次
**结论**：固定3次重试
**理由**：
- 1次太少，格式错误概率高
- 5次以上成本过高
- 3次是成功率和成本的折中
