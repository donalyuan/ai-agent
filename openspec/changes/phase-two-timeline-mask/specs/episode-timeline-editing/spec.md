## MODIFIED Requirements

### Requirement:整数帧 clip 编辑
系统 SHALL 仅以整数帧存储 Clip 的排序、source in/out、timeline start 与 duration，并以单一静态值存储 `position`、`scale`、`opacity`。在独立 `phase-two-timeline-mask` change 启用后，Clip 可通过版本化 mask/track matte command 扩展视觉合成引用；基础 Timeline command 仍必须经过 schema 校验并编译到 canonical RenderPlan，`DeleteClip` 仍只删除 Timeline reference。

#### Scenario:基础编辑与 mask 引用分离
- **WHEN** 用户对 Clip 提交基础编辑或 mask 引用
- **THEN** 两类命令分别校验各自 capability 和 revision，基础编辑不会隐式创建 mask 或覆盖 AssetVersion
