# Agent Worker

Agent Worker 使用锁定的 AgentScope `2.x` runtime，并在连接 Temporal 前校验 Skill Registry
`index.yaml` 与默认 approved metadata。启动阶段不读取 `SKILL.md`/references；只有路由确定后，
API/Worker 才按 exact revision 与 digest 加载所选 runtime policy snapshot。

Worker 不拥有 Provider/Profile/Model catalog、Run 事件或媒体事实，也不直接构造真实模型调用。
缺少真实 Profile/credential/master key 时保持 `agentscope_text_runtime_unconfigured`，默认测试只走
Mock Provider 与显式 `local_workspace` profile。
