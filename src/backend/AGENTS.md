# 后端约定

继承根目录 [AGENTS.md](../../AGENTS.md)；此处仅补充本目录约束。模块职责、接口及详细场景见 [docs/MODULES.md](../../docs/MODULES.md)。

- 协议基线为 Codex CLI **0.166.0**，字段以该版本生成的 JSON Schema 为准；升级须回归握手、真实流式回复和进程清理。启动即运行 `codex app-server --stdio`，按 initialize → initialized → thread/start 握手，同一角色会话复用一个 thread，切换角色新建 thread。默认继承本机登录、模型和权限；程序/工作目录可配置，相对目录基于资源根；桌面 PATH 缺失时补查 `~/.local/bin`、`~/.cargo/bin`。
- Unix 子进程必须使用独立进程组，退出/重启整组终止并回收；关闭 stdin 后再等待读线程，避免启动器后代持有 stdout 导致退出卡住。后台通过标准库事件通道回传，回调只唤醒 UI。
- 通知可能先于 turn/start ACK 到达：按 thread/turn 和请求 ID 关联，忽略旧 ACK，按 item ID 去重流式/最终文本。ACK 超时 30 秒，生成不限时；审批和问题交由 UI，文件 diff 从 item/started 缓存，未知请求返回 -32601。
- 配置保存须保留未知字段和旧密钥；损坏 JSON 拒绝覆盖，同目录临时文件同步后原子替换，Unix 权限 0600，成功后才更新内存。模型/API 表单仅点击保存时一次性校验写入，不能逐字保存；关闭设置丢弃未保存草稿。
- `codex.noApproval` 默认 false，从下一轮发送起生效，不重启、不改变当前任务或沙箱。缓存 thread/start 返回的原 approvalPolicy；开启显式传 never，关闭显式恢复原值（字符串或 granular 对象），不能省略字段恢复。
- 模型/API 位于“连接 → 模型与 API”：model/baseUrl/apiKey 默认空，useCustomApi 默认 false；自定义服务必须提供模型、HTTP(S) 基础地址和 Key，仅支持 Responses。通过进程级 `-c` 覆盖模型及 lexington_api 提供方，不修改用户 Codex 配置；Key 仅经 Command.env 传入，禁止进入 argv/Debug/日志；关闭自定义模式显式 env_remove，协议输出完整 Key 遮蔽。
- 本机配置只读 `CODEX_HOME/config.toml`（默认 `~/.codex`），支持 profile/provider；Key 来源及继承规则见模块文档，不得把 OAuth token 当 API Key。读取仅填草稿；保存继承设置不固定未修改的模型、不复制本机 Key，并保留未启用的自定义凭据。解析错误不得包含源文本。
- 立绘只解析文件名最后的 `_Normal` / `_Damage`（ASCII 大小写不敏感），前面完整服装名用于同角色、同格式配对，无后缀按正常。↻ 切换对应正常/战损，缺对禁用；▶ 仅循环当前状态队列，默认正常；格式切换尽量保留服装和状态，回退优先保留状态。重载入口在“设置 → 常规 → 重新加载”。
- 自动打卡每天本地日期一次，默认开启；持续活跃确认默认 180 秒（30～600），回溯首次新操作，终点须有新操作。闲置 60 秒、锁屏/非活动/远程会话、采样失败或断档 >10 秒、时钟跳变均重置；热加载开关/时长也重置，不追溯启动前工作。
- 打卡/闹钟记录在 XDG 数据目录，普通/开发隔离；状态原子保存成功后才提交内存，损坏文件不得覆盖。打卡取消/撤销/午休/重启后当天不再自动开始，次日重置；编辑当天 HH:MM 拒绝未来/DST 不存在时间；提醒后保存已提醒状态，重启不重复。
- 闹钟到期先保存 pending 和下次时间/一次性停用状态，确认保存成功才关闭；重启保留未确认提醒。每日按本地日历 mktime 计算，不能固定加 86400 秒；停机/休眠错过每个补一次并推进到未来。HH:MM 严格校验，提示语 1～500 字符。
- Linux 单实例和 Unix socket Inbox 按普通/开发隔离；目录 0700/当前用户/非软链接，socket 0600，flock 独占和 SO_PEERCRED 校验。4 字节小端长度 + JSON，≤1 MiB、32 路径、16 待处理，读写超时；无 HTTP/TCP、URL scheme 或剪贴板监听。外部交付自动启动，ACK 只表示入队，发出后不重试；UI 空闲直接发送，复用 thread/审批，不改焦点/草稿，错误用气泡展示；native-messaging stdout 仅输出协议帧。

