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
│   ├── input.rs
│   ├── assets.rs
│   ├── style.rs
│   └── tests.rs
├── backend/
│   ├── mod.rs
│   ├── config.rs
│   ├── resources.rs
│   ├── character.rs
│   ├── llm.rs
│   └── clock.rs
└── tools/
    ├── mod.rs
    ├── paths.rs
    ├── gif.rs
    └── platform.rs
docs/
└── MODULES.md
```

## 启动与依赖方向

- `main.rs`：创建 eframe 原生窗口、选择 X11/Wayland 后端、加载窗口图标并构造 `ui::Pet`。普通尺寸 500×500，`--dev` 为 800×600。
- `ui` 可调用 `backend` 和 `tools`；`backend` 不依赖 `ui`、egui 或 GPU 纹理；`tools` 不依赖应用 UI 或业务模块。
- `tools/platform.rs` 是平台与窗口框架的适配边界，可依赖 eframe/egui/winit；路径和解码工具不依赖这些框架。
- UI 内通过 `impl Pet` 分文件组织相关交互，必要的共享字段与方法限制在 UI 模块内。不要为了目录分层增加只有转发作用的抽象。

## UI

| 文件 | 职责与主要接口 | 依赖及约束 |
|---|---|---|
| `ui/mod.rs` | 声明 UI 子模块，对入口导出 `Pet` | 具体组件保持在 UI 模块内部 |
| `ui/app.rs` | 定义 `Pet` 与 `eframe::App`；初始化、刷新资源与纹理、每帧编排、结果接收、自动关闭对话、点击外部关闭、设置/关于菜单、截图、窗口状态和侧栏模式持久化。主要接口：`Pet::new`、`refresh`、`load_picture`、`ui` | 持有 `CharacterState`、`ClockState`、请求接收通道及 UI 状态；协调其他 UI 模块、配置/资源和平台适配。只持久化侧栏模式，不恢复窗口及 egui 缓存；截图事件读取后须释放 input 锁再发窗口命令 |
| `ui/pet.rs` | `pet_ui` 绘制立绘/默认宠物和阴影，设置宠物、对话、输入层的共同变换，处理圆形命中、点击与拖动；默认宠物几何和变换测试也在此 | 使用 `Picture` 和平台拖动；返回尺寸、变换和点击结果供 app 编排。容器 450×450，侧栏不参与共同缩放；拖动阈值 5 个逻辑像素 |
| `ui/sidebar.rs` | `sidebar` 绘制六个按钮、悬停提示与动画，报告本帧是否处理点击；`clock` 将打卡结果转为对话 | 调用角色状态的刷新/切换能力及 `ClockState::toggle`，选择变更后通过 app 重载纹理。按钮 40×40、间隔 10；侧栏点击须阻止同帧外部点击关闭 |
| `ui/dialog.rs` | `say` 初始化消息与打字机状态，`click_pet` 处理随机台词/显示全文/关闭，`bubble` 测量和绘制对话及滚动区 | 台词来自 `CharacterState`；使用共享样式与背景纹理。每 50ms 一个字符，按全文保留高度，14px 字号、21px 行高、10/14px 内边距；自动关闭与外部点击由 app 协调 |
| `ui/input.rs` | `submit` 发起聊天，`input_ui` 管理输入框位置、焦点、悬停、加载动画、错误展示、Enter 与 Escape | 提交时重新读取配置并调用 `llm::request`，完成回调只请求重绘；app 接收结果并显示对话。Enter 同时检查 `has_focus`/`lost_focus`；启动不抢焦点，占位色固定 #999 |
| `ui/assets.rs` | `Picture::load/size/texture` 将解码像素上传到 GPU，复用纹理并更新动画帧；`icons` 加载内置侧栏图标；保留 GIF 像素、循环、局部帧处置与纹理复用回归测试 | 调用 `tools::gif::Decoder`，不实现循环策略；立绘/背景使用线性 mipmap。字体和图标的 `include_bytes!` 路径相对 `src/ui/` |
| `ui/style.rs` | `install` 注册字体和浅色主题；`font`、`bubble_tail`、`draw_bubble`、`draw_shadow` 提供共同绘制规则 | 使用 egui 与 `Picture`；CJK TTC 使用简体中文 face index 2；对话尾尖距左边 28px，输入尾尖居中。气泡 Area 的溢出策略由对话/输入组件保持 `constrain(false)` |
| `ui/tests.rs` | 使用真实 egui 指针和键盘事件验证组件协作：侧栏、角色与格式、刷新恢复、对话、打卡、输入、HTTP 回复及中文排版 | 使用本地 HTTP 模拟和临时配置，不访问真实 API；GIF 和几何专项测试靠近各自 UI 实现 |

## 后端功能

| 文件 | 职责与主要接口 | 依赖及约束 |
|---|---|---|
| `backend/mod.rs` | 声明后端子模块，集中保留配置、扫描和 HTTP 的测试及共享临时目录夹具 | 测试使用本地模拟 HTTP，不连接外部 API |
| `backend/config.rs` | 定义 `Config`、`DialogConfig`、`LlmConfig`、`ClockConfig`；`Config::load` 读取配置、补默认值并限制尺寸和计时范围 | 标准库文件读取和 serde；保持 `config/config.json` 的 camelCase 兼容及错误时默认回退，不记录 API key |
| `backend/resources.rs` | 定义 `Illustration`；`illustrations` 扫描排序立绘，`background` 选择首张支持的背景，`texts` 读取非空角色台词 | 标准库文件读取和 serde_json；保留 `resource/` 布局，按目录识别角色与格式，拒绝台词路径穿越，不解码图片 |
| `backend/character.rs` | `CharacterState` 持有资源列表、角色、格式、选中索引与台词；`refresh` 保留当前路径并恢复失效选择，`next_illustration`、`toggle_format`、`next_character` 实现切换，`selected_path` 提供当前资源路径 | 仅依赖资源扫描模块和标准库；默认 GIF、缺失格式自动回退。只选择路径，纹理和加载失败的默认宠物由 UI 处理 |
| `backend/llm.rs` | `call_llm` 执行兼容 Chat Completions 的 HTTP 请求和响应/错误解析；`request` 启动工作线程，返回 `Receiver<Result<String, String>>`，完成后调用 `wake` 回调 | 配置、reqwest、serde_json、标准库通道/线程；只发当前用户消息，超时 60 秒。回调不绑定任何 UI 框架；UI 负责防止重复提交与结果展示 |
| `backend/clock.rs` | `ClockState::toggle` 开始/取消计时并返回配置消息，`poll` 检查到期并清除状态、返回下班消息 | 仅依赖 `ClockConfig` 与 `Instant`；不绘制对话、不发起系统通知 |

## 工具代码

| 文件 | 职责与主要接口 | 依赖及约束 |
|---|---|---|
| `tools/mod.rs` | 声明路径、解码与平台模块 | 不承载业务状态 |
| `tools/paths.rs` | `resource_root` 定位资源根目录 | 顺序：`LEXINGTON_HOME` → 可执行文件旁存在 `resource/` 的目录 → 当前目录 |
| `tools/gif.rs` | `Decoder::load` 返回解码器和首张 RGBA 图；支持 GIF 及静态图片；`advance` 在帧到期时返回下一帧。内部处理逐帧合成、循环重开、剩余次数和延迟 | 依赖 image、gif 和标准库；不创建 GPU 纹理、不依赖 egui。只保留流式解码所需帧，禁止缓存全部 RGBA 帧。缺失循环扩展的 `Finite(0)` 视为无限循环，显式有限次数照常执行；不足 20ms 的延迟按 100ms 处理 |
| `tools/platform.rs` | `configure` 设置平台窗口属性；Linux `DragWindow::new/arm/update/configure` 应用 X11 窗口提示和绝对坐标拖动，含 DPI/拖动锚点回归测试 | 使用 eframe，Linux 另依赖 winit/x11rb；非 Linux 保留无操作配置入口。X11 请求关闭临时连接前执行 `.check()`；按下时捕获锚点，阈值换算 DPI，不累加局部位移 |

## 新代码放置与文档维护

- 视觉、输入、焦点、纹理及窗口界面编排放入 UI；配置、资源选择、计时及网络业务规则放入后端；不涉及业务的路径、解码及平台适配放入工具。
- 新增功能优先扩展所属模块，相关逻辑保持内聚；不要仅因代码只用一次就抽取新的辅助函数或文件。
- 后续任何代码文件改动，须同步检查并更新本文件对应的职责、接口、依赖、行为或约束；新增、删除、移动、重命名文件同时更新目录树。只改内部实现时，也应维护受影响的行为或约束说明。要求同时记录于项目 `AGENTS.md`。
- 安装、打包及原生验证脚本仍放在 `scripts/`；原资源布局和配置格式不随 Rust 模块拆分改变。

## 验证入口

```bash
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo run --locked -- --dev --screenshot /tmp/lexington.png
```

涉及窗口、缩放或拖动的改动还应在隔离 X11 桌面运行 `scripts/native-smoke.py`；可用模式和环境要求见 README 与 AGENTS.md。
