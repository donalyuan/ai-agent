# 脚本Agent MVP提案

## 背景

video-agent项目需要实现六大Agent中的第二个核心Agent：**脚本Agent**。该Agent负责将选题转换为结构化的视频脚本，包含分镜、旁白、视觉描述等要素。

根据 [`video-agent-mvp`](../../../docs/requirements/video-agent-mvp.md)，脚本Agent是P0优先级（Month 1-2），必须支持：
- 输入选题 → 输出结构化脚本
- 支持A/B版本生成（同一选题生成多版本）
- 生成5-8个分镜的短视频脚本

## 目标

### 功能目标
1. 提供API接口：接收选题文本，返回结构化脚本JSON
2. LLM驱动生成：使用GPT-4/Claude生成创意脚本内容
3. 数据持久化：将脚本和分镜存储到PostgreSQL
4. 支持多版本：同一选题可生成多个版本进行A/B测试

### 技术目标
1. 响应时间 < 30秒（LLM调用）
2. 生成成功率 > 95%（包含重试机制）
3. 生成的脚本符合结构化规范（可直接用于后续视频生成）
4. 核心逻辑有单元测试覆盖

## 非目标

- ❌ 不做可视化脚本编辑器（MVP只提供JSON）
- ❌ 不做实时协作编辑
- ❌ 不做版本Diff对比界面
- ❌ 不做人工审批流程
- ❌ 不做多语言支持（只支持中文）

## 成功指标

### 功能验收
- [ ] 输入"ChatGPT如何改变程序员工作流" → 生成包含5-8个分镜的完整脚本
- [ ] 每个分镜包含：旁白、视觉描述、情绪标签、时长
- [ ] 脚本包含：标题、hook（前3秒吸引点）
- [ ] 同一选题可生成3个不同版本

### 技术验收
- [ ] API响应时间 < 30秒
- [ ] LLM返回格式错误时自动重试（最多3次）
- [ ] 数据库migration可重放
- [ ] 核心函数有单元测试

## DDD分析

**核心实体**：
- `Script`（脚本）- 聚合根
  - `id`: UUID
  - `project_id`: 所属项目
  - `title`: 标题
  - `hook`: 前3秒吸引点
  - `content`: 完整内容（JSONB）
  - `status`: draft | approved | archived
  - `parent_id`: 用于A/B测试版本关联

- `Scene`（分镜）- 值对象
  - `id`: UUID
  - `script_id`: 所属脚本
  - `sequence`: 顺序（1-8）
  - `narration`: 旁白文本
  - `visual_description`: 视觉描述
  - `emotion`: 情绪标签
  - `duration_sec`: 时长（秒）

**边界**：
- 脚本Agent只负责生成结构化脚本，不涉及视频生成
- 输入来自选题Agent或用户手动输入
- 输出提供给素材Agent和视频Agent使用

**规则**：
- 一个脚本必须属于一个项目
- 一个脚本包含5-8个分镜
- 分镜sequence必须连续且唯一
- 旁白字数建议50-150字/镜

## BDD场景

```gherkin
Feature: 生成结构化视频脚本

  Scenario: 从选题生成标准脚本
    Given 用户已创建项目"科技博主"
    And 项目ID为"550e8400-e29b-41d4-a716-446655440000"
    When 用户提交选题"ChatGPT如何改变程序员工作流"
    And 选择风格"knowledge"（知识类）
    Then 系统调用LLM生成脚本
    And 返回脚本包含标题"程序员必看：ChatGPT的5大颠覆性用法"
    And 返回hook"还在手写重复代码？ChatGPT帮你3秒搞定"
    And 返回6个分镜
    And 第1个分镜旁白为"传统程序员每天要写大量重复代码..."
    And 第1个分镜视觉描述为"程序员盯着屏幕，表情疲惫"
    And 第1个分镜情绪标签为"焦虑"
    And 第1个分镜时长为8秒
    And 脚本存储到数据库
    And 返回状态码200

  Scenario: 生成A/B测试版本
    Given 已存在脚本ID为"660e8400-e29b-41d4-a716-446655440001"
    When 用户请求生成该脚本的变体版本
    Then 系统使用相同选题但不同prompt生成新脚本
    And 新脚本的parent_id指向原脚本ID
    And 返回新脚本ID

  Scenario: LLM返回格式错误
    Given 用户提交选题"AI视频生成技术"
    When LLM第1次返回格式错误的JSON
    Then 系统自动重试
    When LLM第2次返回正确格式
    Then 系统解析成功并存储

  Scenario: LLM超时降级
    Given 用户提交选题"深度学习入门"
    When LLM调用超时（>30秒）
    Then 系统返回503错误
    And 错误信息为"脚本生成超时，请稍后重试"
```

## SDD系统规格

