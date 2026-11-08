# 模块职责

代码按 UI、后端功能、角色扮演和工具代码组织。`main.rs` 负责启动，UI 负责展示与交互，后端负责业务规则及基本 Codex 功能，平级的 roleplay 模块负责角色卡与角色提示，工具提供路径、解码和平台适配能力。

## 目录结构

```text
src/
├── main.rs
├── ui/
│   ├── AGENTS.md
│   ├── mod.rs
│   ├── alarm.rs
│   ├── app.rs
│   ├── pet.rs
│   ├── sidebar.rs
│   ├── settings.rs
│   ├── role_card.rs
│   ├── roleplay_context.rs
│   ├── roleplay_memory.rs
│   ├── dialog.rs
│   ├── clock.rs
│   ├── codex.rs
│   ├── input.rs
│   ├── assets.rs
│   ├── style.rs
│   ├── weather.rs
│   └── tests.rs
├── backend/
│   ├── AGENTS.md
│   ├── mod.rs
│   ├── alarm.rs
│   ├── config.rs
│   ├── resources.rs
│   ├── weather.rs
│   ├── character.rs
│   ├── codex.rs
│   ├── codex_config.rs
│   ├── handoff.rs
│   ├── handoff_windows.rs
│   └── clock.rs
├── roleplay/
│   ├── AGENTS.md
│   ├── mod.rs
│   ├── card.rs
│   ├── context.rs
│   ├── profile.rs
│   ├── registry.rs
│   └── store.rs
└── tools/
    ├── AGENTS.md
    ├── mod.rs
    ├── paths.rs
    ├── activity.rs
    ├── gif.rs
    ├── platform.rs
    ├── platform_windows.rs
    ├── proxy.rs
    ├── time.rs
    └── windows.rs
tests/
└── fixtures/
    └── codex-server.py
integrations/
├── README.md
├── browser/
│   ├── background.js
│   ├── manifest.firefox.json
│   └── manifest.chromium.json
├── vscode/
│   ├── package.json
│   └── extension.js
└── nautilus/
    ├── send.py
    └── lexington_menu.py
scripts/
├── AGENTS.md
├── install-linux.sh
├── install-windows.ps1
├── package-windows.ps1
├── test-windows.ps1
├── test-package.sh
├── windows-smoke.py
├── install-dependencies.sh
├── dependencies.py
├── test-dependencies.py
├── package.sh
├── integrations.py
├── context-smoke.py
├── test-integrations.js
├── settings-smoke.py
└── native-smoke.py
docs/
├── MODULES.md
└── agents/
    ├── AGENTS.md
    └── lexington/
        └── PERSONA.md
resource/agents/
├── characters.json
├── lexington.md
└── lexington.json
```

## Agent 角色设定资料

`docs/agents/` 保存产品内角色的设计，局部维护规则见 [AGENTS.md](agents/AGENTS.md)。[Lexington 设定](agents/lexington/PERSONA.md) 定义人格、语气、默认“太太”关系及交互约定。默认角色卡为 [resource/agents/lexington.json](../resource/agents/lexington.json)，兼容 [旧式 Markdown](../resource/agents/lexington.md)，映射见 [characters.json](../resource/agents/characters.json)。运行时角色资源随 `resource/` 安装和打包，设计文档目录不参与分发。

