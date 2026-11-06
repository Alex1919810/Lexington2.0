# 模块职责

代码按 UI、后端功能和工具代码三类组织。`main.rs` 负责启动，UI 负责展示与交互，后端负责业务规则，工具提供路径、解码和平台适配能力。

## 目录结构

```text
src/
├── main.rs
├── ui/
│   ├── mod.rs
│   ├── app.rs
│   ├── pet.rs
│   ├── sidebar.rs
│   ├── dialog.rs
│   ├── codex.rs
│   ├── input.rs
│   ├── assets.rs
│   ├── style.rs
│   └── tests.rs
├── backend/
│   ├── mod.rs
│   ├── config.rs
│   ├── resources.rs
│   ├── character.rs
│   ├── codex.rs
│   └── clock.rs
└── tools/
    ├── mod.rs
    ├── paths.rs
    ├── gif.rs
    └── platform.rs
tests/
└── fixtures/
    └── codex-server.py
docs/
└── MODULES.md
```

## 启动与依赖方向

- `main.rs`：创建 eframe 原生窗口、选择 X11/Wayland 后端、加载窗口图标并构造 `ui::Pet`。普通尺寸 500×500，`--dev` 为 800×600。Linux X11/XWayland 使用 Dock 桌面浮层类型，不设置 strut、不占用工作区；避免 GNOME 将透明窗体整块限制在屏幕内，使立绘中心可到屏幕边缘。
- `ui` 可调用 `backend` 和 `tools`；`backend` 不依赖 `ui`、egui 或 GPU 纹理；`tools` 不依赖应用 UI 或业务模块。
- `tools/platform.rs` 是平台与窗口框架的适配边界，可依赖 eframe/egui/winit；路径和解码工具不依赖这些框架。
- UI 内通过 `impl Pet` 分文件组织相关交互，必要的共享字段与方法限制在 UI 模块内。不要为了目录分层增加只有转发作用的抽象。

## UI

