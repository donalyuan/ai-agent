## MODIFIED Requirements

### Requirement:整数帧 clip 编辑
系统 SHALL 仅以整数帧存储 Clip 的排序、source in/out、timeline start 与 duration，并以单一静态值存储 `position`、`scale`、`opacity`。MVP-B 额外允许版本化 keyframe、speed、loop 和 snap metadata，但这些字段 MUST 经过 schema 校验并编译到 canonical RenderPlan。系统 MUST 支持显式排序、裁剪、拆分、复制、吸附和 `DeleteClip`，并保留来源关系；`DeleteClip` MUST 只删除 Timeline reference，绝不删除或覆盖 AssetVersion。不得接受浮点帧、负值、零长度或越界裁剪。

#### Scenario:使用关键帧和吸附编辑
- **WHEN** 用户以当前 revision 对 Clip 提交合法关键帧或 snap 命令
- **THEN** 系统持久化新 current revision 和 RenderPlan hash，旧 TimelineVersion 不变

#### Scenario:拒绝不受支持的帧数据
- **WHEN** 请求包含浮点帧、无界循环、非法 speed 或 keyframe 越界
- **THEN** 返回 validation，不写入部分编辑或导出任务
