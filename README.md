# Lexington — Rust 桌面宠物

一个使用 Rust 编写界面和业务逻辑的桌面宠物。eframe/egui + glow 直接绘制透明原生窗口，运行时无需 Node.js、Electron、WebView 或 N-API。

支持 GIF/PNG 立绘、多角色切换、随机台词、打字机对话、Codex 会话与流式回复、打卡提醒、自定义闹钟、自定义气泡背景和窗口拖动。沿用原有 `resource/` 目录和 `config/config.json` 格式。

## 快速开始

安装 Rust 稳定版（支持 edition 2024），在项目目录运行：

```bash
cargo run --release --locked
```

默认窗口为 500×500、透明、无边框、置顶。开发窗口与原版相同，为 800×600，首次位置 (100,100)，之后恢复上次位置：

```bash
cargo run --locked -- --dev
```

Codex 输入输出需要预先安装并登录 Codex CLI（本机验证版本为 0.166.0）。Lexington 启动时自动运行 `codex app-server --stdio`，缺少 CLI 或连接失败会在输入区显示错误，宠物的其他功能仍可使用。

Linux 需要桌面 OpenGL/EGL 驱动及 X11/XWayland。存在 `DISPLAY` 时默认选 X11/XWayland；可通过 `cargo run --release --locked -- --wayland` 选择原生 Wayland。

| 平台 | 支持及验证情况 |
|---|---|
| Linux X11 / XWayland | 提供窗口定位、置顶、拖动及隐藏任务栏；已在独立 X11 桌面验证 |
| Linux 原生 Wayland | 提供后端；窗口位置和层级由合成器控制，尚需对应桌面实测 |
| Windows / macOS | 使用 eframe 原生后端；尚需对应系统构建和运行验证 |

资源查找顺序：`LEXINGTON_HOME` 环境变量指定的目录 → 可执行文件旁的 `resource/` → 当前工作目录。发布时把 `resource/` 与 `config/` 放在可执行文件旁，启动不依赖工作目录。

## 构建和安装

```bash
cargo build --release --locked
bash scripts/package.sh
bash scripts/install-linux.sh
```

以上打包和安装脚本用于 Linux。打包生成 `dist/lexington/` 和 `dist/lexington-linux-<架构>.tar.gz`，包含可执行文件、资源和配置。解压后运行其中的 `lexington` 即可。

安装脚本默认安装到 `~/.local/share/lexington/`（遵循 `XDG_DATA_HOME`），创建名称固定为 `Lexington` 的应用菜单和桌面快捷方式（桌面文件为 `Lexington.desktop`），升级时迁移旧快捷方式并保留已安装版本的配置。分发前请检查配置中没有遗留的密钥；新配置不再保存聊天 API key。Codex CLI 需单独安装，程序不会自动下载。

Windows 在对应系统使用 `cargo build --release --locked` 构建，将 `target/release/lexington.exe` 与 `resource/`、`config/` 一起分发；macOS 对应的可执行文件为 `target/release/lexington`。

## 使用

- 宠物中心圆形区域：单击显示随机角色台词；第二次点击显示全文；第三次关闭。按住后移动超过 5px 拖动整个窗口。Linux X11/XWayland 允许透明边距移出屏幕，立绘中心可拖至屏幕边缘。
- 对话：每 50ms 显示一个字符，鼠标活动重置自动关闭计时；点击圆形区域和气泡以外的位置关闭。
- 输入框：鼠标移到立绘或输入区域时显示在立绘下方，点击后输入；输入时或正在生成时保持显示；鼠标移开且失去焦点后自动收起，草稿和错误会保留，移回后恢复。Enter 发送给 Codex，等待时显示“Lexington正在思考”，回复流式显示在气泡中；连接期间发送的第一条消息会等待会话就绪。任务进行中按 Escape 中断，否则清空输入和错误。请求失败显示错误。窗口启动时不自动聚焦输入框。
- 右键宠物：设置、关于、关闭。设置使用独立窗口，可按住标题栏拖到屏幕其他位置，关闭设置不会退出桌宠。
- 正常退出后保存窗口位置，下次启动恢复；普通与开发模式分别记录，窗口尺寸保持不变。屏幕布局变化导致立绘不可见时回到默认位置（原生 Wayland 由合成器管理位置）。
- X11/XWayland 下透明空白区域可点击穿透；立绘按当前帧轮廓接收鼠标，侧栏、输入框和打开的窗口保留交互。

