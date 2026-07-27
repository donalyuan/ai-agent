# 角色定义规格

## 角色清单

### 1. Producer（制片人）

**职责**：
- 评估项目可行性
- 确定目标受众和调性
- 制定创意方向
- 设定预算和时间约束

**输入**：
- 用户原始需求
- 项目类型（品牌宣传、故事短片、教育内容等）
- 可选的参考作品

**输出**：
- `CreativeBrief`（创意简报）

**权限**：
- 读取用户需求
- 写入 CreativeBrief
- 不能直接生成视频

**PromptDefinition**：`producer.general@1`

---

### 2. Screenwriter（编剧）

**职责**：
- 构建故事世界观和叙事弧
- 创建角色设定
- 编写剧本对话和动作

**输入**：
- CreativeBrief
- 用户补充需求

**输出**：
- `StoryBible`（故事圣经）
- `CharacterBible`（角色圣经，可包含多个角色）
- `ScriptDraft`（剧本草稿）

**权限**：
- 读取 CreativeBrief
- 写入 StoryBible、CharacterBible、ScriptDraft
- 接受导演或表演指导的修改建议
- 不能直接生成视频

**PromptDefinition**：`screenwriter.general@1`

---

### 3. Director（导演）

**职责**：
- 制定视觉风格和叙事节奏
- 分解剧本为镜头序列
- 定义每个镜头的构图、运动和时长

**输入**：
- StoryBible
- ScriptDraft
- CharacterBible

**输出**：
- `DirectorialTreatment`（导演阐述）
- `ShotContract`（镜头合约，包含多个 shot）

**权限**：
- 读取编剧产物
- 写入 DirectorialTreatment、ShotContract
- 向编剧提出修改建议
- 不能直接修改剧本
- 不能直接生成视频

**PromptDefinition**：`director.general@1`

---

### 4. Cinematographer（摄影指导）

**职责**：
- 审核镜头技术可行性
- 评估灯光、构图、运动方案
- 提出技术优化建议

**输入**：
- ShotContract

**输出**：
- `TechnicalReview`（技术评审，以协作建议形式存储）

**权限**：
- 读取 ShotContract
- 提出修改建议到 collaboration_suggestions
- 不能直接修改 ShotContract
- 不能直接生成视频

**PromptDefinition**：`cinematographer.general@1`

---

### 5. PerformanceDirector（表演指导）

**职责**：
- 基于 CharacterBible 制定角色表演策略
- 定义角色情绪弧线
- 约束角色表现的一致性

**输入**：
- CharacterBible
- ScriptDraft

**输出**：
- `PerformanceBrief`（表演简报，按角色分）

**权限**：
- 读取 CharacterBible、ScriptDraft
- 写入 PerformanceBrief
- 向编剧提出角色修改建议
- 不能直接生成视频

**PromptDefinition**：`performance_director.general@1`

---

### 6. SoundDirector（声音指导）

**职责**：
- 制定音乐风格和配乐策略
- 定义音效需求
- 规划对话录音方案

**输入**：
- DirectorialTreatment
- ShotContract

**输出**：
- `SoundPlan`（声音计划）

**权限**：
- 读取导演产物
- 写入 SoundPlan
- 不能直接生成视频

**PromptDefinition**：`sound_director.general@1`

---

### 7. Editor（剪辑师）

**职责**：
- 维护连续性台账
- 记录每个镜头的视觉事实（服装、道具、场景细节）
- 确保后续镜头遵守连续性约束

**输入**：
- 已生成的镜头结果
- ShotContract

**输出**：
- `ContinuityLedger`（连续性台账）

**权限**：
- 读取所有生成结果
- 写入 ContinuityLedger
- 在后续生成时提供连续性约束
- 不能直接生成视频

**PromptDefinition**：`editor.general@1`

---

### 8. QC（质量控制）

**职责**：
- 评审每个生成的镜头
- 检查是否符合 ShotContract
- 检查是否违反连续性
- 提出重新生成建议

**输入**：
- 生成的镜头
- ShotContract
- ContinuityLedger

**输出**：
- `TakeReview`（镜头评审）

**权限**：
- 读取所有产物和生成结果
- 写入 TakeReview
- 触发 QualityGate（通过/不通过）
- 不能直接修改任何产物
- 不能直接生成视频

**PromptDefinition**：`qc.general@1`

---

### 9. CharacterCritic（角色校验器，可选）

**职责**：
- 以特定角色视角审核台词和行为
- 检查是否符合 CharacterBible
- 发现 OOC（Out of Character）问题

**输入**：
- CharacterBible（特定角色）
- ScriptDraft（涉及该角色的片段）

**输出**：
- `CharacterReview`（角色评审，以协作建议形式存储）

**权限**：
- 读取 CharacterBible、ScriptDraft
- 提出修改建议
- 不能直接修改剧本
- 不能生成视频、发布、删除或写入正式记忆

**PromptDefinition**：`character_critic.general@1`

---

## 角色协作规则

1. **产物所有权**：每个角色只拥有自己产出的结构化产物，不得直接覆盖其他角色产物
2. **修改建议机制**：角色通过 `collaboration_suggestions` 表提出修改建议，由产物所有者决定是否接受
3. **版本管理**：每个产物支持多版本（draft、approved、superseded），确保可追溯
4. **权限边界**：所有角色不得拥有发布、付费生成、删除或正式记忆写入权
5. **执行顺序**：Orchestrator 按依赖关系调度角色，确保输入产物已就绪
6. **人工介入点**：ScriptApprovalGate、QualityGate 等关键检查点需人工确认

## RoleDefinition Schema

```yaml
role_key: string              # 唯一标识，如 "producer"
role_name: string             # 展示名称，如 "制片人"
responsibilities: string[]    # 职责描述
input_artifacts: string[]     # 依赖的输入产物类型
output_artifacts: string[]    # 产出的产物类型
allowed_tools: string[]       # 允许使用的工具（当前为空，未来扩展）
prompt_definition_ref:        # Prompt 引用
  key: string                 # 如 "producer.general"
  version: string             # 如 "@1"
lifecycle: string             # candidate | active | supported | revoked
created_at: timestamp
updated_at: timestamp
```

## 角色执行流程

```
1. Orchestrator 选择下一个待执行角色
2. 检查输入产物是否就绪
3. 从 RoleRegistry 加载 RoleDefinition
4. 从 ProductionState 读取相关产物
5. 调用 PromptCompiler 编译角色 Prompt
6. 通过 AgentRunCoordinator 执行 ModelCall
7. 验证输出是否符合产物 Schema
8. 写入 ProductionState
9. 保存 ModelCall 审计快照
10. 触发下一个角色或 Gate
```
