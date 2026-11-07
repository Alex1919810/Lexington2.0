# 模块职责

代码按 UI、后端功能和工具代码三类组织。`main.rs` 负责启动，UI 负责展示与交互，后端负责业务规则，工具提供路径、解码和平台适配能力。

## 目录结构

```text
src/
├── main.rs
├── ui/
│   ├── mod.rs
│   ├── alarm.rs
│   ├── app.rs
│   ├── pet.rs
│   ├── sidebar.rs
│   ├── settings.rs
│   ├── dialog.rs
│   ├── clock.rs
│   ├── codex.rs
│   ├── input.rs
│   ├── assets.rs
│   ├── style.rs
│   └── tests.rs
├── backend/
│   ├── mod.rs
│   ├── alarm.rs
│   ├── config.rs
│   ├── resources.rs
│   ├── character.rs
│   ├── codex.rs
│   ├── handoff.rs
│   └── clock.rs
└── tools/
    ├── mod.rs
    ├── paths.rs
    ├── activity.rs
    ├── gif.rs
    └── platform.rs
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
├── install-linux.sh
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
└── MODULES.md
```

## 启动与依赖方向

- `main.rs`：先处理 `--ask-files` / `--ask-json` / `--native-messaging` 无 GUI 入口，再取得 Linux 普通/开发模式独立实例锁并创建 eframe 原生窗口、选择 X11/Wayland 后端、加载窗口图标并构造 `ui::Pet`。普通尺寸 500×500，`--dev` 为 800×600。Linux X11/XWayland 使用 Dock 桌面浮层类型，不设置 strut、不占用工作区；避免 GNOME 将透明窗体整块限制在屏幕内，使立绘中心可到屏幕边缘。
- `ui` 可调用 `backend` 和 `tools`；`backend` 不依赖 `ui`、egui 或 GPU 纹理；`tools` 不依赖应用 UI 或业务模块。
- `tools/platform.rs` 是平台与窗口框架的适配边界，可依赖 eframe/egui/winit；路径和解码工具不依赖这些框架。
- UI 内通过 `impl Pet` 分文件组织相关交互，必要的共享字段与方法限制在 UI 模块内。不要为了目录分层增加只有转发作用的抽象。

## UI