### API接口

#### 生成脚本
```http
POST /api/scripts/generate
Content-Type: application/json

{
  "project_id": "uuid",
  "topic": "ChatGPT如何改变程序员工作流",
  "style": "knowledge",  // knowledge | story | tutorial
  "scene_count": 6,      // 5-8
  "parent_id": null      // 可选，用于A/B测试
}

Response 200:
{
  "script_id": "uuid",
  "title": "程序员必看：ChatGPT的5大颠覆性用法",
  "hook": "还在手写重复代码？ChatGPT帮你3秒搞定",
  "scenes": [
    {
      "sequence": 1,
      "narration": "传统程序员每天要写大量重复代码...",
      "visual_description": "程序员盯着屏幕，表情疲惫",
      "emotion": "焦虑",
      "duration_sec": 8
    }
  ],
  "status": "draft",
  "created_at": "2026-07-01T10:00:00Z"
}

Response 400: {"error": "project_id不能为空"}
Response 404: {"error": "项目不存在"}
Response 503: {"error": "脚本生成超时"}
```

#### 查询脚本
```http
GET /api/scripts/:script_id

Response 200:
{
  "script_id": "uuid",
  "project_id": "uuid",
  "title": "...",
  "hook": "...",
  "scenes": [...],
  "status": "draft",
  "parent_id": null,
  "created_at": "2026-07-01T10:00:00Z"
}
```

#### 更新脚本状态
```http
PUT /api/scripts/:script_id/status
Content-Type: application/json

{
  "status": "approved"  // draft | approved | archived
}

Response 200: {"message": "状态已更新"}
```

### 数据库Schema

```sql
-- scripts表
CREATE TABLE scripts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    title VARCHAR(200) NOT NULL,
    hook TEXT NOT NULL,
    content JSONB NOT NULL,  -- 完整脚本内容
    status VARCHAR(20) NOT NULL DEFAULT 'draft',  -- draft | approved | archived
    parent_id UUID REFERENCES scripts(id),  -- A/B测试版本关联
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_scripts_project ON scripts(project_id);
CREATE INDEX idx_scripts_status ON scripts(status);
CREATE INDEX idx_scripts_parent ON scripts(parent_id);

-- scenes表
CREATE TABLE scenes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    script_id UUID NOT NULL REFERENCES scripts(id) ON DELETE CASCADE,
    sequence INT NOT NULL,
    narration TEXT NOT NULL,
    visual_description TEXT NOT NULL,
    emotion VARCHAR(50) NOT NULL,
    duration_sec INT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(script_id, sequence)
);

CREATE INDEX idx_scenes_script ON scenes(script_id, sequence);
```

### LLM Prompt设计

#### System Prompt
```
你是专业的短视频脚本创作者，擅长创作15-60秒的抖音/小红书短视频脚本。

任务：
1. 根据用户提供的选题，生成结构化视频脚本
2. 脚本必须包含：标题、hook（前3秒吸引点）、5-8个分镜
3. 每个分镜包含：旁白（50-150字）、视觉描述、情绪标签、时长（秒）

要求：
- 标题吸睛，不超过30字
- hook必须在3秒内抓住观众注意力
- 旁白口语化，适合配音
- 视觉描述具体，方便后续生成视频
- 情绪标签：兴奋、焦虑、好奇、惊讶、平静、紧张等
- 总时长控制在45-60秒

输出JSON格式：
{
  "title": "标题",
  "hook": "前3秒吸引点",
  "scenes": [
    {
      "sequence": 1,
      "narration": "旁白文本",
      "visual_description": "视觉描述",
      "emotion": "情绪标签",
      "duration_sec": 8
    }
  ]
}
```

#### User Prompt模板
```
选题：{{topic}}
风格：{{style}}  // knowledge: 知识科普, story: 故事叙述, tutorial: 教程讲解
分镜数量：{{scene_count}}

请生成符合上述要求的视频脚本。
```

### 重试机制

```rust
async fn generate_script_with_retry(
    topic: &str,
    style: &str,
    max_retries: u32,
) -> Result<Script, ScriptError> {
    for attempt in 1..=max_retries {
        match call_llm(topic, style).await {
            Ok(response) => {
                match parse_script_json(&response) {
                    Ok(script) => return Ok(script),
                    Err(e) => {
                        if attempt == max_retries {
                            return Err(ScriptError::ParseError(e));
                        }
                        // 记录日志并重试
                        log::warn!("Parse error on attempt {}: {}", attempt, e);
                    }
                }
            }
            Err(e) => {
                if attempt == max_retries {
                    return Err(ScriptError::LLMError(e));
                }
            }
        }
    }
    unreachable!()
}
```

## TDD测试规格

### 单元测试

