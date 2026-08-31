# Generation Worker

Generation Worker 轮询 `generation-tasks`，只消费冻结为 `executionRoute=generation` 的
owner outbox。Text/Image/Video 的 provider、storage、catalog admission 和 ledger 均通过
activity dependency 注入；默认配置仍使用 Mock/Local，缺少 live 前置时保持
`unconfigured`，不会自动切换 adapter。