| 文件 | 职责与主要接口 | 依赖及约束 |
|---|---|---|
| `ui/mod.rs` | 声明 UI 子模块，对入口导出 `Pet` | 具体组件保持在 UI 模块内部 |
| `ui/app.rs` | 定义 `Pet` 与 `eframe::App`；初始化、刷新资源与纹理、每帧编排、自动关闭/外部点击关闭对话、右键菜单、关于、截图、窗口状态和侧栏模式持久化。主要接口：`Pet::new`、`refresh`、`reload_config`、`load_picture`、`ui`；启动加载独立打卡/闹钟记录、活动监测、Codex Client 与外部 Inbox，逐帧调用独立设置及提醒 viewport，并在空闲时消费外部请求 | 持有 CharacterState、ClockState、AlarmUi、Monitor、Client、Inbox 和 UI 状态；设置拖动单独保存原生按下锚点/目标窗口、鼠标状态和是否获准拖动，关闭后清空；测试构造不启动真实 CLI/监测或访问用户记录。每帧收集可见立绘/控件/菜单及 Embedded 窗口的 input_regions，不包含独立 viewport、全屏 pet Area 或历史缓存。右键菜单在绘制闭包中收集当前内容加 Frame 边距，仅菜单打开时加入；egui 0.31 不能依赖 context_menu 返回值。X11 仅鼠标按下聚焦，启动/悬停不抢焦点。独立保存普通/开发模式逻辑位置（windowPosition/devWindowPosition），仅恢复位置，尺寸固定 500×500/800×600，禁用 eframe 尺寸及 egui 缓存持久化；原生位置按 DPI 换算，可见立绘需与任一活动显示器相交至少 24×24，透明边距可出屏，无效位置回默认主屏。截图释放 input 锁后发 viewport 命令。配置每 500ms 检查时间/长度，成功才替换，失败保留旧值；普通设置保持会话，程序/工作目录/模型/自定义 API 参数改变待空闲重建；connection_draft 独立保存未提交表单，关闭设置时丢弃草稿 |
| `ui/pet.rs` | `pet_ui` 绘制立绘/默认宠物和阴影，设置宠物、对话、输入层的共同变换，处理圆形命中、点击与拖动；默认宠物几何和变换测试也在此 | 使用 `Picture` 和平台拖动；将当前帧的归一化 alpha 覆盖区域映射到立绘实际位置并应用共同变换，默认宠物按身体轮廓生成输入区域，阴影不拦截点击；返回尺寸、变换和点击结果供 app 编排。容器 450×450，侧栏不参与共同缩放；拖动阈值 5 个逻辑像素；按下时由平台层读取原生屏幕指针/窗口坐标捕获锚点，不使用 egui 可能过时的窗口局部指针 |
| `ui/settings.rs` | `settings_ui` 创建独立 464×390 原生 viewport；`settings_contents` 复用两栏设置内容，包含常规/连接/打卡/闹钟、模型/API/密码 Key 表单与保存/取消修改、开关保存、资源刷新和会话重启 | 无边框、不透明、Dock/置顶；初次打开按桌宠所在显示器适配位置，仅初始化时 with_position，后续自由拖动。标题栏及正文空白均可拖动，后台拖动控件先于正文注册；按下命中其他可点击/拖动控件时禁止本次窗体拖动，正文关闭拖拽滚动，保留滚轮和滚动条。X11 新按下时用 screen_pointer/window_at_pointer 核对进程与设置窗口标题、捕获原生屏幕位置，经 move_window 发送 WM 绝对移动；保留候选锚点等待可能晚一帧到达的 egui 正文命中结果，松开/关闭清除。不能依赖缓存 outer_rect 或 Dock 的 StartDrag；Wayland/其他平台走 StartDrag。映射后 800ms 激活，关闭仅销毁子 viewport；不把独立设置矩形加入桌宠 XShape。Embedded 后端回退 Foreground Window 并记录输入区域；14px 文本局部应用，路径截断/悬停/复制，内容滚动。常规/打卡/连接操作在 viewport 回调结束后执行；连接表单点击保存才验证并原子写入，成功调用 reload_config 触发空闲重连；Key 使用 password TextEdit 且不提供明文按钮，错误独立保留；写入成功才改内存，未知配置/旧密钥保留；重启会话忙碌时禁用 |
| `ui/sidebar.rs` | `sidebar` 绘制六个按钮、悬停提示与动画，报告本帧是否处理点击；↻ 调用 `toggle_damage` 切换同服装正常/战损，▶ 调用 `next_illustration` 在当前状态队列循环；`clock` 将持久化后的手动打卡结果转为对话，失败可从打卡设置查看 | 用 `damage_counterpart` 判断切换按钮是否可用，缺少同角色/格式/服装配对时禁用；战损时 ↻ 为橙色，悬停提示显示切换目标和下一张所属队列。选择变更后通过 app 重载纹理；按钮 40×40、间隔 10，仅按钮自身（随悬停缩放）加入输入区域、间隙穿透；侧栏点击须阻止同帧外部点击关闭 |
| `ui/dialog.rs` | `say` 初始化消息与打字机状态，`click_pet` 处理随机台词/显示全文/关闭，`bubble` 测量和绘制对话及滚动区 | 台词来自 `CharacterState`；使用共享样式与背景纹理。每 50ms 一个字符，按全文保留高度，14px 字号、21px 行高、10/14px 内边距；自动关闭与外部点击由 app 协调；只在对话显示时提交经过层变换的气泡输入区域 |
| `ui/codex.rs` | `start_codex` 创建或替换子进程，`poll_codex` 每帧汇总事件并展示回复，`codex_prompt_ui` 显示审批和问题 | 调用后端 Client；按 item ID 汇总增量，最终文本替换同 ID，避免重复，不二次打字；任务结束清理审批队列；可见审批/问题窗口向 app 提交输入区域，保证原生鼠标穿透不影响审批交互。命令/文件审批允许本次或拒绝，文件展示路径和 diff，问题可选项或自由回答，秘密答案遮蔽。重启先清理旧进程并清除待重建标记，结束与失败均解除忙碌状态；external_turn 标记右键请求，完成不清空草稿，异步失败也通过气泡显示 |
| `ui/alarm.rs` | `AlarmUi` 管理表单和错误状态；`load_alarms` 加载持久化记录；`alarm_settings_ui` 提供 HH:MM、提示语、每天重复、新增/编辑/删除及启停；`alarm_popup_ui` 每秒检查到期并显示独立原生窗口 | 依赖 AlarmState、clock 本地时间和 paths；弹窗 420×260、无边框不透明置顶，按桌宠所在显示器中央定位（X11 用 RandR，支持负坐标和 DPI），不移动桌宠。X11 使用 Dock 层，普通 AlwaysOnTop 可能被桌宠 Dock 遮挡；映射后 800ms 一次性 Focus/AlwaysOnTop（晚于主窗启动置顶），不复用桌宠 XShape 遮罩。多个到期提醒合并，长文本滚动，固定确认按钮；确认或系统关闭先持久化清除 pending，保存失败保留弹窗。非原生 egui 集成降级为居中 Window 并加入输入区域；Wayland 位置由合成器决定 |
| `ui/clock.rs` | `poll_clock` 消费系统采样、驱动自动确认及提醒；`clock_settings_ui` 展示自动开关、检测状态、起点和编辑；`clock_action` 保存/撤销/手动操作；`clock_notice_ui` 提供修改和撤销入口 | 调用后端 ClockState 和工具 Monitor；自动提示加入原生输入区域，关闭立即移除；过期采样不确认，自动开关/确认时长热加载会重置候选；加载失败禁用更改并显示错误，避免覆盖损坏记录 |
| `ui/input.rs` | `submit` 发起输入框聊天，`poll_handoff` 从 Inbox 顺序直接发送外部上下文，二者通过 `send_message` 复用发送状态；`input_ui` 管理输入框位置、焦点、悬停、加载动画（文案固定为“Lexington正在思考”）、错误展示、Enter 与 Escape | 提交给启动时创建的 Codex Client，每次提交附带当前 `noApproval` 值；外部请求不填输入框、不改焦点、保留草稿，不中断忙碌任务，立即提交失败通过气泡提示；不重复提交；Escape 中断正在进行的任务，空闲时清空输入/错误。后台回调只请求重绘，结果由 `ui/codex.rs` 接收。Enter 同时检查 `has_focus`/`lost_focus`；启动不抢焦点，占位色固定 #999；鼠标进入共同变换后的 450×450 立绘容器或输入区域时显示，输入焦点或生成中保持显示；离开悬停且失焦后淡出并移除输入区域，草稿和错误仅保留内容，不强制显示；原生失焦显式清除 TextEdit 焦点 ID；隐藏时不占据原生输入区域，alpha 大于 0.01 才加入。X11 下通过平台读取窗口局部指针发现悬停，失败回退 egui 指针，恢复输入框显示，不依赖透明处收不到的鼠标移动事件，也不抢焦点 |
| `ui/assets.rs` | `Picture::load/size/texture` 将解码像素上传到 GPU，复用纹理并更新动画帧；`opaque_regions` 保存当前合成帧的归一化 alpha 覆盖条带（最多 256×256 网格，按格内最大 alpha ≥ 8 判断，逐格扫描命中即停止，使用 ceil 反推像素边界以等价保留非整除尺寸细节，不额外常驻 RGBA 帧）；`icons` 加载内置侧栏图标；保留 GIF 抽帧后的像素、循环、跳过帧的局部处置、输入遮罩随显示帧透明度更新、非整除尺寸 alpha 覆盖等价性及纹理复用回归测试 | 调用 `tools::gif::Decoder`，读取当前 RgbaImage 并保持只读；颜色转换和 alpha 区域计算仍在 UI 层，不实现循环策略；立绘/背景使用线性 mipmap。字体和图标的 `include_bytes!` 路径相对 `src/ui/` |
| `ui/style.rs` | `install` 注册字体和浅色主题；`font`、`bubble_tail`、`draw_bubble`、`draw_shadow` 提供共同绘制规则 | 使用 egui 与 `Picture`；CJK TTC 使用简体中文 face index 2；对话尾尖距左边 28px，输入尾尖居中。气泡 Area 的溢出策略由对话/输入组件保持 `constrain(false)` |
| `ui/tests.rs` | 使用真实 egui 指针和键盘事件验证组件协作：侧栏、正常/战损对应切换、下一张的状态隔离、跨格式保留服装、角色与格式、对话、打卡、输入、外部请求在任务中排队/同一会话多轮/来源路径和行号保留/完成不覆盖草稿、普通/开发窗口悬停立绘唤出输入、点击聚焦输入、原生失焦后草稿/错误隐藏且内容保留、再悬停恢复及生成中显示、Codex 流式回复/失败、配置热加载/错误保留/忙碌延迟重建、设置开关点击/持久化/同会话连续切换、API 表单滚动保存/密码遮蔽/非法地址拒绝/成功保存及忙碌延迟重连、空白背景/隐藏输入框不命中与关闭窗口输入区域立即清理、右键菜单关闭项在立绘外持续接收原生输入并触发退出、自动打卡提示撤销、时间编辑、自动开关持久化和热加载、闹钟表单校验、编辑/保存、启停/删除、弹窗外点击不消失和确认持久化、位置保存/恢复/开发模式隔离/无效或越界位置回退及中文排版；直接调用 `refresh` 验证资源重载恢复 | 使用模拟 Codex 子进程和临时配置，不访问真实模型或凭据；GIF 和几何专项测试靠近各自 UI 实现 |