| 侧栏按钮 | 功能 |
|---|---|
| 📌 / 📍 | 固定显示 / 悬停显示（退出后保存，下次启动恢复） |
| ↻ | 切换同服装的正常 / 战损立绘；战损时显示橙色，缺少对应版本时禁用 |
| ▶ | 在当前角色、当前格式、当前正常 / 战损队列中循环下一张 |
| G / P | GIF / PNG 切换，优先保留同服装和正常 / 战损状态；资源缺失时回退 |
| 👤 | 切换角色 |
| ⏰ | 上班打卡 / 取消；到时显示下班提醒 |

自动工作计时默认开启：程序运行时，登录、解锁或恢复键鼠操作后持续活跃 3 分钟，自动记录本次首次操作的时间。确认期间连续闲置满 1 分钟、锁屏、会话切换或检测中断会重新确认；短暂打开电脑查看消息不会直接打卡。默认不限工作日和时段，适用于固定工位；记录表示开始使用电脑，不代表实际到达工位，也不会追溯程序启动之前的操作。

自动打卡提示支持“修改时间 / 撤销”；右键 → 设置 → 打卡可关闭自动计时、输入当天 HH:MM、手动打卡或撤销。修改时间不能晚于现在。每天只自动记录一次，午休、重启、取消或撤销后当天不再自动开始，仍可手动开始。当天记录及下班提醒状态立即保存，重启后继续计时，次日本地日期重新开始。

Linux 通过 loginctl 判断本地活动会话和锁定状态，优先读取 GNOME Mutter 的系统空闲时间（需要 gdbus）；原生 X11 可回退到 XScreenSaver。XWayland 的空闲时间不能代表整个 Wayland 桌面，因此其他 Wayland 桌面无法读取时保留手动打卡。只读取空闲时长，不记录键盘内容。状态文件位于 `$XDG_DATA_HOME/lexington/clock.json`（默认 `~/.local/share/lexington/clock.json`），开发模式独立使用 `clock-dev.json`。文件损坏或不可写时显示错误，不覆盖旧记录。

右键 → 设置 → 闹钟：设置本地时间 `HH:MM` 和提示语，点击“添加闹钟”；支持多个闹钟、每天重复、编辑、启停和删除。未勾选重复时只提醒一次；时间已过则安排下次到点。闹钟响起时，在桌宠所在屏幕中央显示独立置顶窗口，提示语完整显示并支持滚动，点击“知道了”关闭；不会移动桌宠或自动消失。原生 Wayland 的窗口位置由桌面控制。

闹钟保存在 `$XDG_DATA_HOME/lexington/alarms.json`，开发模式为 `alarms-dev.json`，保存后即时生效。重启保留闹钟与未确认提醒；程序关闭或电脑休眠期间无法弹窗，恢复运行后会合并补提醒，每个错过的闹钟补一次。损坏或不可写记录会显示错误并保留原文件。

GIF 逐帧解码，缺少循环信息时默认持续循环；显式设置了循环次数的文件按该次数播放。播放器不会把全部 1024×1024 帧保留在内存；不足 20ms 的帧延迟按浏览器行为使用 100ms。未配置图片或图片解码失败时显示粉色动画宠物。

## 资源与配置

```text
resource/
  illustration/<角色>/gif/*.gif
  illustration/<角色>/png/*.png
  texts/<角色>/text.json
  background/dialog/*.{png,jpg,jpeg,gif,webp}
  background/input/*.{png,jpg,jpeg,gif,webp}
  icon/
  fonts/                  # 内置中文字体与许可证
config/config.json
```

台词格式为 JSON 字符串数组，空台词会过滤。角色从立绘目录发现；各资源按名称排序，背景使用第一张。