角色背景及原台词摘句参考舰R百科与页面贡献者的[列克星敦 CV-2 固定版本 125209](https://www.shipgirlfriends.moe/index.php?title=%E5%88%97%E5%85%8B%E6%98%9F%E6%95%A6%28CV-2%29&oldid=125209)，角色来自《战舰少女R》。默认太太关系、桌面日常与新增对话示例为项目改编或二创；角色文字改编按 [CC BY-NC-SA 4.0](https://creativecommons.org/licenses/by-nc-sa/4.0/deed.zh-hans) 提供，不改变其他文件许可。这些维护信息不注入模型。

### 角色映射与会话生命周期

`src/roleplay/` 是与 backend 平级的领域模块，局部规则见 [角色扮演约定](../src/roleplay/AGENTS.md)。不依赖后端、UI 或 egui，不管理 Codex 进程。

| 文件 | 职责和接口 |
|---|---|
| `roleplay/mod.rs` | 导出 RoleCard、Rule、WorldEntry、RoleplaySession、ContextPlan、UserProfile、Memory、agent_instructions、session_card、default_character、static_plan、save_role_card |
| `roleplay/registry.rs` | 按 resource/agents/characters.json 映射角色，校验目录边界和 .md 相对路径；有结构化卡时生成提示，否则读取旧 Markdown；文件上限 64 KiB，拒绝缺失、空白、错误编码与非法路径 |
| `roleplay/card.rs` | 角色名称、描述、性格、场景、关系、对话示例、用户人设、手动记忆、世界书及附加规则；保留核心设定、编译规则与分段示例，包含上下文预算和世界书持续/冷却配置；卡片最多 64 KiB，世界书和规则各最多 64 条；Debug 隐藏内容 |
| `roleplay/store.rs` | 从 config/roleplay.json 的 roleCards 分组读取按角色保存的覆盖值；角色卡和用户档案复用按分组保存接口，保留未知字段和其他角色，同目录临时文件同步后原子替换，Unix 权限 0600，损坏文件拒绝覆盖；不读写基本 Codex 配置或凭据 |
| `ui/role_card.rs` | RoleEditor 保存编辑对象、草稿、分区、错误和提示；角色卡页面编辑五类内容，支持世界书新增/删除和规则新增/启停/排序/删除；保存后才提交角色领域模块，取消/关闭/切换编辑对象丢弃草稿 |

优先级：用户保存的角色卡 → 映射 Markdown 同名的 .json 默认卡 → 原 Markdown。未映射且没有用户卡的角色使用新的普通会话。默认角色卡为 resource/agents/lexington.json，默认太太关系、原台词摘句、二创日常和多轮对话示例分字段保存；来源与许可等维护信息不注入模型。JSON 和 Markdown 随现有资源分发；用户角色配置不作为内置资源覆盖。

backend/character.rs 继续负责立绘、格式和台词选择；首次选择通过 roleplay::default_character 优先匹配 lexington，否则回退首个可用角色，已有有效选择保持。角色领域数据无需增加 Rust 角色专属分支。

backend/codex.rs 的 Client::start(config, root, character, wake) 取得组装了用户档案的角色卡并创建 RoleplaySession；静态计划通过 thread/start.developerInstructions 发送一次，本轮计划作为 turn/start.input 的独立文本块，原始用户消息仍为第一个文本块，审批和沙箱流程保持。实际写入请求后回传 Context 报告；预览不发送模型请求。

ui/app.rs 保存 codex_character、codex_role_card 和角色配置文件时间戳；设置窗口新增第 5 号“角色卡”页面（ui/settings.rs），操作在 viewport 回调结束后执行。每 500ms 检查 config/roleplay.json 和 config/roleplay-memory.json；仅当前角色卡与会话快照不同时排队重建，任务结束后应用，文件错误保留现有会话并展示错误。当前角色没有启动过 Codex 时不因编辑而启动真实服务。

切换显示角色（ui/sidebar.rs 或资源刷新回退）立即结束旧任务并创建新会话，清理回复、气泡、审批及重启标记，保留未发送草稿；同角色换装、战损与格式变化保持会话。ui/codex.rs 在启动时记录新的角色卡和文件快照。手动重启重新读取默认资源；默认资源编辑不自动热加载。

定向验证：`cargo test --locked role_card` 覆盖结构化默认/覆盖、配置保留与损坏保护、世界书匹配、真实协议分块、设置规则操作及保存/取消/延后重启。相关回归使用 `cargo test --locked backend::codex::tests::`、`cargo test --locked backend::character::tests::` 和 `cargo test --locked ui::tests::`。测试仅用临时资源与模拟服务。

### 上下文规划 场景与记忆

- `roleplay/context.rs` 定义 PromptBlock、ContextPlan、ContextSettings、SceneState 和 RoleplaySession。静态提示按核心、规则、记忆、场景、分段示例构建；核心身份、性格、关系、用户人设与已启用规则完整保留，其余内容按字符预算选取。超额核心完整保留并在检查页提示。预算统计含标题和本轮背景标记，不等同于模型 token 或完整 Codex 历史预算。
- 世界书扫描本轮输入与最近成功对话（最多 32 条，每条最多 8192 字符）；支持全局/单条深度、优先级、始终启用、持续与冷却，扫描深度 0 关闭关键词扫描。优先级高的条目先分配本轮预算；超额条目记录拒绝原因，不触发持续效果。持续 N 轮指首次触发后的 N 轮，不因重复命中续期；随后冷却 N 轮。默认 Lexington 扫描 6 条，持续 2 轮，冷却 1 轮。
- RoleplaySession 在后端通信线程中使用。接受发送时先保存快照、生成计划；成功结束后将用户输入和按 item ID 去重的最终回复加入最近历史。任务失败或中断恢复快照，被拒绝或初始化前取消的输入不推进轮次。预览克隆会话后规划，不改变历史、轮数与时效，也不调用模型。静态计划只在 thread/start 携带，本轮场景和世界书另行携带；已进入 Codex 历史的旧背景不由本程序删除。
- `roleplay/profile.rs` 的 UserProfile/Memory 将用户人设与记忆独立保存到 config/roleplay-memory.json 的 profiles 分组，按角色隔离；session_card 合并有效档案后启动会话。旧角色卡的 userPersona/memory 只在没有独立档案时兼容读取；旧式 Markdown 角色添加档案后仍保留原人格。候选默认未确认，不加载；仅 confirmed && enabled 的非空记忆进入提示。记忆的来源元数据仅供用户核对，不注入模型。文本编辑会撤销确认；保存失败保留原文件。
- `ui/roleplay_context.rs` 在“角色卡 → 上下文”显示上次实际发送与预览报告，逐块显示选中状态、类别、字符数、作用域及原因，支持查看内容；只检查本软件附加内容。临时场景支持当前活动、风格、尚待完成，应用/清空不重启模型，下轮起使用，新会话清空。预览使用当前已知状态，不预测进行中任务的后续回复。
- `ui/roleplay_memory.rs` 在“用户与记忆”管理独立草稿，支持手动新增或从最近输入创建候选、确认、撤销、启停和删除，通过独立保存按钮应用；保存操作在 viewport 回调之后执行。保存候选/来源等不影响有效提示时不重启，已确认内容变化待当前任务结束重建；关闭或取消丢弃未保存草稿。不会自动推断事实或自动生成摘要。
- 新 Event::Context 通过标准通道回传至 UI，实际与预览分开保存；ContextPlan 的 Debug 不含提示文本，已知自定义 API Key 在事件出口统一遮蔽。用户未主动保存时，最近输入、最近对话、临时场景和检查报告仅存在内存，不写用户记录。
- Linux `scripts/package.sh` 改为写入空配置，并清理旧输出遗留的 roleplay.json、roleplay-memory.json；`scripts/test-package.sh` 用临时工程和假构建器验证重复打包不会携带私人配置，角色默认资源仍完整。此夹具可在 Git Bash 运行，不代替 Linux 桌面实测。

参考酒馆的 [Prompt Manager](https://docs.sillytavern.app/usage/prompts/prompt-manager/)、[世界书](https://docs.sillytavern.app/usage/core-concepts/worldinfo/) 与[摘要可修正设计](https://docs.sillytavern.app/extensions/summarize/)。本实现保留 Codex 管理完整历史，不假设能够照搬任意历史插入位置。

定向验证：`cargo test --locked roleplay::`、`cargo test --locked backend::codex::tests::`、`cargo test --locked ui::tests::`、`cargo test --locked ui::role_card::tests::`，以及 `bash scripts/test-package.sh`。场景包括预算及溢出原因、最近助理消息触发、持续/冷却、预览无副作用、失败/中断回滚、记忆确认与撤销、旧式迁移、规则与设置交互。均使用临时配置和模拟服务，不连接真实模型。

## 启动与依赖方向

- `main.rs`：先处理 `--ask-files` / `--ask-json` / `--native-messaging` 无 GUI 入口，再取得 Linux/Windows 普通/开发模式独立实例锁并创建 eframe 原生窗口、选择 X11/Wayland/Win32 后端、加载窗口图标并构造 `ui::Pet`。普通尺寸 500×500，`--dev` 为 800×600。Linux X11/XWayland 使用 Dock 桌面浮层类型，不设置 strut、不占用工作区；避免 GNOME 将透明窗体整块限制在屏幕内，使立绘中心可到屏幕边缘。
- `ui` 可调用 `backend`、`roleplay` 和 `tools`；`backend` 可消费 `roleplay` 的提示和世界书结果，不依赖 `ui`、egui 或 GPU 纹理；`roleplay` 不依赖 `backend`、UI 或 egui；`tools` 不依赖应用 UI 或业务模块。
- `tools/platform.rs` 是平台与窗口框架的适配边界，可依赖 eframe/egui/winit；路径和解码工具不依赖这些框架。
- UI 内通过 `impl Pet` 分文件组织相关交互，必要的共享字段与方法限制在 UI 模块内。不要为了目录分层增加只有转发作用的抽象。

## UI

Windows 同样启用 UI 的 Inbox、原生位置保存/恢复、显示器检测、指针轮询、拖动及输入区域；独立设置窗口初始位置与闹钟中央提醒使用 Win32 显示器矩形，设置按原生按下锚点绝对移动。主窗口启动使用 active=false，不主动抢焦点。浏览器宿主可通过 `lexington-native-host.exe` 别名直接进入协议入口，不能把该别名当 GUI 再次启动。

| 文件 | 职责与主要接口 | 依赖及约束 |
|---|---|---|
| `ui/mod.rs` | 声明 UI 子模块（含天气），对入口导出 `Pet` | 具体组件保持在 UI 模块内部 |
| `ui/app.rs` | 定义 `Pet` 与 `eframe::App`；初始化、刷新资源与纹理、每帧编排、自动关闭/外部点击关闭对话、右键菜单、关于、截图、窗口状态和侧栏模式持久化。主要接口：`Pet::new`、`refresh`、`reload_config`、`load_picture`、`ui`；启动加载独立打卡/闹钟记录、活动监测、Codex Client 与外部 Inbox，逐帧调用独立设置及提醒 viewport，并在空闲时消费外部请求 | 持有 CharacterState、ClockState、AlarmUi、WeatherUi、Monitor、Client、Inbox 和 UI 状态；每帧 poll_weather 仅收取结果或提交显式排队的查询，空闲不请求网络；设置拖动单独保存原生按下锚点/目标窗口、鼠标状态和是否获准拖动，关闭后清空；测试构造不启动真实 CLI/监测或访问用户记录。每帧收集可见立绘/控件/菜单及 Embedded 窗口的 input_regions，不包含独立 viewport、全屏 pet Area 或历史缓存。右键菜单在绘制闭包中收集当前内容加 Frame 边距，仅菜单打开时加入；egui 0.31 不能依赖 context_menu 返回值。X11 仅鼠标按下聚焦，启动/悬停不抢焦点。独立保存普通/开发模式逻辑位置（windowPosition/devWindowPosition），仅恢复位置，尺寸固定 500×500/800×600，禁用 eframe 尺寸及 egui 缓存持久化；原生位置按 DPI 换算，可见立绘需与任一活动显示器相交至少 24×24，透明边距可出屏，无效位置回默认主屏。截图释放 input 锁后发 viewport 命令。配置每 500ms 检查时间/长度，成功才替换，失败保留旧值；普通设置保持会话，程序/工作目录/模型/自定义 API 参数改变待空闲重建；connection_draft 独立保存未提交表单，关闭设置时丢弃草稿；new 解析 CODEX_HOME / HOME/.codex，from_root 不读取用户 Codex 配置，测试须显式注入临时路径 |
| `ui/pet.rs` | `pet_ui` 绘制立绘/默认宠物和阴影，设置宠物、对话、输入层的共同变换，处理圆形命中、点击与拖动；默认宠物几何和变换测试也在此 | 使用 `Picture` 和平台拖动；将当前帧的归一化 alpha 覆盖区域映射到立绘实际位置并应用共同变换，默认宠物按身体轮廓生成输入区域，阴影不拦截点击；返回尺寸、变换和点击结果供 app 编排。容器 450×450，侧栏不参与共同缩放；拖动阈值 5 个逻辑像素；按下时由平台层读取原生屏幕指针/窗口坐标捕获锚点，不使用 egui 可能过时的窗口局部指针 |
| `ui/settings.rs` | `settings_ui` 创建独立 464×390 原生 viewport；`settings_contents` 复用两栏设置内容，包含常规/连接/打卡/闹钟/天气、模型/API/密码 Key 表单、本机配置自动填充/手动重读与保存/取消修改、开关保存、资源刷新和会话重启 | 无边框、不透明、Dock/置顶；初次打开按桌宠所在显示器适配位置，仅初始化时 with_position，后续自由拖动。标题栏及正文空白均可拖动，后台拖动控件先于正文注册；按下命中其他可点击/拖动控件时禁止本次窗体拖动，正文关闭拖拽滚动，保留滚轮和滚动条。X11/Windows 新按下时用 screen_pointer/window_at_pointer 核对进程与设置窗口标题、捕获原生屏幕位置，经 move_window 发送原生绝对移动；保留候选锚点等待可能晚一帧到达的 egui 正文命中结果，松开/关闭清除。不能依赖缓存 outer_rect 或跨阈值后的 StartDrag；Wayland/其他平台走 StartDrag。映射后 800ms 激活，关闭仅销毁子 viewport；不把独立设置矩形加入桌宠 XShape。Embedded 后端回退 Foreground Window 并记录输入区域；14px 文本局部应用，路径截断/悬停/复制，内容滚动。常规/打卡/连接/天气操作在 viewport 回调结束后执行；连接表单点击保存才验证并原子写入，成功调用 reload_config 触发空闲重连；Key 默认 password TextEdit 遮蔽，按住显示按钮时暂时明文（含继承配置），松开/失焦隐藏，不保存显示状态，错误独立保留；连接页首次打开读取 LocalCodexConfig，已启用的自定义 API 保留；手动重读仅更新草稿并保留自定义 API 开关，关闭再开重新读取；取消恢复已保存值及本机默认值，保存继承配置不固定未修改的本机模型、不复制本机 Key；写入成功才改内存，未知配置/旧密钥保留；重启会话忙碌时禁用 |
| `ui/sidebar.rs` | `sidebar` 绘制六个按钮、悬停提示与动画，报告本帧是否处理点击；↻ 调用 `toggle_damage` 切换同服装正常/战损，▶ 调用 `next_illustration` 在当前状态队列循环；`clock` 将持久化后的手动打卡结果转为对话，仅成功开始计时后排队查询天气，取消不查询，失败可从打卡设置查看 | 用 `damage_counterpart` 判断切换按钮是否可用，缺少同角色/格式/服装配对时禁用；战损时 ↻ 为橙色，悬停提示显示切换目标和下一张所属队列。选择变更后通过 app 重载纹理；按钮 40×40、间隔 10，仅按钮自身（随悬停缩放）加入输入区域、间隙穿透；侧栏点击须阻止同帧外部点击关闭 |
| `ui/dialog.rs` | `say` 初始化消息与打字机状态，`click_pet` 处理随机台词/显示全文/关闭，`bubble` 测量和绘制对话及滚动区 | 台词来自 `CharacterState`；使用共享样式与背景纹理。每 50ms 一个字符，按全文保留高度，14px 字号、21px 行高、10/14px 内边距；自动关闭与外部点击由 app 协调；只在对话显示时提交经过层变换的气泡输入区域 |
| `ui/codex.rs` | `start_codex` 创建或替换子进程，`poll_codex` 每帧汇总事件并展示回复，`codex_prompt_ui` 显示审批和问题 | 调用后端 Client；按 item ID 汇总增量，最终文本替换同 ID，避免重复，不二次打字；任务结束清理审批队列；可见审批/问题窗口向 app 提交输入区域，保证原生鼠标穿透不影响审批交互。命令/文件审批允许本次或拒绝，文件展示路径和 diff，问题可选项或自由回答，秘密答案遮蔽。重启先清理旧进程并清除待重建标记，结束与失败均解除忙碌状态；external_turn 标记右键请求，完成不清空草稿，异步失败也通过气泡显示 |
| `ui/alarm.rs` | `AlarmUi` 管理表单和错误状态；`load_alarms` 加载持久化记录；`alarm_settings_ui` 提供 HH:MM、提示语、每天重复、新增/编辑/删除及启停；`alarm_popup_ui` 每秒检查到期并显示独立原生窗口 | 依赖 AlarmState、clock 本地时间和 paths；弹窗 420×260、无边框不透明置顶，按桌宠所在显示器中央定位（X11 用 RandR，支持负坐标和 DPI），不移动桌宠。X11 使用 Dock 层，普通 AlwaysOnTop 可能被桌宠 Dock 遮挡；映射后 800ms 一次性 Focus/AlwaysOnTop（晚于主窗启动置顶），不复用桌宠 XShape 遮罩。多个到期提醒合并，长文本滚动，固定确认按钮；确认或系统关闭先持久化清除 pending，保存失败保留弹窗。非原生 egui 集成降级为居中 Window 并加入输入区域；Wayland 位置由合成器决定 |
| `ui/clock.rs` | `poll_clock` 消费系统采样、驱动自动确认及提醒；`clock_settings_ui` 展示自动开关、检测状态、起点和编辑；`clock_action` 保存/撤销/手动操作；`clock_notice_ui` 提供修改和撤销入口 | 调用后端 ClockState 和工具 Monitor；自动打卡成功后排队天气查询（忙碌时也排队但不覆盖聊天）；自动提示加入原生输入区域，关闭立即移除；过期采样不确认，自动开关/确认时长热加载会重置候选；加载失败禁用更改并显示错误，避免覆盖损坏记录 |
| `ui/input.rs` | `submit` 仅发送输入原文（裁首尾空白，不拼接路径或来源）发起输入框聊天，`poll_handoff` 从 Inbox 顺序直接发送外部上下文，二者通过 `send_message` 复用发送状态；`input_ui` 管理输入框位置、焦点、悬停、加载动画（文案固定为“Lexington正在思考”）、错误展示、Enter 与 Escape | 提交给启动时创建的 Codex Client，每次提交附带当前 `noApproval` 值；外部请求不填输入框、不改焦点、保留草稿，不中断忙碌任务，立即提交失败通过气泡提示；不重复提交；Escape 中断正在进行的任务，空闲时清空输入/错误。后台回调只请求重绘，结果由 `ui/codex.rs` 接收。连接/任务错误必须显示在编辑框下方的限高滚动区，不能替换 TextEdit、吞掉后续输入或清除草稿；单行显式使用可用宽度，长文本滚动保持尾部/光标可见。输入/错误面板按共同变换后的完整尺寸限制在 viewport 内，避免缩放或错误展开后出屏。Enter 同时检查 `has_focus`/`lost_focus`；启动不抢焦点，占位色固定 #999；鼠标进入共同变换后的 450×450 立绘容器或输入区域时显示，输入焦点或生成中保持显示；离开悬停且失焦后淡出并移除输入区域，草稿和错误仅保留内容，不强制显示；原生失焦显式清除 TextEdit 焦点 ID；隐藏时不占据原生输入区域，alpha 大于 0.01 才加入。X11/Windows 下通过平台读取窗口局部指针发现悬停，失败回退 egui 指针，恢复输入框显示，不依赖透明处收不到的鼠标移动事件，也不抢焦点 |
| `ui/assets.rs` | `Picture::load/size/texture` 将解码像素上传到 GPU，复用纹理并更新动画帧；`opaque_regions` 保存当前合成帧的归一化 alpha 覆盖条带（最多 256×256 网格，按格内最大 alpha ≥ 8 判断，逐格扫描命中即停止，使用 ceil 反推像素边界以等价保留非整除尺寸细节，不额外常驻 RGBA 帧）；`icons` 加载内置侧栏图标；保留 GIF 抽帧后的像素、循环、跳过帧的局部处置、输入遮罩随显示帧透明度更新、非整除尺寸 alpha 覆盖等价性及纹理复用回归测试 | 调用 `tools::gif::Decoder`，读取当前 RgbaImage 并保持只读；颜色转换和 alpha 区域计算仍在 UI 层，不实现循环策略；立绘/背景使用线性 mipmap。字体和图标的 `include_bytes!` 路径相对 `src/ui/` |
| `ui/style.rs` | `install` 注册字体和浅色主题；`font`、`bubble_tail`、`draw_bubble`、`draw_shadow` 提供共同绘制规则 | 使用 egui 与 `Picture`；CJK TTC 使用简体中文 face index 2；对话尾尖距左边 28px，输入尾尖居中。气泡 Area 的溢出策略由对话/输入组件保持 `constrain(false)` |
| `ui/weather.rs` | `WeatherUi` 保存城市搜索、请求 ID 和待展示天气；`weather_settings_ui/weather_action` 提供搜索、选择保存、停用和主动查看；`queue_clock_weather/poll_weather` 连接手动/自动打卡与对话输出 | 只在打卡成功或用户主动搜索/查看时联网，平时不轮询、不预取；未配置城市维持原打卡消息。工作线程回调只请求重绘；结果按请求 ID、城市、打卡起点及本地日期关联，取消/换城/跨日丢弃，生成中或已有其他消息时延后，错误不回滚打卡；主动查看不要求打卡。显示全文并复用气泡滚动区；测试用夹具验证设置点击、自动/手动触发、闲置不联网、消息避让和失效回复 |
| `ui/tests.rs` | 使用真实 egui 指针和键盘事件验证组件协作：侧栏、正常/战损对应切换、下一张的状态隔离、跨格式保留服装、角色与格式、对话、打卡、输入、外部请求在任务中排队/同一会话多轮/来源路径和行号保留/完成不覆盖草稿、普通/开发窗口悬停立绘唤出输入、点击聚焦输入、原生失焦后草稿/错误隐藏且内容保留、再悬停恢复及生成中显示、Codex 流式回复/失败、配置热加载/错误保留/忙碌延迟重建、设置开关点击/持久化/同会话连续切换、API 表单本机夹具自动填充/读取不落盘/继承保存不复制凭据和模型/滚动保存/密码遮蔽及按住显示与松开或失焦隐藏/非法地址拒绝/成功保存及忙碌延迟重连、空白背景/隐藏输入框不命中与关闭窗口输入区域立即清理、右键菜单关闭项在立绘外持续接收原生输入并触发退出、自动打卡提示撤销、时间编辑、自动开关持久化和热加载、闹钟表单校验、编辑/保存、启停/删除、弹窗外点击不消失和确认持久化、位置保存/恢复/开发模式隔离/无效或越界位置回退及中文排版；真实立绘下的连续中文输入、输入三个字后异步错误仍可继续、长文本水平滚动与 IME 提交；侧栏资源用例统一验证图片加载、格式/正常战损切换，并在每次切换后调用 `refresh` 验证路径、状态和图片加载保留；资源数量与配对完整性统一由 backend/character.rs 检查 | 使用模拟 Codex 子进程和临时配置，不访问真实模型或凭据；GIF 和几何专项测试靠近各自 UI 实现 |

## 后端功能

| 文件 | 职责与主要接口 | 依赖及约束 |
|---|---|---|
| `backend/mod.rs` | 声明后端子模块（含天气、外部上下文 handoff 和本机 Codex 配置读取），集中保留配置、审批/自动打卡开关无损保存、确认时长边界及错误保护、扫描的测试及共享临时目录夹具 | 协议测试位于 codex.rs，资源扫描和配置测试不需要外部服务 |
| `backend/handoff.rs` | `Request::validate/prompt` 定义选区/路径/来源/行号与分析提示；Linux `Endpoint::bind/start` / `Inbox::poll` 提供用户级单实例、本地入队通道；`deliver` 自动启动桌宠并交付一次；`native_host` 实现浏览器本机协议 | 无 UI 依赖。Unix socket 位于 XDG_RUNTIME_DIR 下 lexington-UID（开发模式 -dev），缺失时用临时目录；目录须 0700/当前用户且非软链接，flock 独占锁下清理旧 socket，0600 socket 加 SO_PEERCRED 校验当前用户。长度帧 ≤1 MiB、队列 16 条、最多 32 个绝对路径，读取/写入超时，后台线程只唤醒 UI；ACK 表示入队，发出后不自动重试避免重复；无 HTTP/TCP/剪贴板监测。普通/开发各一个实例；Windows 由 handoff_windows.rs 提供同协议的私有命名管道；其他平台外部交付明确不支持。含排队/满队列/残留恢复/不安全路径/分帧及校验测试 |
| `backend/config.rs` | 定义 `Config`、`DialogConfig`、`CodexConfig`、`ClockConfig`、`WeatherConfig`（CodexConfig 的 Debug 隐藏 Key，validate_connection 校验模型/基础地址/Key，connection_changed 排除审批开关）；`Config::load` 启动读取失败时默认回退，`read` 返回读取/解析错误供热加载保留旧值；补默认值并限制尺寸和计时范围；`Config::save_no_approval` / `save_auto_clock` / `save_connection` / `save_weather` 通过共享无损原子保存入口修改相应字段，连接字段 model/useCustomApi/baseUrl/apiKey 同次原子写入 | 标准库文件读取、serde 与 url（校验 API 地址）；保持 `config/config.json` 的 camelCase 兼容及错误时默认回退，未知旧 `llm` 字段忽略，不记录密钥；Codex 程序默认 codex，工作目录默认资源根目录；`noApproval` 默认 false；自动打卡默认开启，confirmationSeconds 默认 180，限制 30～600 秒。保存开关以 JSON 对象保留未知字段和旧密钥，损坏配置拒绝覆盖；同目录临时文件写入并同步后原子替换，Unix 新文件权限 0600；weather.city 默认 null，选定城市以 name/latitude/longitude/timezone 原子保存；读写校验经纬度与名称/时区，保留未知字段，不影响 Codex 连接；模型/地址/Key 裁掉首尾空白，基础地址去尾斜线；自定义模式必须提供模型、无内嵌账号/查询/片段的 HTTP(S) URL 和有效 Key，无效热加载保留旧值；API Key 为用户配置文件明文存储，不能记录到日志/Debug |
| `backend/resources.rs` | 定义 `Illustration`（含服装名 `outfit` 和战损标记 `damaged`）；`illustrations` 扫描排序立绘并解析配对元数据，`background` 选择首张支持的背景，`texts` 读取非空角色台词 | 标准库文件读取和 serde_json；只解析文件名最后的 `_Normal` / `_Damage` 后缀，ASCII 大小写不敏感；无后缀按正常处理，其完整文件名主体作为服装名。保留 `resource/` 布局，按目录识别角色与格式，拒绝台词路径穿越，不解码图片 |
| `backend/character.rs` | `CharacterState` 持有资源列表、角色、格式、正常/战损状态、选中索引与台词；`filtered` 返回当前角色/格式/状态的队列，`damage_counterpart` 查同服装的相反状态，`toggle_damage` 双向切换；`refresh` 保留当前路径并恢复失效选择，`next_illustration` 循环当前队列，`toggle_format`/`next_character` 切换格式/角色，`selected_path` 提供路径 | 仅依赖资源扫描模块和标准库；默认 GIF + 正常。配对严格限制同角色、同格式、服装名 ASCII 大小写不敏感，缺少对应版本时不切换；格式切换尽量保留同服装。选取回退顺序为当前格式/当前状态 → 另一格式/当前状态 → 当前格式/另一状态 → 另一格式/另一状态。含现有 15 对资源双向匹配、独立队列循环、后缀大小写、缺失配对及删除恢复测试；只选择路径，纹理和默认宠物由 UI 处理 |
| `backend/codex_config.rs` | `LocalCodexConfig::read` 读取本机用户级 TOML 的模型、所选 profile 和提供方地址/凭据；`draft` 填充展示值，`to_save` 保持继承语义 | 依赖 toml、serde_json 和 CodexConfig；只读 config.toml/auth.json，不解析项目/管理员配置层。Key 读取 provider env_key 环境变量、experimental_bearer_token，OpenAI 登录再读 OPENAI_API_KEY 环境/登录文件；忽略 OAuth tokens。缺文件允许，错误不带 TOML 源码或凭据，不派生 Debug；单测注入环境和临时目录，覆盖来源、profile、继承/显式优先及解析错误不泄密 |
| `backend/codex.rs` | `Client::start/submit(text, no_approval)/interrupt/reply/poll` 管理 app-server 子进程、stdio JSON 消息与事件通道；`Event`/`Prompt` 表达回复、审批、问题及连接状态 | 依赖配置、serde_json 和标准库，不依赖 UI 或 HTTP 客户端。自定义模型/提供方通过 -c 覆盖（JSON 字符串编码兼容 TOML），provider=lexington_api、wire_api=responses、requires_openai_auth=false；Key 仅传 LEXINGTON_MODEL_API_KEY 子进程环境，关闭模式显式移除变量，不修改用户 Codex 配置；初始化/流式事件中的完整 Key 统一遮蔽。启动 initialize → initialized → thread/start，输入可等待初始化，忙碌提示使用 Lexington 名称；同一角色会话复用一个 thread，切换角色新建 thread；保留 thread/start 返回的原始 approvalPolicy，逐轮明确传入 never 或原始策略，兼容字符串和 granular 对象，防止 turn/start 的持久覆盖导致关闭开关后仍无需审批；不覆盖沙箱。后台线程隔离阻塞读取，唤醒回调仅通知 UI。ACK 超时 30 秒，生成不限时；按 thread/turn 及请求 ID 关联，忽略过期 ACK；文本增量与最终 item 分开处理。缓存 item/started 中命令与文件 diff 用于审批，不支持的服务器请求返回 -32601。退出、重启或连接失败须终止并回收子进程；Unix 通过独立进程组与 libc 清理启动器及后代，先关 stdin 再回收并 join 读线程，防止后代持有 stdout 导致退出卡住；Windows 隐藏且挂起启动原生 exe，加入 kill-on-close Job Object 后恢复主线程，退出清理整棵进程树；默认补查 PATH、用户 CLI 目录及 npm 原生 vendor 的 codex/、bin/ 布局，不执行 .cmd/Node；command 构建独立以验证参数和环境隔离。不记录协议原文/凭据 |
| `backend/weather.rs` | `City::validate` 校验选定地点；`Client::start/send/poll` 通过有界通道在单个工作线程中搜索城市和查询三天预报；解析 WMO 天气代码并生成中文文本 | ureq + Rustls HTTPS 直连 Open-Meteo 地理编码/预报接口，不经过 Codex，不依赖 egui；请求总超时 10 秒、禁重定向、响应限制 1 MiB、待处理队列 4。按城市及请求日缓存 30 分钟，更新失败可显示同城同日 6 小时内旧数据并注明；缓存只在内存，含更新时间/时区和来源。退出丢弃队列并回收线程，最多等待当前有超时请求。测试用本地 HTTP 夹具覆盖编码、缓存/换城、超时/非法/超大数据及无损配置保存 |
| `backend/alarm.rs` | `AlarmState::load/save/toggle/delete/poll/dismiss` 管理多个本地时间闹钟、调度与待确认提醒 | 不依赖 UI；时间严格 HH:MM，提示语 1～500 字符；一次性闹钟响后关闭，每天重复按 libc mktime 计算下一本地日期，不能固定加 86400 秒，跳过 DST 不存在时间。设置时间已过时安排下一次；启用重新计算；到期（含休眠/停机错过）每个闹钟只补一次并推进到未来，pending 跨重启保存；同时到期合并，重启不重复。普通/开发记录隔离，0600 临时文件同步后原子替换，保存失败不提交内存，损坏数据拒绝覆盖。含同时触发、重复/错过、重启、校验、编辑启停删除与存储失败测试 |
| `backend/clock.rs` | `ClockState::load/observe/toggle/edit/undo/poll` 管理本地日期的自动确认、手动打卡、时间修正、撤销和一次性下班提醒；`now/local_time` 处理时间戳与本地时间 | 依赖配置、serde、标准库及 tools/time 跨平台 libc 本地时间转换，不依赖 UI。自动开始须持续活跃默认 180 秒，回溯首次操作；空闲满 60 秒、锁屏/不可用样本、采样间隔超过 10 秒、时钟跳变均重置确认，确认终点须有新操作。每天自动一次，手动取消、撤销或提醒后当天不再自动开始；跨本地日期重置。记录立即原子保存，失败不提交内存状态；起点、已处理和已提醒状态跨启动恢复，开发模式隔离；编辑拒绝未来或不存在的本地时间。含模拟时钟、断档、持久化、撤销、编辑和存储错误测试 |

## 工具代码

### Windows 后端与平台接口

显式 .cmd/.bat 配置在启动前拒绝（不区分扩展名大小写），防止 Windows Command 的批处理特殊路径隐式引入 shell/Node；启动参数测试使用含空格与中文的真实工作目录。安装夹具同样覆盖空格/中文源及目标，并从 ZIP 解压后验证无需源码、Rust 或 Python 的基本安装。

独立设置窗口在 Windows 与 X11 共用按下锚点逻辑；Windows screen_pointer/window_at_pointer/move_window 仅定位当前进程同标题的原生顶层窗口，使用指针宽度 WindowId 保存 HWND，并以 SetWindowPos 绝对移动，避免 StartDrag 在跨阈值后丢失最初几像素。

Windows 桌宠映射后清除 APPWINDOW 并设置 TOOLWINDOW，以原生窗口样式补强 eframe 的任务栏隐藏要求；独立设置窗口不复用此样式。

穿透样式每帧与实际 GWL_EXSTYLE 比较后按需更新，不缓存期望布尔值：winit 在焦点/置顶变化时可能重写原生样式，缓存会使无事件透明区域永久失去穿透或控件永久不可点击。

原生 smoke 的穿透断言失败时仅报告窗口几何、样式和缩放，不读取真实配置或协议内容。

windows-smoke 的 --artwork 可复制指定 GIF/PNG 到临时资源根，验证纹理 alpha 路径；默认验证缺图回退。启动焦点先以真实点击激活独立底层窗口再比较，避免操作系统拒绝无输入激活导致误报；设置拖动检查两轴，关闭按钮使用真实点击。

指定资源时用 --pet-point X Y 选择 500×500 逻辑窗口中持续可见且处于拖动圆内的实体点，不能假定 GIF 画布中心有 alpha；边缘拖动将实体点移至四角附近，验证原始偏移与负窗口坐标并保持该点可再次抓取。

Windows 非阻塞字节管道使用 PeekNamedPipe 区分暂时无数据与断开，ACK 后等待客户端关闭再断开服务端，避免丢弃未读确认；独占绑定的 ACCESS_DENIED/PIPE_BUSY 均表示已有实例。Python 协议夹具显式使用 UTF-8，不依赖 Windows 系统代码页。

协议夹具以 samefile 校验工作目录身份，兼容 Windows canonicalize 的扩展路径前缀与 Python getcwd 的普通路径表示，不以字符串相等判断同一目录。

- `backend/handoff_windows.rs`：实现与 Linux 相同的 `Endpoint::bind/start`、`Inbox::poll` 和 `deliver`；管道名包含当前用户 SID 和 normal/dev，DACL 仅当前用户和 SYSTEM，拒绝远程客户端，以 first-pipe-instance 保证独占。非阻塞轮询、有界读写截止时间、16 条 FIFO；ACK 仅入队，发帧后不重发。宿主自动启动同目录 `lexington.exe`，避免别名递归。测试覆盖独占、排队、退出释放和 UI 忙碌/草稿保持。
- `tools/windows.rs`：无 eframe 依赖；`Handle` 负责 Win32 句柄生命周期，`user_sid/wide` 处理身份与 UTF-16，`Job::attach/terminate` 将挂起的子进程加入 kill-on-close 作业后恢复线程。`idle_duration` 仅活动本地控制台且 WTS 未锁定时返回 GetLastInputInfo 的空闲时长，失败保留手动入口；不采集输入内容。
- `tools/proxy.rs`：Windows 专用，无 UI 依赖。`backend/codex.rs::command` 在继承登录及自定义 API 两种模式调用 `inherit_system_proxy`；经 WinHTTP 只读当前用户的系统手动代理，释放三个系统分配的字符串，通过 Command.env 向新建 Codex 子进程补 HTTP_PROXY/HTTPS_PROXY，不修改系统代理、用户 Codex 配置或全局环境。任一代理环境变量（含小写、空值）已存在时完全保留；NO_PROXY 已存在时也不覆盖。支持统一 host:port 或 http=/https= 分项，HTTPS 目标默认仍用 HTTP CONNECT；拒绝含账号密码、查询、片段和路径的代理。无手动代理/仅 PAC 或自动发现时不注入；不执行 PAC。排除项支持字面主机/IP、域名后缀和 IPv4 前缀子网，默认排除本机回环；其他 Windows 通配符和 `<local>` 不能完整映射到 NO_PROXY，不转换。代理变更后须重启会话。纯夹具测试覆盖协议选择、环境优先、无效值拒绝及排除项，不连接模型。
- `tools/time.rs`：`local` 调用 Unix localtime_r / Windows localtime_s，`timestamp` 调用 Unix mktime / Windows CRT _mktime64（libc 未导出 Windows mktime）；打卡与每日闹钟共享本地日历/DST 语义，测试时间戳往返和极值拒绝。
- `tools/platform_windows.rs`：平台边界依赖 eframe/winit/windows-sys；`DragWindow` 读取 HWND、全局指针和客户区原点，以按下锚点和 DPI 阈值绝对移动；置顶不激活，点击才聚焦。EnumDisplayMonitors 主屏优先且支持负坐标；指针轮询按当前 alpha/控件矩形切换 WS_EX_TRANSPARENT/WS_EX_LAYERED，不裁剪绘制区域；实际跨进程穿透及拖动必须以 Windows 原生 smoke 验证，模拟事件不能替代。

| 文件 | 职责与主要接口 | 依赖及约束 |
|---|---|---|
| `tools/mod.rs` | 声明路径、活动监测、解码、时间、平台及 Windows 代理模块 | 不承载业务状态 |
| `tools/paths.rs` | `resource_root` 定位资源根目录；`clock_state_file` / `alarm_state_file` 返回普通/开发模式独立的用户数据记录路径 | 顺序：`LEXINGTON_HOME` → 可执行文件旁存在 `resource/` 的目录 → 当前目录；闹钟（alarms.json / alarms-dev.json）及打卡状态使用 XDG_DATA_HOME，默认 ~/.local/share/lexington；Windows data_root 使用 LOCALAPPDATA/lexington，回退 USERPROFILE/AppData/Local，不能写入资源目录 |
| `tools/activity.rs` | `Monitor::start/poll` 在后台每 2 秒采样系统空闲时间和会话可用性，返回带单调时间戳的 Sample；退出停止并回收线程 | Linux 用 loginctl 排除锁屏、远程和非活动会话，优先 gdbus 调用 Mutter IdleMonitor，原生 X11 可回退 XScreenSaver；Wayland 禁止回退 XWayland 空闲时钟。子进程有超时并清理，UI 不阻塞；不记录键鼠内容；不可用样本重置自动确认，保留手动入口；测试可注入采样 |
| `tools/gif.rs` | `Decoder::load/advance` 返回当前 RgbaImage；支持 GIF/静态图片，控制隔帧播放、时序、循环次数、索引缓存与超限流式回退 | 不依赖 UI，不创建 GPU 纹理，不缓存合成帧。源帧 0、2、4…输出，各跳过帧先独立归一化延迟再合并到前一显示帧，奇数结尾保留，不跨循环合并，总时长不变。首轮缓存索引帧/调色板，96 MiB 预算仅针对单个当前 GIF，超限释放前缀并保持流式；完整缓存回放不重读源文件，循环时清空处置画布。保留透明索引、局部/全局调色板、交错帧及 Keep/Background/Previous：跳过 Keep 直接更新处置画布，跳过 Background 仅清区域，Previous 保留原画布，无需合成/上传被跳过帧的 RGBA。Finite(0) 视为无限，显式有限次数保留，原帧低于 20ms 按 100ms。回归覆盖无源文件回放、缓存超限、奇数/变延迟、跳过帧依赖、有限循环和 UI 纹理/透明遮罩；全量参考像素与合并延迟对比用 cargo test --release --locked bundled_frames_match_reference_decoder_pixel_for_pixel -- --ignored；frame_sampling_performance 用 --release --ignored --nocapture --test-threads=1 比较同等循环的原帧率/抽帧取帧，不含 UI/纹理开销 |
| `tools/platform.rs` | `configure` 设置平台窗口属性；Linux `DragWindow::new/arm/update/configure/focus` 应用 X11 窗口提示和绝对坐标拖动；`window_at_pointer` 从原生指针下窗口层级查找当前进程/指定标题并返回真实位置；`move_window` 对指定子窗口发送经确认的 _NET_MOVERESIZE_WINDOW，无窗口管理器时直接 ConfigureWindow；`position` 获取原生窗口屏幕坐标；`monitors` 返回 RandR 活动显示器矩形（主屏优先、含负坐标，失败回退根窗口尺寸），统一换算逻辑坐标；`screen_pointer` 获取屏幕绝对指针和左键状态供独立设置拖动；`pointer_position` 获取原生窗口局部指针，`set_input_regions` 应用 XShape Input 区域，含 DPI/拖动锚点回归测试 | 使用 eframe，Linux 另依赖 winit/x11rb（shape、randr、screensaver 特性）；XShape ≥ 1.1 时仅更新 Input、不能裁剪 Bounding/绘制区域；逻辑坐标按 DPI 换算后裁到 viewport，缓存相同输入区域以避免重复请求，使用 checked 请求。原生 Wayland 不应用该 X11 遮罩；Windows 导出 platform_windows.rs 实现，其他平台保留无操作配置入口。X11 请求关闭临时连接前执行 `.check()`；按下时同时查询原生屏幕指针与窗口位置捕获锚点，避免窗口移动后 egui 缓存的局部坐标滞后导致贴边重新拖动跳跃；阈值换算 DPI，不累加局部位移；独立设置使用根窗口指针查询实现 Dock 标题栏拖动，不依赖 WM 的 StartDrag |

## 外部右键客户端与安装脚本

- `scripts/install-windows.ps1`：源码或解压包用户级安装，参数 InstallDir/SkipBuild/NoShortcuts/NoIntegrations/NoRestart/Python。只安全关闭目标路径的旧进程，拒绝强杀；同目录临时 exe 经 File.Replace 原子替换并核对 SHA256，PowerShell 5.1 以 NullString 传递无备份参数；保留既有 config/config.json，更新资源与原生宿主别名。HKCU 注册 Explorer 经典菜单及浏览器宿主，Chromium key 计算 ID 与白名单保持一致，WScript 创建快捷方式；不修改 Codex 登录，不自动安装 CLI。
- `scripts/package-windows.ps1`：构建 locked release（SkipBuild 供夹具使用），Python 标准库生成集成产物，ZIP 包携带二进制、资源、安装器及空配置 `{}`；绝不打包源目录中的用户凭据。
- `scripts/test-windows.ps1`：临时目录夹具验证含空格路径的初次/重复安装、资源更新、配置逐字保留、二进制/别名校验、临时文件清理、危险目标拒绝以及 ZIP 无凭据/VSIX 完整性；不写真实注册表或桌面。
- `scripts/windows-smoke.py`：Python 标准库 ctypes 创建独立进程的 Win32 底层窗口，真实指针验证跨进程穿透/立绘及侧栏点击、启动不抢焦点、置顶/任务栏样式、桌宠/设置拖动、四边出屏及重新拖回、关闭重启的位置恢复、release JSON/原生宿主别名帧。临时 LEXINGTON_HOME/LOCALAPPDATA/APPDATA/CODEX_HOME 和不可用 exe 阻止读取真实记录/连接模型；已有桌宠时拒绝运行，串行执行；finally 释放按钮/恢复指针并仅回收所创建的进程。原生指针会短暂操作当前桌面，运行需明确授权；多显示器、锁屏/远程会话及浏览器实际安装仍需单独验证。
- `scripts/install-linux.sh`：非 Linux 主机提前失败并指向 Windows 安装器，避免在 MSYS/Windows 修改 Linux 桌面目录。

| 文件 | 职责与主要接口 | 依赖及约束 |
|---|---|---|
| `integrations/vscode/package.json` / `extension.js` | 注册编辑器选区和文件资源管理器的“看看Lexington怎么说”；通过无 shell 子进程 stdin JSON 调用 `--ask-json` | 仅 VS Code 扩展宿主依赖 JS；本地 UI 扩展，不增加主程序 Node 运行链。选区带本地路径/起始行；未保存标记、远程 URI 不冒充本机路径；失败显示错误；可配置 executable；Windows 默认 LOCALAPPDATA/lexington/lexington.exe，spawn 使用 windowsHide 且不经过 shell |
| `integrations/browser/background.js` / 两份 manifest | Firefox/Chromium 选区右键菜单，通过 `com.lexington.context` Native Messaging 发送选区及 page/frame URL；file URL 转本地路径，含 Windows file:///C:/ 盘符 | 仅 contextMenus/nativeMessaging 权限，无页面监听/网络服务；超过大小不发送；失败工具栏 ! 与悬停提示；Chromium public key 固定 ID，Firefox ID 固定且正式包需签名；沙箱浏览器桥接仍需平台支持 |
| `integrations/nautilus/send.py` / `lexington_menu.py` | 脚本子菜单入口按 URI 解码本地路径，读取安装位置 JSON 并调用 `--ask-files`；可选 Nautilus 4.0 顶层菜单提供器转交同一脚本 | Python 3 标准库；顶层提供器额外依赖 nautilus-python/gi。不解析 shell、不读剪贴板；失败用 notify-send 提示；最多 32 个本地文件；缺少系统扩展支持仍保留脚本入口 |
| `scripts/integrations.py` | `build` 用标准库生成 VSIX、Firefox XPI 和 Chromium 目录；`install` 安装 Nautilus 脚本/提供器、Native Messaging 宿主与白名单，检测 code 自动安装 VSIX | 以 UTF-8 读取元数据，build 支持 Windows，install 仍仅 Linux。无 npm；Firefox 未签名只可临时加载，浏览器安装需按 integrations/README.md；遵循 XDG 数据/配置路径，不更改浏览器签名策略、不关闭文件管理器 |
| `scripts/install-linux.sh` / `package.sh` | release 构建、保留配置的安装及打包，并带上集成客户端；先调用依赖安装，源码模式构建，发布包模式直接复制二进制且跳过 Rust；安装脚本创建桌面快捷方式、注册菜单/通信桥；发布包提供 install.sh 和安装脚本 | 安装二进制先复制到同目录临时文件再原子替换，避免运行中 ETXTBSY；桌面名 Lexington；升级后按项目约定安全重启旧实例 |
| `scripts/install-dependencies.sh` / `dependencies.py` | Python 引导、Ubuntu/Debian 系统依赖检查/授权安装、源码安装的缺失 Rust 工具链安装，以及缺失 Codex 官方原生包下载安装 | 默认固定已验证 CLI 0.166.0；只在缺失时下载，保留现有 CLI/登录配置，无法运行的现有 CLI 明确报错。原生发行包来自 @openai/codex 官方 registry，SHA-512 完整性校验、安全解包、保留完整 vendor 布局，用户级安装 ~/.local/bin/codex，无 Node/npm；Rust rustup-init 用官方 SHA-256 校验。--runtime-only 跳过构建依赖，--skip-optional 跳过 Nautilus 等可选依赖；系统包调用 sudo/pkexec，可选取消保留脚本菜单，必需依赖失败阻止后续安装；不运行 Codex 登录 |
| `scripts/test-dependencies.py` | 用临时 HOME、下载/包夹具验证缺失安装、已有 CLI 保留、资源布局、完整性/路径穿越拒绝及用户配置保留 | 不访问网络、不写系统目录、不安装真实 CLI，不替代真实平台安装验证 |
| `scripts/context-smoke.py` / `test-integrations.js` | 验证真实 CLI/native framing/Nautilus URI 路径和非法输入隔离；用宿主桩验证 VS Code 选区/路径/远程/未保存及浏览器菜单/来源/失败提示 | 测试不调用模型、不修改剪贴板/用户配置；前者 Python 3，后者开发机 Node；Rust UI 测试另验证 Inbox → Codex 完整队列与草稿保护 |
| `integrations/README.md` | 各客户端安装、适用范围与 native bridge 协议说明 | 记录顶层菜单依赖、浏览器签名与沙箱限制，不宣称覆盖任意软件内部菜单 |

## 新代码放置与文档维护

根 [AGENTS.md](../AGENTS.md) 维护全项目约束；目录规则分别位于 [后端](../src/backend/AGENTS.md)、[界面](../src/ui/AGENTS.md)、[工具](../src/tools/AGENTS.md)、[脚本](../scripts/AGENTS.md)。跨目录任务读取相关规则，模块知识在对应目录维护。

- 视觉、输入、焦点、纹理及窗口界面编排放入 UI；角色卡、角色映射、世界书及提示组装放入 roleplay；配置、资源选择、计时及基本 Codex 会话业务规则放入后端；不涉及业务的路径、解码及平台适配放入工具。
- 新增功能优先扩展所属模块，相关逻辑保持内聚；不要仅因代码只用一次就抽取新的辅助函数或文件。
- 后续任何代码文件改动，须同步检查并更新本文件对应的职责、接口、依赖、行为或约束；新增、删除、移动、重命名文件同时更新目录树。只改内部实现时，也应维护受影响的行为或约束说明。工作规则由根及对应目录 `AGENTS.md` 分层维护；新增可复用知识写入最接近所属模块的 AGENTS.md，仅跨模块通用规则写入根文件，更新前阅读并合并去重，无新增知识不必添加。
- 安装、打包及原生验证脚本仍放在 `scripts/`；原资源布局和配置格式不随 Rust 模块拆分改变。

## 测试夹具

`tests/fixtures/codex-server.py` 用 Python 3 模拟 stdio app-server，供后端与 UI 测试共享；覆盖初始化、Unicode 输入、流式/最终消息去重、同一会话多轮与延迟 ACK、模型/提供方启动参数与 Key 环境/命令行隔离及完整 Key 遮蔽、无需审批/原策略逐轮恢复（含 granular）、沙箱保持、命令/文件审批、提问、未支持请求、中断、失败恢复、崩溃和持有 stdout 的启动器后代清理。只在测试中通过 `include_str!` 使用，不作为发布版运行依赖；不访问用户 Codex 登录信息或真实模型。

## 验证入口

### Windows 适配验收（2026-11-08）

本机 Windows x64/MSVC，依赖与 Rust 工具链使用清华镜像；测试使用临时配置和模拟服务，不调用真实模型或读取登录凭据。

- `cargo fmt --check`、`cargo clippy --locked --all-targets -- -D warnings` 通过。
- `cargo test --locked`：65 passed、0 failed、2 ignored；两项默认忽略的 GIF 全资源像素比对与性能专项再用 `cargo test --release --locked -- --ignored` 执行，2 passed。
- `node scripts/test-integrations.js`：VS Code/浏览器来源、路径及未保存/远程文档边界通过；Python 集成产物构建成功。
- `scripts/test-windows.ps1 -Python <python.exe>`：初装、升级、配置逐字保留、二进制/宿主哈希、安全目标拒绝、中文空格路径、安全 ZIP 及无源码/Rust/Python 基础安装通过。
- `cargo build --release --locked` 与完整资源 Windows ZIP 打包通过。用户安装版和原生宿主别名与 release SHA256 一致；桌面/开始菜单、Explorer 命令及五种浏览器本机宿主注册核验通过。
- `scripts/windows-smoke.py`：缺图回退和实际 GIF 均通过；最终安装版使用 `--artwork resource/illustration/lexington/gif/BiKiNi_Normal.gif --pet-point 200 225`，真实跨进程穿透、焦点/置顶/任务栏、双轴桌宠/设置拖动、四边重抓、关闭重启恢复和 JSON/宿主交付通过。

未覆盖范围：多显示器混合 DPI、锁屏/RDP、真实浏览器扩展选区端到端及真实模型请求；Linux 原生回归需要 Linux/WM，不能从本机 Windows 测试推断。`bash scripts/install-linux.sh` 在本机按设计拒绝执行，已使用 Windows 安装入口完成等价 release 同步。Linux 专用依赖安装夹具的 shebang 进程不能在 Windows 运行，不记为通过。浏览器注册/产物测试不等于各浏览器已完成真实端到端安装。

### 测试选择

UI 外部 Inbox 排队测试须先等模拟进程返回首轮流式文本，再发送中断；握手尚未结束时的中断会正确取消未发出的首轮，不能据此假设后续回复从第三轮开始。Windows 中文及空格路径按 JSON 编码后的字符串验证。

默认只运行本次增量及受影响模块/直接依赖链路的测试；仅用户明确要求时运行全量测试。模块或用例过滤器必须按改动选择，以下为入口示例，不要求逐条执行；纯文档改动只检查内容、链接和差异。Rust 改动仍运行格式检查及 Clippy。

```bash
cargo fmt --check
cargo test --locked backend::codex::tests::  # 示例：仅 Codex 后端测试
cargo test --locked sidebar_switches_damage_and_format_and_refresh_preserves_selection  # 示例：单用例
cargo clippy --locked --all-targets -- -D warnings
cargo run --locked -- --dev --screenshot /tmp/lexington.png  # 涉及真实渲染时
```

不带过滤器的 `cargo test --locked` 仅在用户明确要求全量测试时使用。专项 smoke、GIF 参考像素及性能测试按改动影响选择，受影响检查通过且无新问题后不重复运行。

`scripts/settings-smoke.py` 在临时配置/资源根目录下验证设置标题栏和正文空白拖动、按钮手势不拖窗、独立设置不移动桌宠、关闭/重开和失焦、开关点击、1×/1.5× DPI；不调用真实模型，显示默认粉色宠物，宜在隔离显示中运行，退出恢复指针并回收测试进程。

`scripts/native-smoke.py` 验证原生窗口尺寸、层级、拖动与交互；`position` 拖动后正常关闭并再次启动，断言位置精确恢复、透明边距可出屏及尺寸不变；`click-through` 创建真实底层窗口验证透明空白点击传递、立绘及侧栏保留点击；`drag-edges` 用真实指针逐步拖动可见立绘抓取点到四角，断言透明边距出屏、释放不回弹、重新按下拖回和尺寸保持；失败清理时也释放鼠标按钮。涉及窗口、缩放或拖动的改动还应在隔离 X11 桌面运行该脚本；可用模式、环境要求和调用方式见下文。


### 原生与集成专项验证

以下入口仅选择与改动直接相关的脚本/模式。

```bash
python3 scripts/native-smoke.py target/release/lexington /tmp/lexington.png drag
python3 scripts/settings-smoke.py target/release/lexington
python3 scripts/context-smoke.py target/release/lexington
node scripts/test-integrations.js
python3 scripts/test-dependencies.py
```

- native-smoke 模式为 `drag`、`drag-edges`、`click-through`、`position`、`clock`、`input`、`pin`、`idle`，`--production` 验证普通尺寸。需要 libX11/libXtst，优先 Xephyr + 窗口管理器隔离，同一 display 的指针测试必须串行；脚本移动并恢复鼠标。GIF 画布中心可能透明，拖动须从可见轮廓按下；验收包括阈值、精确位移、反向移动和尺寸保持，失败也须释放鼠标按钮。
- 侧栏模式跨启动验证使用独立 `XDG_DATA_HOME`，两次正常退出后检查 `lexington/app.ron` 中的固定模式。
- settings-smoke 用无资源临时根目录显示粉色测试形象，不代表安装资源丢失；覆盖 1×/1.5× DPI。集成和依赖脚本使用夹具，不调用模型或安装真实依赖；Node 仅是 JS 客户端测试的开发依赖。

## 实现补充与兼容细节

以下细节与模块表配合阅读，避免在项目操作规则中重复维护。

- 配置：旧 `llm` 可加载但不再使用。模型提供方进程覆盖包含 `model`、`model_provider=lexington_api` 及 `model_providers.lexington_api.*`（base_url、wire_api=responses、env_key=LEXINGTON_MODEL_API_KEY、requires_openai_auth=false）；thread/start 返回所选模型/提供方，不支持仅有 Chat Completions 的服务。
- UI：设置两栏为“常规 / 连接 / 打卡 / 闹钟”导航与分组滚动内容，说明用悬停提示；设置拖动在 primary_pressed 时取得 interaction_snapshot 命中 ID，再于锁外 read_response 排除其他 click/drag 控件，禁止锁内重入；无 WM 时直接 ConfigureWindow。气泡/输入局部坐标检查后再平移到窗口中心。字体组合为 Noto Sans CJK TTC 简体 face index 2、Liberation、Noto Sans Math；14px 中文字宽应接近 14px，不额外 FontTweak.scale 放大。
- 立绘与 GIF：`Normal_Normal` 的服装名为 `Normal`；随包 GIF 7 对、PNG 8 对。现有 1024×1024、67 帧 GIF 索引约 67 MiB；完整 RGBA 全库缓存曾导致系统杀进程。LZ4 压缩 RGBA 本机解压慢于索引合成，不作为默认方案；性能测试不代表整程序 CPU/RSS，后者需另行采样。跳过 Keep/Any 帧均更新处置画布，UI 不额外保存合成帧副本。
- 状态文件：打卡为 `$XDG_DATA_HOME/lexington/clock.json` / `clock-dev.json`（默认 `~/.local/share`），闹钟为同目录 `alarms.json` / `alarms-dev.json`，含 alarms/pending。Unix 本地时间使用 libc；打卡提醒后保留起点并立即保存已提醒状态，取消、撤销、午休和重启当天不再自动开始。
- 安装：用 `xdg-user-dir DESKTOP`，桌面文件为 `Lexington.desktop`，迁移小写旧文件，菜单 ID 仍为 `lexington.desktop`；设可执行并尝试可信标记。缺 Rust 时安装最小稳定工具链；发布包 `install.sh` 仅安装运行依赖。Codex 原生包须保留 `bin/codex-code-mode-host`、`codex-resources`、`codex-path`、`codex-package.json` 完整布局后链接 `~/.local/bin/codex`；`--skip-optional` 可跳过可选系统包。


### 天气增量验证

```bash
cargo test --locked backend::weather::tests::
cargo test --locked ui::weather::tests::
```

天气测试使用固定 JSON 和本地 HTTP 夹具，不连接真实天气服务或模型。配置、打卡或设置联动变化再选择对应模块/用例补测。
