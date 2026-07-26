## ADDED Requirements

### Requirement: Rust 生产节点必须统一通过受治理 Context Compiler

全部 Rust 生产 LLM node SHALL 由业务 Adapter 提供原子 ContextCandidate，并 SHALL 通过 `ContextCompiler + PromptCompiler + AuditedModelExecutor` 唯一入口执行；Adapter SHALL NOT 自行决定最终选择、token 预算或任意文本裁剪。

#### Scenario: Rust Adapter 执行 LLM node
- **WHEN** 项目策略、脚本、选题、质量评审/重写、主题组评审、声音或作品 node 准备调用模型
- **THEN** Adapter SHALL 只读取其拥有的领域来源并提交结构化候选
- **AND** 通用执行层 SHALL 解析固定 Policy/tokenizer、编译 Context、保存 ContextSnapshot 后创建 ModelCall
- **AND** 现有领域 Run/Step、输出解析、事务和 Gate SHALL 继续由 Adapter/Coordinator 拥有

#### Scenario: 静态检查旧 Context 路径
- **WHEN** CI 扫描 Rust 生产代码
- **THEN** SHALL 不存在 Adapter 手工字符截断、整段预格式化 Prompt fragment、绕过 Context Compiler 的 PromptCompileInput 或裸模型入口
- **AND** 测试 fixture 与 provider contract 之外的生产路径 SHALL 只有一个 Context/Prompt/ModelCall 执行链