## 后端功能

| 文件 | 职责与主要接口 | 依赖及约束 |
|---|---|---|
| `backend/mod.rs` | 声明后端子模块（含外部上下文 handoff），集中保留配置、审批/自动打卡开关无损保存、确认时长边界及错误保护、扫描的测试及共享临时目录夹具 | 协议测试位于 codex.rs，资源扫描和配置测试不需要外部服务 |
| `backend/handoff.rs` | `Request::validate/prompt` 定义选区/路径/来源/行号与分析提示；Linux `Endpoint::bind/start` / `Inbox::poll` 提供用户级单实例、本地入队通道；`deliver` 自动启动桌宠并交付一次；`native_host` 实现浏览器本机协议 | 无 UI 依赖。Unix socket 位于 XDG_RUNTIME_DIR 下 lexington-UID（开发模式 -dev），缺失时用临时目录；目录须 0700/当前用户且非软链接，flock 独占锁下清理旧 socket，0600 socket 加 SO_PEERCRED 校验当前用户。长度帧 ≤1 MiB、队列 16 条、最多 32 个绝对路径，读取/写入超时，后台线程只唤醒 UI；ACK 表示入队，发出后不自动重试避免重复；无 HTTP/TCP/剪贴板监测。普通/开发各一个实例；非 Linux 外部交付明确不支持。含排队/满队列/残留恢复/不安全路径/分帧及校验测试 |
| `backend/config.rs` | 定义 `Config`、`DialogConfig`、`CodexConfig`、`ClockConfig`（CodexConfig 的 Debug 隐藏 Key，validate_connection 校验模型/基础地址/Key，connection_changed 排除审批开关）；`Config::load` 启动读取失败时默认回退，`read` 返回读取/解析错误供热加载保留旧值；补默认值并限制尺寸和计时范围；`Config::save_no_approval` / `save_auto_clock` / `save_connection` 通过共享无损原子保存入口修改相应字段，连接字段 model/useCustomApi/baseUrl/apiKey 同次原子写入 | 标准库文件读取、serde 与 url（校验 API 地址）；保持 `config/config.json` 的 camelCase 兼容及错误时默认回退，未知旧 `llm` 字段忽略，不记录密钥；Codex 程序默认 codex，工作目录默认资源根目录；`noApproval` 默认 false；自动打卡默认开启，confirmationSeconds 默认 180，限制 30～600 秒。保存开关以 JSON 对象保留未知字段和旧密钥，损坏配置拒绝覆盖；同目录临时文件写入并同步后原子替换，Unix 新文件权限 0600；模型/地址/Key 裁掉首尾空白，基础地址去尾斜线；自定义模式必须提供模型、无内嵌账号/查询/片段的 HTTP(S) URL 和有效 Key，无效热加载保留旧值；API Key 为用户配置文件明文存储，不能记录到日志/Debug |
| `backend/resources.rs` | 定义 `Illustration`（含服装名 `outfit` 和战损标记 `damaged`）；`illustrations` 扫描排序立绘并解析配对元数据，`background` 选择首张支持的背景，`texts` 读取非空角色台词 | 标准库文件读取和 serde_json；只解析文件名最后的 `_Normal` / `_Damage` 后缀，ASCII 大小写不敏感；无后缀按正常处理，其完整文件名主体作为服装名。保留 `resource/` 布局，按目录识别角色与格式，拒绝台词路径穿越，不解码图片 |
| `backend/character.rs` | `CharacterState` 持有资源列表、角色、格式、正常/战损状态、选中索引与台词；`filtered` 返回当前角色/格式/状态的队列，`damage_counterpart` 查同服装的相反状态，`toggle_damage` 双向切换；`refresh` 保留当前路径并恢复失效选择，`next_illustration` 循环当前队列，`toggle_format`/`next_character` 切换格式/角色，`selected_path` 提供路径 | 仅依赖资源扫描模块和标准库；默认 GIF + 正常。配对严格限制同角色、同格式、服装名 ASCII 大小写不敏感，缺少对应版本时不切换；格式切换尽量保留同服装。选取回退顺序为当前格式/当前状态 → 另一格式/当前状态 → 当前格式/另一状态 → 另一格式/另一状态。含现有 15 对资源双向匹配、独立队列循环、后缀大小写、缺失配对及删除恢复测试；只选择路径，纹理和默认宠物由 UI 处理 |
| `backend/codex.rs` | `Client::start/submit(text, no_approval)/interrupt/reply/poll` 管理 app-server 子进程、stdio JSON 消息与事件通道；`Event`/`Prompt` 表达回复、审批、问题及连接状态 | 依赖配置、serde_json 和标准库，不依赖 UI 或 HTTP 客户端。自定义模型/提供方通过 -c 覆盖（JSON 字符串编码兼容 TOML），provider=lexington_api、wire_api=responses、requires_openai_auth=false；Key 仅传 LEXINGTON_MODEL_API_KEY 子进程环境，关闭模式显式移除变量，不修改用户 Codex 配置；初始化/流式事件中的完整 Key 统一遮蔽。启动 initialize → initialized → thread/start，输入可等待初始化，忙碌提示使用 Lexington 名称；一次程序运行复用一个 thread；保留 thread/start 返回的原始 approvalPolicy，逐轮明确传入 never 或原始策略，兼容字符串和 granular 对象，防止 turn/start 的持久覆盖导致关闭开关后仍无需审批；不覆盖沙箱。后台线程隔离阻塞读取，唤醒回调仅通知 UI。ACK 超时 30 秒，生成不限时；按 thread/turn 及请求 ID 关联，忽略过期 ACK；文本增量与最终 item 分开处理。缓存 item/started 中命令与文件 diff 用于审批，不支持的服务器请求返回 -32601。退出、重启或连接失败须终止并回收子进程；Unix 通过独立进程组与 libc 清理启动器及后代，先关 stdin 再回收并 join 读线程，防止后代持有 stdout 导致退出卡住；Windows 目前清理直接子进程，尚需该平台验证。不记录协议原文/凭据 |
| `backend/alarm.rs` | `AlarmState::load/save/toggle/delete/poll/dismiss` 管理多个本地时间闹钟、调度与待确认提醒 | 不依赖 UI；时间严格 HH:MM，提示语 1～500 字符；一次性闹钟响后关闭，每天重复按 libc mktime 计算下一本地日期，不能固定加 86400 秒，跳过 DST 不存在时间。设置时间已过时安排下一次；启用重新计算；到期（含休眠/停机错过）每个闹钟只补一次并推进到未来，pending 跨重启保存；同时到期合并，重启不重复。普通/开发记录隔离，0600 临时文件同步后原子替换，保存失败不提交内存，损坏数据拒绝覆盖。含同时触发、重复/错过、重启、校验、编辑启停删除与存储失败测试 |
| `backend/clock.rs` | `ClockState::load/observe/toggle/edit/undo/poll` 管理本地日期的自动确认、手动打卡、时间修正、撤销和一次性下班提醒；`now/local_time` 处理时间戳与本地时间 | 依赖配置、serde、标准库及 Unix libc 本地时间转换，不依赖 UI。自动开始须持续活跃默认 180 秒，回溯首次操作；空闲满 60 秒、锁屏/不可用样本、采样间隔超过 10 秒、时钟跳变均重置确认，确认终点须有新操作。每天自动一次，手动取消、撤销或提醒后当天不再自动开始；跨本地日期重置。记录立即原子保存，失败不提交内存状态；起点、已处理和已提醒状态跨启动恢复，开发模式隔离；编辑拒绝未来或不存在的本地时间。含模拟时钟、断档、持久化、撤销、编辑和存储错误测试 |