立绘文件按 `<服装名>_Normal.gif/png` 与 `<服装名>_Damage.gif/png` 对应，例如 `Normal_Normal.gif` 对应 `Normal_Damage.gif`。只识别文件名最后的 `_Normal` / `_Damage` 后缀（不区分大小写），前面的完整服装名用于配对；仅在同角色、同格式内切换，不会跳到其他服装的战损图。没有状态后缀的图片按正常立绘处理。

启动默认使用正常队列。GIF 的正常和战损队列各有 7 张，PNG 各有 8 张。切换角色或格式时优先保留正常 / 战损状态；目标格式缺少该状态时先尝试另一格式，两种格式都缺少该状态才回退到另一状态。

添加角色时，在 `resource/illustration/<角色名>/gif/` 或 `png/` 放入立绘，并可在 `resource/texts/<角色名>/text.json` 添加台词：

```json
["你好，提督！", "今天也一起加油吧。"]
```

配置文件每 0.5 秒自动热加载，无需重启程序；无效或临时写到一半的配置会保留上次有效设置，并在设置窗口提示。资源文件改动后，右键宠物打开“设置”，在“常规”页点击“重新加载”。安装版的资源和配置位于安装目录；源码运行时使用项目目录中的文件。

```json
{
  "dialog": {
    "width": 200,
    "maxHeight": 120,
    "offsetX": 150,
    "offsetY": 50,
    "autoCloseDelay": 10
  },
  "codex": {
    "executable": "codex",
    "workingDirectory": null,
    "noApproval": false
  },
  "clockIn": {
    "autoStart": true,
    "confirmationSeconds": 180,
    "workReminderHours": 12,
    "clockInMessage": "打卡成功！",
    "clockOutMessage": "下班时间到了！",
    "clockInCancelMessage": "打卡已终止"
  }
}
```

`width/maxHeight/offsetX/offsetY` 为逻辑像素，`autoCloseDelay` 为秒，`workReminderHours` 为小时。`clockIn.autoStart` 默认 true；`confirmationSeconds` 默认 180，限制在 30～600 秒，热加载改变这些设置后重新确认。输入框水平居中。缺失字段采用默认值，启动读取失败时回退默认配置，热加载错误保留上次有效配置。

聊天改为连接本机 Codex app-server，通过后台线程处理标准输入输出，不阻塞界面。沿用 Codex CLI 的登录、模型及权限配置，不再使用旧的 `llm` 字段；旧配置仍可加载，安装升级保留原配置。

`codex.executable` 可设为 CLI 的绝对路径；默认从 PATH 查找，Linux 桌面启动时还会查找 `~/.local/bin/codex` 和 `~/.cargo/bin/codex`。`codex.workingDirectory` 指定 Codex 操作的项目目录：绝对路径直接使用，相对路径以资源根目录为基准；`null` 默认使用资源根目录（安装版通常为 `~/.local/share/lexington`）。需要操作其他项目时应改为该项目的路径。

右键 → 设置 → 常规中的“无需审批”开关会自动保存到 `codex.noApproval`，默认关闭。开启后，从下一条消息起使用 `approvalPolicy: never`，关闭后恢复当前会话启动时继承的本机审批策略（包括本机原本就无需审批的情况）。切换不重启服务、不清空对话，也不改变当前任务或沙箱权限；超出沙箱允许范围的操作仍会失败。Codex 的补充信息提问仍需回答。

同一次启动内复用一个 Codex thread，支持连续对话；退出程序会终止所启动的 app-server（Unix 同时清理该进程组的后代），下次启动新建会话。右键 → 设置 → 连接可查看连接状态、程序路径和工作目录，路径支持悬停查看及复制；普通设置热加载时保留对话；程序路径或工作目录改变时自动重建 Codex 会话，正在处理任务则等任务结束后切换，会话上下文随重建清空。也可空闲时手动点击“重启会话”。命令或文件审批、Codex 提问会显示交互窗口；不支持的请求会明确返回错误。连接请求响应超时为 30 秒，模型生成不设置固定超时。