```rust
// backend/tests/script_agent.rs

#[tokio::test]
async fn test_generate_script_success() {
    // Given: 有效的project_id和选题
    let project_id = Uuid::new_v4();
    let topic = "ChatGPT如何改变程序员工作流";
    
    // When: 调用生成脚本
    let result = generate_script(project_id, topic, "knowledge", 6).await;
    
    // Then: 返回成功
    assert!(result.is_ok());
    let script = result.unwrap();
    assert!(!script.title.is_empty());
    assert!(!script.hook.is_empty());
    assert_eq!(script.scenes.len(), 6);
    assert_eq!(script.scenes[0].sequence, 1);
}

#[tokio::test]
async fn test_parse_script_json_valid() {
    // Given: 有效的LLM JSON响应
    let json = r#"{
        "title": "测试标题",
        "hook": "测试hook",
        "scenes": [
            {
                "sequence": 1,
                "narration": "测试旁白",
                "visual_description": "测试视觉",
                "emotion": "好奇",
                "duration_sec": 8
            }
        ]
    }"#;
    
    // When: 解析JSON
    let result = parse_script_json(json);
    
    // Then: 解析成功
    assert!(result.is_ok());
    let script = result.unwrap();
    assert_eq!(script.title, "测试标题");
}

#[tokio::test]
async fn test_parse_script_json_invalid() {
    // Given: 无效的JSON
    let json = r#"{"invalid": true}"#;
    
    // When: 解析JSON
    let result = parse_script_json(json);
    
    // Then: 返回错误
    assert!(result.is_err());
}

#[tokio::test]
async fn test_generate_ab_version() {
    // Given: 已存在的脚本
    let parent_script_id = Uuid::new_v4();
    
    // When: 生成A/B版本
    let result = generate_script_ab_version(parent_script_id).await;
    
    // Then: 新脚本的parent_id正确
    assert!(result.is_ok());
    let script = result.unwrap();
    assert_eq!(script.parent_id, Some(parent_script_id));
}
```

### 集成测试

```rust
#[tokio::test]
async fn test_script_api_e2e() {
    // Setup: 启动测试服务器和数据库
    let app = create_test_app().await;
    let db = setup_test_db().await;
    
    // Given: 创建测试项目
    let project_id = create_test_project(&db).await;
    
    // When: 调用生成脚本API
    let response = app
        .post("/api/scripts/generate")
        .json(&json!({
            "project_id": project_id,
            "topic": "AI视频生成技术",
            "style": "knowledge",
            "scene_count": 6
        }))
        .send()
        .await;
    
    // Then: 返回成功
    assert_eq!(response.status(), 200);
    let body: ScriptResponse = response.json().await;
    assert!(!body.script_id.is_nil());
    
    // And: 数据库中有记录
    let script = db.get_script(body.script_id).await.unwrap();
    assert_eq!(script.title, body.title);
    
    // And: scenes表中有6条记录
    let scenes = db.get_scenes(body.script_id).await.unwrap();
    assert_eq!(scenes.len(), 6);
}
```

### 失败场景测试

```rust
#[tokio::test]
async fn test_invalid_project_id() {
    let app = create_test_app().await;
    let response = app
        .post("/api/scripts/generate")
        .json(&json!({
            "project_id": Uuid::nil(),
            "topic": "测试"
        }))
        .send()
        .await;
    
    assert_eq!(response.status(), 404);
}

#[tokio::test]
async fn test_empty_topic() {
    let app = create_test_app().await;
    let response = app
        .post("/api/scripts/generate")
        .json(&json!({
            "project_id": Uuid::new_v4(),
            "topic": ""
        }))
        .send()
        .await;
    
    assert_eq!(response.status(), 400);
}
```

## 依赖

### 外部依赖
- LLM API：OpenAI GPT-4 或 Anthropic Claude
- PostgreSQL：存储脚本和分镜数据

### 内部依赖
- `projects`表：必须先存在project记录
- 后续被依赖：素材Agent、视频Agent会读取scripts和scenes数据

## 风险与缓解

### 风险1：LLM返回格式不稳定
- **缓解**：实现重试机制（最多3次），使用严格的JSON schema验证

### 风险2：LLM成本过高
- **缓解**：MVP阶段使用GPT-4-turbo或Claude-3-haiku，评估成本后再优化

### 风险3：生成内容质量不可控
- **缓解**：使用Few-shot examples，收集用户反馈后迭代prompt

### 风险4：并发场景下数据库写入冲突
- **缓解**：使用事务，scenes表有唯一索引约束

## 时间估算

- 数据库migration：0.5天
- LLM调用层：1天
- JSON解析和验证：0.5天
- API路由：0.5天
- 单元测试：1天
- 集成测试：0.5天
- 文档和联调：0.5天

**总计：4.5天**
