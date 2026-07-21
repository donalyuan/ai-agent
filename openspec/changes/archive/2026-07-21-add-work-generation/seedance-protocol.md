# Seedance 官方协议核验（2026-07-18）

核验来源均为火山引擎方舟官方文档：

- 创建任务：`https://www.volcengine.com/docs/82379/1520757`
- 查询任务：`https://www.volcengine.com/docs/82379/1521309`
- 取消或删除任务：`https://www.volcengine.com/docs/82379/1521720`

实施契约：

- 创建：`POST https://ark.cn-beijing.volces.com/api/v3/contents/generations/tasks`。
- 查询：`GET https://ark.cn-beijing.volces.com/api/v3/contents/generations/tasks/{id}`。
- 取消或删除：`DELETE https://ark.cn-beijing.volces.com/api/v3/contents/generations/tasks/{id}`。
- 输入使用 `content[]`；文本项为 `type=text/text`。Seedance 2.0 多参考图为 `type=image_url/image_url.url/role=reference_image`，支持 `1~9` 张。
- Seedance 1.5 使用单首帧或 `first_frame/last_frame`，时长为整数 `4~12s`，支持 `480p/720p/1080p`；不得承载 6 张 `reference_image` 请求。多分镜单任务时只将首尾两张发给 provider，其余分镜保留为提示词语义。
- `generate_audio` 为 boolean；独立 TTS 模式明确发送 `false`，原声相关模式明确发送 `true`，不依赖上游默认值。
- 创建参数使用独立 `resolution`、`ratio`、`duration` 字段；输出由查询响应的 `content.video_url` 获取。
- 当前官方样例已包含 `doubao-seedance-2-0-260128`，并支持参考视频/参考音频；本 change 首版不接入这两类输入，保持已确认 Non-Goals。
- 取消接口与删除共用 `DELETE`，Provider 必须把本地 `cancelled` 与上游删除响应审计关联，不能伪造已取消。

真实调用仍受单独许可约束；本次仅核验文档和 fake provider contract，未产生外部费用。