| 文件 | 职责与主要接口 | 依赖及约束 |
|---|---|---|
| `ui/mod.rs` | 声明 UI 子模块，对入口导出 `Pet` | 具体组件保持在 UI 模块内部 |
| `ui/app.rs` | 定义 `Pet` 与 `eframe::App`；初始化、刷新资源与纹理、每帧编排、结果接收、自动关闭对话、点击外部关闭、设置/关于菜单、截图、窗口状态和侧栏模式持久化。主要接口：`Pet::new`、`refresh`、`reload_config`、`load_picture`、`ui`；启动自动创建 Codex Client，设置窗口采用固定尺寸两栏布局，左侧“常规 / 连接”导航；常规页提供“无需审批”开关（保存成功后生效，下一次发送应用，保持当前会话）、配置路径复制和“重新加载”，连接页提供状态、CLI/工作目录路径查看与复制及“重启会话”（忙碌时禁用） | 持有 `CharacterState`、`ClockState`、Codex Client、流式消息和审批队列及 UI 状态；协调其他 UI 模块、配置/资源和平台适配；每帧清空并收集 `input_regions`，将可见立绘、控件、菜单及打开的窗口区域提交平台层，关闭窗口立即移除输入区域，不能包含全屏 pet Area 或 egui 残留缓存。设置窗口与桌宠侧栏同用 Foreground 层，避免侧栏遮挡设置；使用局部 14px 正文/按钮字号，不影响其他组件。窗口不可折叠，关闭时不触发操作；内容区独立滚动，长路径截断后悬停查看，说明仅在悬停提示中显示；在窗口回调结束后执行刷新。持久化侧栏模式及普通/开发模式独立的逻辑窗口位置（`windowPosition` / `devWindowPosition`，JSON 数组），不恢复 eframe 窗口尺寸及 egui 缓存；X11 保存时读取原生位置并换算 DPI，首帧校验当前立绘可见范围与活动显示器相交至少 24×24，透明边距可出屏，无效/失效屏幕位置回到主屏默认位置；保持普通/开发固定尺寸，不能相互污染。`restore_state` 负责读取并校验存储格式；截图事件读取后须释放 input 锁再发窗口命令；X11 浮层窗口仅在用户按下鼠标时调用平台 focus 以支持键盘输入，不在启动/悬停时抢焦点；生成期间不自动关闭对话；每 500ms 检查配置文件修改时间和长度，仅变化时读取解析，成功后应用，读取/解析失败保留上次有效值并在设置显示错误；普通设置不重建会话，Codex 程序/工作目录变更标记待重建，空闲时执行，忙碌时等任务结束。审批开关不改变运行中的任务，写入失败显示错误并保持原值；测试构造不启动真实 CLI |
| `ui/pet.rs` | `pet_ui` 绘制立绘/默认宠物和阴影，设置宠物、对话、输入层的共同变换，处理圆形命中、点击与拖动；默认宠物几何和变换测试也在此 | 使用 `Picture` 和平台拖动；将当前帧的归一化 alpha 覆盖区域映射到立绘实际位置并应用共同变换，默认宠物按身体轮廓生成输入区域，阴影不拦截点击；返回尺寸、变换和点击结果供 app 编排。容器 450×450，侧栏不参与共同缩放；拖动阈值 5 个逻辑像素；按下时由平台层读取原生屏幕指针/窗口坐标捕获锚点，不使用 egui 可能过时的窗口局部指针 |
| `ui/sidebar.rs` | `sidebar` 绘制六个按钮、悬停提示与动画，报告本帧是否处理点击；↻ 调用 `toggle_damage` 切换同服装正常/战损，▶ 调用 `next_illustration` 在当前状态队列循环；`clock` 将打卡结果转为对话 | 用 `damage_counterpart` 判断切换按钮是否可用，缺少同角色/格式/服装配对时禁用；战损时 ↻ 为橙色，悬停提示显示切换目标和下一张所属队列。选择变更后通过 app 重载纹理；按钮 40×40、间隔 10，仅按钮自身（随悬停缩放）加入输入区域、间隙穿透；侧栏点击须阻止同帧外部点击关闭 |
| `ui/dialog.rs` | `say` 初始化消息与打字机状态，`click_pet` 处理随机台词/显示全文/关闭，`bubble` 测量和绘制对话及滚动区 | 台词来自 `CharacterState`；使用共享样式与背景纹理。每 50ms 一个字符，按全文保留高度，14px 字号、21px 行高、10/14px 内边距；自动关闭与外部点击由 app 协调；只在对话显示时提交经过层变换的气泡输入区域 |
| `ui/codex.rs` | `start_codex` 创建或替换子进程，`poll_codex` 每帧汇总事件并展示回复，`codex_prompt_ui` 显示审批和问题 | 调用后端 Client；按 item ID 汇总增量，最终文本替换同 ID，避免重复，不二次打字；任务结束清理审批队列；可见审批/问题窗口向 app 提交输入区域，保证原生鼠标穿透不影响审批交互。命令/文件审批允许本次或拒绝，文件展示路径和 diff，问题可选项或自由回答，秘密答案遮蔽。重启先清理旧进程并清除待重建标记，结束与失败均解除忙碌状态 |
| `ui/input.rs` | `submit` 发起聊天，`input_ui` 管理输入框位置、焦点、悬停、加载动画（文案固定为“Lexington正在思考”）、错误展示、Enter 与 Escape | 提交给启动时创建的 Codex Client，每次提交附带当前 `noApproval` 值；不重复提交；Escape 中断正在进行的任务，空闲时清空输入/错误。后台回调只请求重绘，结果由 `ui/codex.rs` 接收。Enter 同时检查 `has_focus`/`lost_focus`；启动不抢焦点，占位色固定 #999；隐藏时不占据原生输入区域，alpha 大于 0.01 才加入。X11 下通过平台读取窗口局部指针发现悬停，恢复输入框显示，不依赖透明处收不到的鼠标移动事件，也不抢焦点 |
| `ui/assets.rs` | `Picture::load/size/texture` 将解码像素上传到 GPU，复用纹理并更新动画帧；`opaque_regions` 保存当前合成帧的归一化 alpha 覆盖条带（最多 256×256 网格，按格内最大 alpha ≥ 8 判断，保留细节且不额外常驻 RGBA 帧）；`icons` 加载内置侧栏图标；保留 GIF 像素、循环、局部帧处置、输入遮罩随透明度更新及纹理复用回归测试 | 调用 `tools::gif::Decoder`，不实现循环策略；立绘/背景使用线性 mipmap。字体和图标的 `include_bytes!` 路径相对 `src/ui/` |
| `ui/style.rs` | `install` 注册字体和浅色主题；`font`、`bubble_tail`、`draw_bubble`、`draw_shadow` 提供共同绘制规则 | 使用 egui 与 `Picture`；CJK TTC 使用简体中文 face index 2；对话尾尖距左边 28px，输入尾尖居中。气泡 Area 的溢出策略由对话/输入组件保持 `constrain(false)` |
| `ui/tests.rs` | 使用真实 egui 指针和键盘事件验证组件协作：侧栏、正常/战损对应切换、下一张的状态隔离、跨格式保留服装、角色与格式、对话、打卡、输入、Codex 流式回复/失败、配置热加载/错误保留/忙碌延迟重建、设置开关点击/持久化/同会话连续切换、空白背景/隐藏输入框不命中与关闭窗口输入区域立即清理、位置保存/恢复/开发模式隔离/无效或越界位置回退及中文排版；直接调用 `refresh` 验证资源重载恢复 | 使用模拟 Codex 子进程和临时配置，不访问真实模型或凭据；GIF 和几何专项测试靠近各自 UI 实现 |

