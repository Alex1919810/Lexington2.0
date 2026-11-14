# 后端约定

继承根目录 [AGENTS.md](../../AGENTS.md)；此处仅补充本目录约束。模块与功能见 [docs/MODULES.md](../../docs/MODULES.md)；接口约束、实现细节及验证场景见 [docs/DEVELOPMENT_NOTES.md](../../docs/DEVELOPMENT_NOTES.md)。

- 协议基线为 Codex CLI **0.167.0**，字段以该版本生成的 JSON Schema 为准；升级须回归握手、真实流式回复和进程清理。启动即运行 `codex app-server --stdio`，按 initialize → initialized → thread/start 握手，同一角色会话复用一个 thread，切换角色新建 thread。默认继承本机登录、模型和权限；程序路径由环境 `LEXINGTON_CODEX_EXECUTABLE` 指定，工作目录由所选工作区决定；桌面 PATH 缺失时补查 `~/.local/bin`、`~/.cargo/bin`。
- Unix 子进程必须使用独立进程组，退出/重启整组终止并回收；关闭 stdin 后再等待读线程，避免启动器后代持有 stdout 导致退出卡住。后台通过标准库事件通道回传，回调只唤醒 UI。
- 通知可能先于 turn/start ACK 到达：按 thread/turn 和请求 ID 关联，忽略旧 ACK，按 item ID 去重流式/最终文本。ACK 超时 30 秒，生成不限时；审批和问题交由 UI，文件 diff 从 item/started 缓存，未知请求返回 -32601。
- 配置保存须保留未知字段和旧密钥；损坏 JSON 拒绝覆盖，同目录临时文件同步后原子替换，Unix 权限 0600，成功后才更新内存。应用开关仅保存对应字段，不重写外部 Codex 连接参数。
- `codex.noApproval` 默认 false，从下一轮发送起生效，不重启、不改变当前任务或沙箱。缓存 thread/start 返回的原 approvalPolicy；开启显式传 never，关闭显式恢复原值（字符串或 granular 对象），不能省略字段恢复。
- 会话初始模型、API、登录与提供方配置继承当前环境及本机 Codex；/model 可在当前会话选择模型与推理强度，不能写本机 Codex 配置或覆盖提供方；config.json 的旧 executable/workingDirectory/model/useCustomApi/baseUrl/apiKey 不再加载或覆盖。仅保存 Lexington 的 noApproval/developmentMode；后者默认关闭，改变后须新建会话，不能恢复旧模式线程。Windows 环境指定的 exe 优先于默认程序查找。
- Windows 缺少本机原生 CLI 时回退到当前 EXE 旁 codex/vendor/<target>/bin/codex.exe；安装目录从 current_exe 定位，不能依赖工作区或 LEXINGTON_HOME。随包 CLI 沿用用户已有 CODEX_HOME/登录，不创建替代凭据或修改系统 PATH。
- codex_config 读取并识别原生 Codex 提供方，也接受设置界面的显式保存；不得创建 Lexington 专用连接配置或修改进程/系统环境变量。保存 config.toml 保留注释、未知字段及其他提供方，校验读取快照以拒绝覆盖外部变更，同目录原子替换；不改写 auth.json、OAuth tokens 或已有登录。API Key 只写原生提供方 experimental_bearer_token，日志/Debug/错误不得带出；标准环境 Key 与当前提供方 Key 在文本、上下文、审批、执行事件及持久化前遮蔽。设置保存通过仅运行时 connection_revision 隔离旧后台队列；config.json 热加载须保留该修订号。测试仅使用临时目录和显式注入的凭据来源，不读取真实用户环境 Key。
- 所选工作区的任务可通过 thread/resume 恢复，返回 thread ID 绑定当前任务；恢复失败明确报错，用户新建任务可重新开始，不静默跨项目回退。开发指令从 development 模块取得，只有 developmentMode 开启时附加。
- 立绘只解析文件名最后的 `_Normal` / `_Damage`（ASCII 大小写不敏感），前面完整服装名用于同角色、同格式配对，无后缀按正常。↻ 切换对应正常/战损，缺对禁用；▶ 仅循环当前状态队列，默认正常；格式切换尽量保留服装和状态，回退优先保留状态。重载入口在“设置 → 常规 → 重新加载”。
- 自动打卡每天本地日期一次，默认开启；持续活跃确认默认 180 秒（30～600），回溯首次新操作，终点须有新操作。闲置 60 秒、锁屏/非活动/远程会话、采样失败或断档 >10 秒、时钟跳变均重置；热加载开关/时长也重置，不追溯启动前工作。
- 打卡/闹钟记录在 XDG 数据目录，普通/开发隔离；状态原子保存成功后才提交内存，损坏文件不得覆盖。打卡取消/撤销/午休/重启后当天不再自动开始，次日重置；编辑当天 HH:MM 拒绝未来/DST 不存在时间；提醒后保存已提醒状态，重启不重复。
- 闹钟到期先保存 pending 和下次时间/一次性停用状态，确认保存成功才关闭；重启保留未确认提醒。每日按本地日历 mktime 计算，不能固定加 86400 秒；停机/休眠错过每个补一次并推进到未来。HH:MM 严格校验，提示语 1～500 字符。
- Linux 单实例和 Unix socket Inbox 按普通/开发隔离；目录 0700/当前用户/非软链接，socket 0600，flock 独占和 SO_PEERCRED 校验。4 字节小端长度 + JSON，≤1 MiB、32 路径、16 待处理，读写超时；无 HTTP/TCP、URL scheme 或剪贴板监听。外部交付自动启动，ACK 只表示入队，发出后不重试；UI 空闲直接发送，复用 thread/审批，不改焦点/草稿，错误用气泡展示；native-messaging stdout 仅输出协议帧。