## 工具代码

| 文件 | 职责与主要接口 | 依赖及约束 |
|---|---|---|
| `tools/mod.rs` | 声明路径、活动监测、解码与平台模块 | 不承载业务状态 |
| `tools/paths.rs` | `resource_root` 定位资源根目录；`clock_state_file` / `alarm_state_file` 返回普通/开发模式独立的用户数据记录路径 | 顺序：`LEXINGTON_HOME` → 可执行文件旁存在 `resource/` 的目录 → 当前目录；闹钟（alarms.json / alarms-dev.json）及打卡状态使用 XDG_DATA_HOME，默认 ~/.local/share/lexington，不能写入资源目录 |
| `tools/activity.rs` | `Monitor::start/poll` 在后台每 2 秒采样系统空闲时间和会话可用性，返回带单调时间戳的 Sample；退出停止并回收线程 | Linux 用 loginctl 排除锁屏、远程和非活动会话，优先 gdbus 调用 Mutter IdleMonitor，原生 X11 可回退 XScreenSaver；Wayland 禁止回退 XWayland 空闲时钟。子进程有超时并清理，UI 不阻塞；不记录键鼠内容；不可用样本重置自动确认，保留手动入口；测试可注入采样 |
| `tools/gif.rs` | `Decoder::load/advance` 返回当前 RgbaImage；支持 GIF/静态图片，控制隔帧播放、时序、循环次数、索引缓存与超限流式回退 | 不依赖 UI，不创建 GPU 纹理，不缓存合成帧。源帧 0、2、4…输出，各跳过帧先独立归一化延迟再合并到前一显示帧，奇数结尾保留，不跨循环合并，总时长不变。首轮缓存索引帧/调色板，96 MiB 预算仅针对单个当前 GIF，超限释放前缀并保持流式；完整缓存回放不重读源文件，循环时清空处置画布。保留透明索引、局部/全局调色板、交错帧及 Keep/Background/Previous：跳过 Keep 直接更新处置画布，跳过 Background 仅清区域，Previous 保留原画布，无需合成/上传被跳过帧的 RGBA。Finite(0) 视为无限，显式有限次数保留，原帧低于 20ms 按 100ms。回归覆盖无源文件回放、缓存超限、奇数/变延迟、跳过帧依赖、有限循环和 UI 纹理/透明遮罩；全量参考像素与合并延迟对比用 cargo test --release --locked bundled_frames_match_reference_decoder_pixel_for_pixel -- --ignored；frame_sampling_performance 用 --release --ignored --nocapture --test-threads=1 比较同等循环的原帧率/抽帧取帧，不含 UI/纹理开销 |
| `tools/platform.rs` | `configure` 设置平台窗口属性；Linux `DragWindow::new/arm/update/configure/focus` 应用 X11 窗口提示和绝对坐标拖动；`window_at_pointer` 从原生指针下窗口层级查找当前进程/指定标题并返回真实位置；`move_window` 对指定子窗口发送经确认的 _NET_MOVERESIZE_WINDOW，无窗口管理器时直接 ConfigureWindow；`position` 获取原生窗口屏幕坐标；`monitors` 返回 RandR 活动显示器矩形（主屏优先、含负坐标，失败回退根窗口尺寸），统一换算逻辑坐标；`screen_pointer` 获取屏幕绝对指针和左键状态供独立设置拖动；`pointer_position` 获取原生窗口局部指针，`set_input_regions` 应用 XShape Input 区域，含 DPI/拖动锚点回归测试 | 使用 eframe，Linux 另依赖 winit/x11rb（shape、randr、screensaver 特性）；XShape ≥ 1.1 时仅更新 Input、不能裁剪 Bounding/绘制区域；逻辑坐标按 DPI 换算后裁到 viewport，缓存相同输入区域以避免重复请求，使用 checked 请求。原生 Wayland 不应用该 X11 遮罩；非 Linux 保留无操作配置入口。X11 请求关闭临时连接前执行 `.check()`；按下时同时查询原生屏幕指针与窗口位置捕获锚点，避免窗口移动后 egui 缓存的局部坐标滞后导致贴边重新拖动跳跃；阈值换算 DPI，不累加局部位移；独立设置使用根窗口指针查询实现 Dock 标题栏拖动，不依赖 WM 的 StartDrag |

