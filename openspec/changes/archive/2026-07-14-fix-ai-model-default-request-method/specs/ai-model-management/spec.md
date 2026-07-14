## MODIFIED Requirements

### Requirement: 管理后台必须提供完整模型生命周期操作

`admin/` SHALL 通过模型管理 API 提供列表、筛选、创建、编辑、设为默认、启用、停用和删除操作，并 SHALL 使用版本号避免并发编辑互相覆盖。

#### Scenario: 设为默认使用公开 API 契约

- **GIVEN** 操作者读取了一个已启用非默认模型的当前 `version`
- **WHEN** 操作者点击“设为默认”
- **THEN** Admin SHALL `POST /api/admin/models/:model_id/default`
- **AND** 请求体 SHALL 包含当前 `version`
- **AND** Admin SHALL NOT 使用后端未注册的 `PUT` 方法

#### Scenario: 设为默认成功后刷新列表

- **WHEN** 默认模型切换 API 返回成功
- **THEN** Admin SHALL 重新加载模型列表
- **AND** 新默认模型 SHALL 显示默认标记
- **AND** 页面 SHALL NOT 显示请求失败错误