## 增量验证

- 角色映射、角色卡和提示组装由平级 roleplay 模块负责；后端保持 Codex 进程/协议/会话/审批职责。组装后的角色提示作为 thread/start.developerInstructions 注入一次；匹配世界书作为本轮独立文本块携带，原始用户输入保留为第一个文本块。不能替换 baseInstructions 或扩大审批/沙箱权限。新增角色数据无需 Codex 分支。
- 上下文规划在确认可以发送新任务时执行，实际发送后回传 Context 报告；预览只操作会话副本，不调用模型。失败/中断回滚角色会话快照，最终回复去重后进入最近历史；Context 内容同错误/回复一样遮蔽已知 API Key，不能写入日志。
- 向量记忆的配置、存储调用与 Embedding 请求属于 roleplay；后端只绑定会话角色/根目录并沿用 begin/finish/preview。已知当前提供方 API Key 在用户输入和最终回复进入角色历史之前遮蔽，原始用户协议输入不改写。仅成功 finish 允许归档，失败/中断不得持久化本轮内容。
- 结构化后台任务使用独立临时 cwd、ephemeral 只读线程和 outputSchema，复用通用连接/代理与进程树清理；不改正常会话权限或全局配置。禁用工具和交互，MCP 需通过 config/read 获取名称后用嵌套 mcp_servers 对象逐个覆盖 enabled/required=false；空表不能清空继承项，不能回传含 null 的完整配置。临时目录在子进程回收后释放。

- Windows Codex 仅运行原生 exe，显式 .cmd/.bat 在启动前拒绝；进程以挂起状态加入 kill-on-close Job 后再恢复，退出清理后代。协议夹具必须显式 UTF-8，目录身份比较用 samefile，不能依赖扩展路径前缀的字符串表示。
- Windows 的 Codex 子进程通过 tools::proxy 桥接系统手动代理；继承外部 Codex 环境时适用，已有代理环境变量优先。系统代理变更需重启会话，不修改本机 Codex 配置或登录。
- Windows Inbox 使用当前用户及 SYSTEM 的私有命名管道；非阻塞空读先用 PeekNamedPipe 区分暂时无数据和断开，ACK 后等待客户端关闭再 DisconnectNamedPipe。native-host 别名拉起同目录 lexington.exe，不得递归启动自身。

- 按受影响模块筛选，如 `cargo test --locked backend::codex::tests::`、`cargo test --locked backend::clock::tests::`；集中配置测试用 `cargo test --locked backend::tests::`，也可按具体用例名筛选。涉及 UI 消费协议/状态时补测对应 UI 用例。
- 测试使用临时路径、环境和 Python 3 模拟进程，不访问真实模型、登录或用户记录。`Wife::from_root` 不启动监测或读取用户记录/Codex 配置；资源数量/配对由角色模块检查。

