## ADDED Requirements

### Requirement: Full Crew ScriptPackage 必须确定性晋升为正式脚本

Full Crew 的 screenwriter 输出 SHALL 包含正式脚本映射所需的 `title`、`hook`，以及每个 scene 的 `sequence`、`narration`、`visual_description`、`emotion` 和 `duration_sec`；系统 SHALL 对 StoryBible、CharacterBible 和 ScriptDraft 组成的精确 ScriptPackage 做完整 schema 与引用校验，并 SHALL 在包级批准后以零额外模型调用确定性创建正式脚本和分镜。

#### Scenario: 编剧输出满足正式字段契约

- **WHEN** screenwriter 完成 Full Crew role step
- **THEN** ScriptDraft SHALL 包含非空 title、hook 和有序 scenes
- **AND** 每个 scene SHALL 包含正式 Scene 所需全部字段并满足顺序、数量和时长约束
- **AND** StoryBible、CharacterBible 和 ScriptDraft SHALL 关联同一 role attempt、ModelCall 和 package 版本

#### Scenario: 正式字段缺失

- **WHEN** ScriptDraft 缺少任一正式字段、scene 顺序不连续或字段违反领域约束
- **THEN** role step SHALL 失败并记录 schema 错误
- **AND** 系统 SHALL NOT保存部分 ScriptPackage
- **AND** 系统 SHALL NOT创建正式脚本或调用另一个 LLM 修补输出

#### Scenario: 批准 ScriptPackage 后晋升

- **GIVEN** 当前 ScriptPackage digest 已通过人工 Gate 且来源选题仍可晋升
- **WHEN** 系统执行 ScriptPackagePromotion
- **THEN** 系统 SHALL 创建状态为 `approved` 的正式 Script 和全部 Scene
- **AND** Script SHALL 保存 project_id、topic_id、topic_snapshot、production ID、package digest 和来源产物引用
- **AND** 系统 SHALL NOT要求操作者再次批准同一脚本
- **AND** 晋升过程 SHALL NOT调用模型

#### Scenario: 晋升操作重复提交

- **GIVEN** 相同 ProductionRun、ScriptPackage digest 和晋升幂等键已经成功创建正式脚本
- **WHEN** 客户端重试晋升命令
- **THEN** 系统 SHALL 返回原 Script 和 Scene 集合
- **AND** 系统 SHALL NOT创建第二个脚本、分镜或新版本

#### Scenario: 旧 ScriptPackage 不得晋升

- **GIVEN** ScriptPackage 获批后任一组成产物产生新版本
- **WHEN** 客户端请求晋升旧 package digest
- **THEN** 系统 SHALL 返回 `stale_package`
- **AND** 系统 SHALL NOT创建或修改正式 Script/Scene

### Requirement: Full Crew 后续产物不得静默修改已批准脚本

正式脚本晋升后，Director 和其他制作角色 SHALL 使用真实 Script/Scene ID 作为输入和引用；需要改变旁白、分镜语义或 Scene 结构时，系统 SHALL 通过现有脚本版本关系创建新的 Script，而 SHALL NOT直接覆盖已批准 Script/Scene。

#### Scenario: ShotContract 引用正式 Scene

- **GIVEN** ScriptPackage 已晋升为正式 Script/Scene
- **WHEN** Director 生成 ShotContract
- **THEN** 每个 ShotContract SHALL 引用存在且属于该 Script 的真实 `scenes.id`
- **AND** 系统 SHALL 拒绝自由字符串、跨脚本 Scene 或无法解析的 scene reference

#### Scenario: 导演修改已批准脚本语义

- **WHEN** Director 建议改变已批准旁白、Scene 顺序或核心叙事内容
- **THEN** 系统 SHALL 要求创建带 `parent_id` 的新 Script 版本并重新经过相应 Gate
- **AND** 原 Script、Scene、来源快照和下游审计 SHALL 保持不变
- **AND** 依赖旧 Script 的 ProductionPackage 和 WorkPlan SHALL 失效

#### Scenario: 脚本语义修订必须重新形成 ScriptPackage

- **GIVEN** 正式 Script 已由 Full Crew 晋升
- **WHEN** 操作者接受旁白、Scene 顺序、Scene 结构或核心叙事修改
- **THEN** 系统 SHALL 创建新的 script revision epoch 并由 screenwriter 生成 StoryBible、CharacterBible 和 ScriptDraft 一致版本集合
- **AND** 新 ScriptPackage SHALL 重新通过包级审批后才能确定性晋升
- **AND** 新 Script SHALL 以 `parent_id` 引用原 Script
- **AND** Director 或其他下游角色 SHALL NOT直接写入 Script/Scene

#### Scenario: 新脚本晋升使旧下游失效

- **GIVEN** 新 ScriptPackage 已批准并成功晋升为子 Script
- **WHEN** 系统提交晋升事务
- **THEN** 旧 Script SHALL 保持 approved 历史事实且不得被覆盖
- **AND** 当前 ProductionRun SHALL 将正式脚本关联切换到新 Script
- **AND** 依赖旧 Script 的 ProductionPackage、SceneVisualManifest 关联、WorkVersion 草稿和未确认 WorkPlan SHALL 失效
- **AND** 已确认或已运行的 WorkVersion SHALL 保持不可变并进入显式重新制作决策

### Requirement: Full Crew ScriptPackage reject 必须保持来源和事务边界

ScriptPackage 被拒绝时，系统 SHALL 保留原 package、GateDecision、role attempt 和 ModelCall，创建新的 screenwriter revision step，并 SHALL NOT修改 Topic、创建正式 Script/Scene 或复用旧 approval。达到固定修订上限后 SHALL 停止模型调用并要求取消或新建制作意图。

#### Scenario: 拒绝后生成新 ScriptPackage

- **WHEN** 操作者以非空理由拒绝当前 ScriptPackage
- **THEN** 原 package SHALL 保持不可变
- **AND** 新 screenwriter attempt SHALL 使用新的 revision epoch 和独立资源预占
- **AND** 只有新 package digest 的批准才能触发晋升

#### Scenario: 重放旧 ScriptPackage approval

- **GIVEN** ScriptPackage 已被 reject 且存在更新 revision epoch
- **WHEN** 客户端重放旧 digest 的 approve 或 promotion 命令
- **THEN** 系统 SHALL 返回 `stale_package`
- **AND** 系统 SHALL NOT修改当前 revision 或 Topic 状态