## 后端功能

| 文件 | 职责与主要接口 | 依赖及约束 |
|---|---|---|
| `backend/mod.rs` | 声明后端子模块，集中保留配置、审批开关无损保存/错误保护、扫描的测试及共享临时目录夹具 | 协议测试位于 codex.rs，资源扫描和配置测试不需要外部服务 |
| `backend/config.rs` | 定义 `Config`、`DialogConfig`、`CodexConfig`、`ClockConfig`；`Config::load` 启动读取失败时默认回退，`read` 返回读取/解析错误供热加载保留旧值；补默认值并限制尺寸和计时范围；`Config::save_no_approval` 仅修改 `codex.noApproval` 并保存 | 标准库文件读取和 serde；保持 `config/config.json` 的 camelCase 兼容及错误时默认回退，未知旧 `llm` 字段忽略，不记录密钥；Codex 程序默认 codex，工作目录默认资源根目录；`noApproval` 默认 false。保存开关以 JSON 对象保留未知字段和旧密钥，损坏配置拒绝覆盖；同目录临时文件写入并同步后原子替换，Unix 新文件权限 0600 |
| `backend/resources.rs` | 定义 `Illustration`（含服装名 `outfit` 和战损标记 `damaged`）；`illustrations` 扫描排序立绘并解析配对元数据，`background` 选择首张支持的背景，`texts` 读取非空角色台词 | 标准库文件读取和 serde_json；只解析文件名最后的 `_Normal` / `_Damage` 后缀，ASCII 大小写不敏感；无后缀按正常处理，其完整文件名主体作为服装名。保留 `resource/` 布局，按目录识别角色与格式，拒绝台词路径穿越，不解码图片 |
| `backend/character.rs` | `CharacterState` 持有资源列表、角色、格式、正常/战损状态、选中索引与台词；`filtered` 返回当前角色/格式/状态的队列，`damage_counterpart` 查同服装的相反状态，`toggle_damage` 双向切换；`refresh` 保留当前路径并恢复失效选择，`next_illustration` 循环当前队列，`toggle_format`/`next_character` 切换格式/角色，`selected_path` 提供路径 | 仅依赖资源扫描模块和标准库；默认 GIF + 正常。配对严格限制同角色、同格式、服装名 ASCII 大小写不敏感，缺少对应版本时不切换；格式切换尽量保留同服装。选取回退顺序为当前格式/当前状态 → 另一格式/当前状态 → 当前格式/另一状态 → 另一格式/另一状态。含现有 15 对资源双向匹配、独立队列循环、后缀大小写、缺失配对及删除恢复测试；只选择路径，纹理和默认宠物由 UI 处理 |
| `backend/codex.rs` | `Client::start/submit(text, no_approval)/interrupt/reply/poll` 管理 app-server 子进程、stdio JSON 消息与事件通道；`Event`/`Prompt` 表达回复、审批、问题及连接状态 | 依赖配置、serde_json 和标准库，不依赖 UI 或 HTTP 客户端。启动 initialize → initialized → thread/start，输入可等待初始化，忙碌提示使用 Lexington 名称；一次程序运行复用一个 thread；保留 thread/start 返回的原始 approvalPolicy，逐轮明确传入 never 或原始策略，兼容字符串和 granular 对象，防止 turn/start 的持久覆盖导致关闭开关后仍无需审批；不覆盖沙箱。后台线程隔离阻塞读取，唤醒回调仅通知 UI。ACK 超时 30 秒，生成不限时；按 thread/turn 及请求 ID 关联，忽略过期 ACK；文本增量与最终 item 分开处理。缓存 item/started 中命令与文件 diff 用于审批，不支持的服务器请求返回 -32601。退出、重启或连接失败须终止并回收子进程；Unix 通过独立进程组与 libc 清理启动器及后代，先关 stdin 再回收并 join 读线程，防止后代持有 stdout 导致退出卡住；Windows 目前清理直接子进程，尚需该平台验证。不记录协议原文/凭据 |
| `backend/clock.rs` | `ClockState::toggle` 开始/取消计时并返回配置消息，`poll` 检查到期并清除状态、返回下班消息 | 仅依赖 `ClockConfig` 与 `Instant`；不绘制对话、不发起系统通知 |