- 天气仅在显式请求时通过 ureq/Rustls 查询 Open-Meteo，不接入 Codex；缓存按城市/日期区分，响应有大小及总超时限制，退出丢弃队列并回收工作线程。城市保存在 weather.city，沿用配置无损原子写入。
- 立绘扫描同时包含 resource/illustration 与 config/characters，忽略隐藏暂存目录；两处沿用相同格式、服装与战损解析，不读取角色设定或写入导入数据。
- Live2D 使用角色 live2d/<换装>_Normal（或 _Damage）/actor.json，换装配对沿用最终后缀规则；只发现模型路径，不渲染。切换格式优先同换装且保持角色会话，三格式回退必须覆盖由 Live2D 切到仅有 PNG 的角色。

- Client::run_test 使用 command/exec 直接执行显式选择的 argv，省略 sandboxPolicy/env 以继承本机配置，不发送模型 turn。processId 用于 terminate；长命令不受 30 秒 ACK 限制，独立执行按配置超时及有界输出处理。TestResult 与普通 turn 事件分开，错误/取消/缺退出码不得视为通过。

- 运行中输入通过 turn/steer + expectedTurnId 追加到当前轮次，独立 Steered 结果确认成功或失败；不能借此创建新轮次、恢复已结束轮次或改变权限。补充 ACK 可以晚于完成通知；补充拒绝/编号不匹配/确认超时不终止原任务，迟到 ACK 不应影响后续请求。

- 备忘录由 memo::MemoState 独立管理，UI 注入普通/--dev 数据路径，各角色共用；正文按 Unicode 字符校验。临时文件同步后原子替换，成功才更新内存；加载须校验版本、唯一 ID 和正文，错误不能以空记录覆盖。

- 发送清单在 turn/start 写入成功后才回传，不能把通道入队或预览当作已发送；Project/Roleplay 正文由同一清单选择生成，静态指令仅登记沿用。新清单和日志不得旁路凭据遮蔽。
- 结构化后台工作使用 jobs::StructuredJobs，任务绑定 owner 与连接快照，轮询有帧预算；先释放 Client/临时 cwd 再交付终态。普通对话和测试执行不迁入后台队列，角色模块不持有进程。

- app-server 不解析 CLI 斜杠文本；客户端指令用独立 run_command 入口，查询依据 thread/start/resume 的实际设置与本线程 tokenUsage 通知；/status 的账号余额须额外按需调用 account/rateLimits/read，不能从 token 用量估算，也不能把 secondary 固定当作每周额度，必须按 windowDurationMins 判断。多分组视图优先、旧单分组仅回退；接口失败/超时保留会话，取消后丢弃迟到回复。结果沿已遮蔽 Text + CommandFinished 返回，不创建模型 turn、不推进角色历史或伪造发送清单；候选与识别共用 commands 模块，未接入指令明确提示。

- 定时任务使用 schedule::Executor 的独立普通 Client，每次新线程，显式 approvalPolicy=never（用户已要求无需审批），不改全局 noApproval 或沙箱；不能迁入禁工具的 StructuredJobs。持久化 begin 先于发送，Manifest 才标已发送，终态整体遮蔽回复后写历史；异常退出的未确认运行不得自动重试。

- `/model` 交互目录只通过按需 model/list 获取并遍历分页，取消移除请求 ID，错误不得终止连接；SelectModel 必须在空闲时按已读取目录校验，独立 ModelSelected 确认，不借此创建 turn。选择仅缓存于当前 Client，后续 turn/start 明确传 model/effort，保持原始审批基线和沙箱；Panel/错误沿用凭据遮蔽。

- 日历加班记录独立于每天重置的 ClockState，自动记录保存下班基准与加班打卡时间快照，手动记录单独保存分钟数（每日 0～1440），两者互斥且兼容既有自动记录，不伪造手动上下班时间；重复日期替换，月统计累计各日完整分钟，不能按点击次数累加或用后来配置重算历史。日历公历网格用年月日运算，避免以固定秒数计算月份/星期。

- 日历节日由 calendar::month_festivals 离线计算，与加班记录及法定放假/调休独立；传统节日须枚举前后农历年再按公历年月筛选，避免漏掉一月除夕/春节，同时保留同日多个节日，不能用固定公历日期代替农历或清明节气。
