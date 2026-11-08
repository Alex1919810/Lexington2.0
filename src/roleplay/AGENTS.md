# 角色扮演模块约定

继承根目录 [AGENTS.md](../../AGENTS.md)，职责与接口见 [docs/MODULES.md](../../docs/MODULES.md)。

- 本模块与 backend 平级，保存角色卡、角色映射、提示组装、世界书匹配及角色配置持久化；不依赖 backend、UI、egui，不启动 Codex 或访问其登录信息。
- Codex 进程、会话、协议、审批和权限属于 backend；UI 只编辑草稿及请求保存。角色内容不增加权限。
- 默认资源放在 resource/agents，用户修改放在 config/roleplay.json，保存须保留未知字段与其他角色，原子写入，损坏文件拒绝覆盖。来源和许可不注入模型。
- 使用临时资源验证默认/覆盖优先级、规则与世界书匹配、文件错误及保存保护，不调用真实模型、凭据或用户记录。