## 工具代码

| 文件 | 职责与主要接口 | 依赖及约束 |
|---|---|---|
| `tools/mod.rs` | 声明路径、解码与平台模块 | 不承载业务状态 |
| `tools/paths.rs` | `resource_root` 定位资源根目录 | 顺序：`LEXINGTON_HOME` → 可执行文件旁存在 `resource/` 的目录 → 当前目录 |
| `tools/gif.rs` | `Decoder::load` 返回解码器和首张 RGBA 图；支持 GIF 及静态图片；`advance` 在帧到期时返回下一帧。内部处理逐帧合成、循环重开、剩余次数和延迟 | 依赖 image、gif 和标准库；不创建 GPU 纹理、不依赖 egui。只保留流式解码所需帧，禁止缓存全部 RGBA 帧。缺失循环扩展的 `Finite(0)` 视为无限循环，显式有限次数照常执行；不足 20ms 的延迟按 100ms 处理 |
| `tools/platform.rs` | `configure` 设置平台窗口属性；Linux `DragWindow::new/arm/update/configure/focus` 应用 X11 窗口提示和绝对坐标拖动；`position` 获取原生窗口屏幕坐标；`monitors` 返回 RandR 活动显示器矩形（主屏优先、含负坐标，失败回退根窗口尺寸），统一换算逻辑坐标；`pointer_position` 获取原生窗口局部指针，`set_input_regions` 应用 XShape Input 区域，含 DPI/拖动锚点回归测试 | 使用 eframe，Linux 另依赖 winit/x11rb（shape、randr 特性）；XShape ≥ 1.1 时仅更新 Input、不能裁剪 Bounding/绘制区域；逻辑坐标按 DPI 换算后裁到 viewport，缓存相同输入区域以避免重复请求，使用 checked 请求。原生 Wayland 不应用该 X11 遮罩；非 Linux 保留无操作配置入口。X11 请求关闭临时连接前执行 `.check()`；按下时同时查询原生屏幕指针与窗口位置捕获锚点，避免窗口移动后 egui 缓存的局部坐标滞后导致贴边重新拖动跳跃；阈值换算 DPI，不累加局部位移 |

## 新代码放置与文档维护

- 视觉、输入、焦点、纹理及窗口界面编排放入 UI；配置、资源选择、计时及 Codex 会话业务规则放入后端；不涉及业务的路径、解码及平台适配放入工具。
- 新增功能优先扩展所属模块，相关逻辑保持内聚；不要仅因代码只用一次就抽取新的辅助函数或文件。
- 后续任何代码文件改动，须同步检查并更新本文件对应的职责、接口、依赖、行为或约束；新增、删除、移动、重命名文件同时更新目录树。只改内部实现时，也应维护受影响的行为或约束说明。要求同时记录于项目 `AGENTS.md`。
- 安装、打包及原生验证脚本仍放在 `scripts/`；原资源布局和配置格式不随 Rust 模块拆分改变。

## 测试夹具

`tests/fixtures/codex-server.py` 用 Python 3 模拟 stdio app-server，供后端与 UI 测试共享；覆盖初始化、Unicode 输入、流式/最终消息去重、同一会话多轮与延迟 ACK、无需审批/原策略逐轮恢复（含 granular）、沙箱保持、命令/文件审批、提问、未支持请求、中断、失败恢复、崩溃和持有 stdout 的启动器后代清理。只在测试中通过 `include_str!` 使用，不作为发布版运行依赖；不访问用户 Codex 登录信息或真实模型。

## 验证入口

```bash
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo run --locked -- --dev --screenshot /tmp/lexington.png
```

`scripts/native-smoke.py` 验证原生窗口尺寸、层级、拖动与交互；`position` 拖动后正常关闭并再次启动，断言位置精确恢复、透明边距可出屏及尺寸不变；`click-through` 创建真实底层窗口验证透明空白点击传递、立绘及侧栏保留点击；`drag-edges` 用真实指针逐步拖动可见立绘抓取点到四角，断言透明边距出屏、释放不回弹、重新按下拖回和尺寸保持；失败清理时也释放鼠标按钮。涉及窗口、缩放或拖动的改动还应在隔离 X11 桌面运行该脚本；可用模式和环境要求见 README 与 AGENTS.md。