## 增量验证

- 角色映射、角色卡和提示组装由平级 roleplay 模块负责；后端保持 Codex 进程/协议/会话/审批职责。组装后的角色提示作为 thread/start.developerInstructions 注入一次；匹配世界书作为本轮独立文本块携带，原始用户输入保留为第一个文本块。不能替换 baseInstructions 或扩大审批/沙箱权限。新增角色数据无需 Codex 分支。
- 上下文规划在确认可以发送新任务时执行，实际发送后回传 Context 报告；预览只操作会话副本，不调用模型。失败/中断回滚角色会话快照，最终回复去重后进入最近历史；Context 内容同错误/回复一样遮蔽已知 API Key，不能写入日志。
- 向量记忆的配置、存储调用与 Embedding 请求属于 roleplay；后端只绑定会话角色/根目录并沿用 begin/finish/preview。已知自定义对话 API Key 在用户输入和最终回复进入角色历史之前遮蔽，原始用户协议输入不改写。仅成功 finish 允许归档，失败/中断不得持久化本轮内容。
- 结构化后台任务使用独立临时 cwd、ephemeral 只读线程和 outputSchema，复用通用连接/代理与进程树清理；不改正常会话权限或全局配置。禁用工具和交互，MCP 需通过 config/read 获取名称后用嵌套 mcp_servers 对象逐个覆盖 enabled/required=false；空表不能清空继承项，不能回传含 null 的完整配置。临时目录在子进程回收后释放。

- Windows Codex 仅运行原生 exe，显式 .cmd/.bat 在启动前拒绝；进程以挂起状态加入 kill-on-close Job 后再恢复，退出清理后代。协议夹具必须显式 UTF-8，目录身份比较用 samefile，不能依赖扩展路径前缀的字符串表示。
- Windows 的 Codex 子进程通过 tools::proxy 桥接系统手动代理；继承登录和自定义 API 都适用，已有代理环境变量优先。系统代理变更需重启会话，不修改本机 Codex 配置或登录。
- Windows Inbox 使用当前用户及 SYSTEM 的私有命名管道；非阻塞空读先用 PeekNamedPipe 区分暂时无数据和断开，ACK 后等待客户端关闭再 DisconnectNamedPipe。native-host 别名拉起同目录 lexington.exe，不得递归启动自身。

- 按受影响模块筛选，如 `cargo test --locked backend::codex::tests::`、`cargo test --locked backend::clock::tests::`；集中配置测试用 `cargo test --locked backend::tests::`，也可按具体用例名筛选。涉及 UI 消费协议/状态时补测对应 UI 用例。
- 测试使用临时路径、环境和 Python 3 模拟进程，不访问真实模型、登录或用户记录。`Pet::from_root` 不启动监测或读取用户记录/Codex 配置；资源数量/配对由角色模块检查。

- 天气仅在显式请求时通过 ureq/Rustls 查询 Open-Meteo，不接入 Codex；缓存按城市/日期区分，响应有大小及总超时限制，退出丢弃队列并回收工作线程。城市保存在 weather.city，沿用配置无损原子写入。
- 立绘扫描同时包含 resource/illustration 与 config/characters，忽略隐藏暂存目录；两处沿用相同格式、服装与战损解析，不读取角色设定或写入导入数据。