## 外部右键客户端与安装脚本

| 文件 | 职责与主要接口 | 依赖及约束 |
|---|---|---|
| `integrations/vscode/package.json` / `extension.js` | 注册编辑器选区和文件资源管理器的“看看Lexington怎么说”；通过无 shell 子进程 stdin JSON 调用 `--ask-json` | 仅 VS Code 扩展宿主依赖 JS；本地 UI 扩展，不增加主程序 Node 运行链。选区带本地路径/起始行；未保存标记、远程 URI 不冒充本机路径；失败显示错误；可配置 executable |
| `integrations/browser/background.js` / 两份 manifest | Firefox/Chromium 选区右键菜单，通过 `com.lexington.context` Native Messaging 发送选区及 page/frame URL；file URL 转本地路径 | 仅 contextMenus/nativeMessaging 权限，无页面监听/网络服务；超过大小不发送；失败工具栏 ! 与悬停提示；Chromium public key 固定 ID，Firefox ID 固定且正式包需签名；沙箱浏览器桥接仍需平台支持 |
| `integrations/nautilus/send.py` / `lexington_menu.py` | 脚本子菜单入口按 URI 解码本地路径，读取安装位置 JSON 并调用 `--ask-files`；可选 Nautilus 4.0 顶层菜单提供器转交同一脚本 | Python 3 标准库；顶层提供器额外依赖 nautilus-python/gi。不解析 shell、不读剪贴板；失败用 notify-send 提示；最多 32 个本地文件；缺少系统扩展支持仍保留脚本入口 |
| `scripts/integrations.py` | `build` 用标准库生成 VSIX、Firefox XPI 和 Chromium 目录；`install` 安装 Nautilus 脚本/提供器、Native Messaging 宿主与白名单，检测 code 自动安装 VSIX | 无 npm；Firefox 未签名只可临时加载，浏览器安装需按 integrations/README.md；遵循 XDG 数据/配置路径，不更改浏览器签名策略、不关闭文件管理器 |
| `scripts/install-linux.sh` / `package.sh` | release 构建、保留配置的安装及打包，并带上集成客户端；先调用依赖安装，源码模式构建，发布包模式直接复制二进制且跳过 Rust；安装脚本创建桌面快捷方式、注册菜单/通信桥；发布包提供 install.sh 和安装脚本 | 安装二进制先复制到同目录临时文件再原子替换，避免运行中 ETXTBSY；桌面名 Lexington；升级后按项目约定安全重启旧实例 |
| `scripts/install-dependencies.sh` / `dependencies.py` | Python 引导、Ubuntu/Debian 系统依赖检查/授权安装、源码安装的缺失 Rust 工具链安装，以及缺失 Codex 官方原生包下载安装 | 默认固定已验证 CLI 0.166.0；只在缺失时下载，保留现有 CLI/登录配置，无法运行的现有 CLI 明确报错。原生发行包来自 @openai/codex 官方 registry，SHA-512 完整性校验、安全解包、保留完整 vendor 布局，用户级安装 ~/.local/bin/codex，无 Node/npm；Rust rustup-init 用官方 SHA-256 校验。--runtime-only 跳过构建依赖，--skip-optional 跳过 Nautilus 等可选依赖；系统包调用 sudo/pkexec，可选取消保留脚本菜单，必需依赖失败阻止后续安装；不运行 Codex 登录 |
| `scripts/test-dependencies.py` | 用临时 HOME、下载/包夹具验证缺失安装、已有 CLI 保留、资源布局、完整性/路径穿越拒绝及用户配置保留 | 不访问网络、不写系统目录、不安装真实 CLI，不替代真实平台安装验证 |
| `scripts/context-smoke.py` / `test-integrations.js` | 验证真实 CLI/native framing/Nautilus URI 路径和非法输入隔离；用宿主桩验证 VS Code 选区/路径/远程/未保存及浏览器菜单/来源/失败提示 | 测试不调用模型、不修改剪贴板/用户配置；前者 Python 3，后者开发机 Node；Rust UI 测试另验证 Inbox → Codex 完整队列与草稿保护 |
| `integrations/README.md` | 各客户端安装、适用范围与 native bridge 协议说明 | 记录顶层菜单依赖、浏览器签名与沙箱限制，不宣称覆盖任意软件内部菜单 |