协议参考：[Codex app-server](https://developers.openai.com/codex/app-server/)。本机集成根据已安装 CLI 的帮助与生成的 JSON Schema 验证。

## 代码结构

| 目录 / 文件 | 职责 |
|---|---|
| `src/main.rs` | 应用入口与原生窗口启动参数 |
| `src/ui/` | 应用编排、宠物、侧栏、对话、Codex 状态与审批、独立设置窗口、闹钟设置及原生弹窗、输入、纹理与样式 |
| `src/backend/` | 配置、资源扫描、角色/立绘选择、打卡、闹钟调度与记录和 Codex 子进程/协议 |
| `src/tools/` | 资源根目录定位、图片/GIF 流式解码、平台窗口适配 |
| `scripts/` | Linux 安装、打包和 X11 窗口验证 |

完整目录、每个文件的职责、主要接口及依赖方向见 [模块职责说明](docs/MODULES.md)。后续代码变更须同步维护该文档，具体要求见 [AGENTS.md](AGENTS.md)。

## 验证

```bash
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo run --locked -- --dev --screenshot /tmp/lexington.png
```

`--screenshot` 默认等待 4 秒后保存透明 PNG 并退出，`--screenshot-delay 7` 可调整等待秒数。测试覆盖正常/战损配对与独立队列、缺失配对和格式回退、资源删除后的刷新恢复、配置、GIF 循环与局部帧还原、Codex 握手、流式回复、配置热加载/错误保护、审批开关保存及原策略恢复、审批、提问、中断与进程异常、UI 指针与键盘交互、闹钟调度/保存/重启/设置和确认、打卡计时和对话关闭。Codex 测试使用 Python 3 模拟子进程，不连接模型、不读取用户凭据；运行测试需安装 Python 3。真实 CLI 启动和握手需单独验证。

内置 Noto Sans CJK SC、Liberation 和 Noto Sans Math 字体，并附带许可证，减少字体缺失造成的字号、换行及按钮符号差异。立绘、气泡和输入框共同缩放与浮动；图片使用线性 mipmap 过滤，保留缩小后的平滑效果。

活动 Linux 桌面可进一步执行（会移动鼠标，需要 libX11 和 libXtst）：

```bash
python3 scripts/native-smoke.py target/release/lexington /tmp/lexington-drag.png drag
python3 scripts/native-smoke.py target/release/lexington /tmp/lexington-edges.png drag-edges --production
python3 scripts/native-smoke.py target/release/lexington /tmp/lexington-position.png position --production
python3 scripts/native-smoke.py target/release/lexington /tmp/lexington-click-through.png click-through --production
python3 scripts/native-smoke.py target/release/lexington /tmp/lexington-clock.png clock
python3 scripts/native-smoke.py target/release/lexington /tmp/lexington-input.png input
python3 scripts/native-smoke.py target/release/lexington /tmp/lexington-production.png idle --production
```

脚本验证窗口尺寸、置顶、隐藏任务栏和窗口切换器；`drag` 还断言 5px 阈值、精确位移、反向移动、离开初始命中区域和拖动后尺寸。`click-through` 检查透明空白处将点击传给真实底层窗口，立绘与侧栏仍接收点击。`drag-edges` 从可见立绘按下并检查四角、贴边释放与拖回，透明窗体允许部分出屏且保持尺寸。`position` 模式拖动后正常关闭并再次启动，检查位置及尺寸恢复。`pin` 模式切换侧栏显示模式，配合独立 `XDG_DATA_HOME` 可验证重启后的保存结果（Linux 状态文件为 `$XDG_DATA_HOME/lexington/app.ron`，默认位于 `~/.local/share/lexington/`）。测试宜使用 Xephyr + 窗口管理器等独立 X11 桌面，避免影响工作桌面。

已在 Linux X11 独立桌面验证上述窗口行为、500×500 普通窗口、800×600 开发窗口和侧栏状态保存，并对照原 Vue 渲染检查立绘、气泡、输入框和默认宠物。

## 许可证

项目代码沿用 MIT 许可。内置字体及表情图标的许可说明分别位于 `resource/fonts/LICENSE-*.txt` 和 `resource/icon/LICENSE-emoji.txt`。