## 新代码放置与文档维护

- 视觉、输入、焦点、纹理及窗口界面编排放入 UI；配置、资源选择、计时及 Codex 会话业务规则放入后端；不涉及业务的路径、解码及平台适配放入工具。
- 新增功能优先扩展所属模块，相关逻辑保持内聚；不要仅因代码只用一次就抽取新的辅助函数或文件。
- 后续任何代码文件改动，须同步检查并更新本文件对应的职责、接口、依赖、行为或约束；新增、删除、移动、重命名文件同时更新目录树。只改内部实现时，也应维护受影响的行为或约束说明。要求同时记录于项目 `AGENTS.md`。
- 安装、打包及原生验证脚本仍放在 `scripts/`；原资源布局和配置格式不随 Rust 模块拆分改变。

## 测试夹具

`tests/fixtures/codex-server.py` 用 Python 3 模拟 stdio app-server，供后端与 UI 测试共享；覆盖初始化、Unicode 输入、流式/最终消息去重、同一会话多轮与延迟 ACK、模型/提供方启动参数与 Key 环境/命令行隔离及完整 Key 遮蔽、无需审批/原策略逐轮恢复（含 granular）、沙箱保持、命令/文件审批、提问、未支持请求、中断、失败恢复、崩溃和持有 stdout 的启动器后代清理。只在测试中通过 `include_str!` 使用，不作为发布版运行依赖；不访问用户 Codex 登录信息或真实模型。

## 验证入口

```bash
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo run --locked -- --dev --screenshot /tmp/lexington.png
```

`scripts/settings-smoke.py` 在临时配置/资源根目录下验证设置标题栏和正文空白拖动、按钮手势不拖窗、独立设置不移动桌宠、关闭/重开和失焦、开关点击、1×/1.5× DPI；不调用真实模型，显示默认粉色宠物，宜在隔离显示中运行，退出恢复指针并回收测试进程。

`scripts/native-smoke.py` 验证原生窗口尺寸、层级、拖动与交互；`position` 拖动后正常关闭并再次启动，断言位置精确恢复、透明边距可出屏及尺寸不变；`click-through` 创建真实底层窗口验证透明空白点击传递、立绘及侧栏保留点击；`drag-edges` 用真实指针逐步拖动可见立绘抓取点到四角，断言透明边距出屏、释放不回弹、重新按下拖回和尺寸保持；失败清理时也释放鼠标按钮。涉及窗口、缩放或拖动的改动还应在隔离 X11 桌面运行该脚本；可用模式和环境要求见 README 与 AGENTS.md。
