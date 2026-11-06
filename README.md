# Lexington — Rust 桌面宠物

一个使用 Rust 编写界面和业务逻辑的桌面宠物。eframe/egui + glow 直接绘制透明原生窗口，运行时无需 Node.js、Electron、WebView 或 N-API。

支持 GIF/PNG 立绘、多角色切换、随机台词、打字机对话、大模型聊天、打卡提醒、自定义气泡背景和窗口拖动。沿用原有 `resource/` 目录和 `config/config.json` 格式。

## 快速开始

安装 Rust 稳定版（支持 edition 2024），在项目目录运行：

```bash
cargo run --release --locked
```

默认窗口为 500×500、透明、无边框、置顶。开发窗口与原版相同，为 800×600，位置 (100,100)：

```bash
cargo run --locked -- --dev
```

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

安装脚本默认安装到 `~/.local/share/lexington/`（遵循 `XDG_DATA_HOME`），创建名称固定为 `Lexington` 的应用菜单和桌面快捷方式（桌面文件为 `Lexington.desktop`），升级时迁移旧快捷方式并保留已安装版本的配置。发布前请确保配置中的 `llm.apiKey` 为空。

Windows 在对应系统使用 `cargo build --release --locked` 构建，将 `target/release/lexington.exe` 与 `resource/`、`config/` 一起分发；macOS 对应的可执行文件为 `target/release/lexington`。

## 使用

- 宠物中心圆形区域：单击显示随机角色台词；第二次点击显示全文；第三次关闭。按住后移动超过 5px 拖动整个窗口。
- 对话：每 50ms 显示一个字符，鼠标活动重置自动关闭计时；点击圆形区域和气泡以外的位置关闭。
- 输入框：在立绘下方悬停显示，点击后输入；Enter 发送，显示“正在思考...”，回复通过气泡展示。请求失败显示错误，Escape 清空输入和错误。窗口启动时不自动聚焦输入框。
- 右键宠物：设置、关于、关闭。

| 侧栏按钮 | 功能 |
|---|---|
| 📌 / 📍 | 固定显示 / 悬停显示（退出后保存，下次启动恢复） |
| ↻ | 重扫立绘、台词、背景和配置，保留当前图片 |
| ▶ | 切换当前角色、当前格式的下一张立绘 |
| G / P | GIF / PNG 切换；格式不存在时回退 |
| 👤 | 切换角色 |
| ⏰ | 上班打卡 / 取消；到时显示下班提醒 |

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

添加角色时，在 `resource/illustration/<角色名>/gif/` 或 `png/` 放入立绘，并可在 `resource/texts/<角色名>/text.json` 添加台词：

```json
["你好，提督！", "今天也一起加油吧。"]
```

修改资源或配置后，点击侧栏 ↻ 重新加载。安装版的资源和配置位于安装目录；源码运行时使用项目目录中的文件。

```json
{
  "dialog": {
    "width": 200,
    "maxHeight": 120,
    "offsetX": 150,
    "offsetY": 50,
    "autoCloseDelay": 10
  },
  "llm": {
    "apiUrl": "https://api.deepseek.com/chat/completions",
    "apiKey": "",
    "model": "deepseek-chat"
  },
  "clockIn": {
    "workReminderHours": 12,
    "clockInMessage": "打卡成功！",
    "clockOutMessage": "下班时间到了！",
    "clockInCancelMessage": "打卡已终止"
  }
}
```

`width/maxHeight/offsetX/offsetY` 为逻辑像素，`autoCloseDelay` 为秒，`workReminderHours` 为小时。输入框水平居中。缺失字段采用默认值，格式错误时回退默认配置。

聊天需填写 `llm.apiUrl`、`llm.apiKey` 和 `llm.model`。接口使用 OpenAI Chat Completions 兼容格式，仅发送当前一条用户消息，60 秒超时；网络请求在后台执行，不阻塞界面。

## 代码结构

| 文件 | 职责 |
|---|---|
| `src/main.rs` | 应用入口、界面、交互状态和后台请求通道 |
| `src/visual.rs` | 默认宠物、容器变换与气泡形状 |
| `src/assets.rs` | 静态图片及 GIF 流式解码 |
| `src/backend.rs` | 配置读取、资源扫描和 HTTP 请求 |
| `src/platform.rs` | Linux 窗口属性及拖动 |
| `scripts/` | Linux 安装、打包和 X11 窗口验证 |

## 验证

```bash
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo run --locked -- --dev --screenshot /tmp/lexington.png
```

`--screenshot` 默认等待 4 秒后保存透明 PNG 并退出，`--screenshot-delay 7` 可调整等待秒数。测试覆盖资源删除后的刷新恢复、配置、GIF 循环与局部帧还原、HTTP 协议与失败响应、UI 指针与键盘交互、打卡计时和对话关闭。HTTP 测试使用本地模拟服务，不需要真实 API key。

内置 Noto Sans CJK SC、Liberation 和 Noto Sans Math 字体，并附带许可证，减少字体缺失造成的字号、换行及按钮符号差异。立绘、气泡和输入框共同缩放与浮动；图片使用线性 mipmap 过滤，保留缩小后的平滑效果。

活动 Linux 桌面可进一步执行（会移动鼠标，需要 libX11 和 libXtst）：

```bash
python3 scripts/native-smoke.py target/release/lexington /tmp/lexington-drag.png drag
python3 scripts/native-smoke.py target/release/lexington /tmp/lexington-clock.png clock
python3 scripts/native-smoke.py target/release/lexington /tmp/lexington-input.png input
python3 scripts/native-smoke.py target/release/lexington /tmp/lexington-production.png idle --production
```

脚本验证窗口尺寸、置顶、隐藏任务栏和窗口切换器；`drag` 还断言 5px 阈值、精确位移、反向移动、离开初始命中区域和拖动后尺寸。`pin` 模式切换侧栏显示模式，配合独立 `XDG_DATA_HOME` 可验证重启后的保存结果（Linux 状态文件为 `$XDG_DATA_HOME/lexington/app.ron`，默认位于 `~/.local/share/lexington/`）。测试宜使用 Xephyr + 窗口管理器等独立 X11 桌面，避免影响工作桌面。

已在 Linux X11 独立桌面验证上述窗口行为、500×500 普通窗口、800×600 开发窗口和侧栏状态保存，并对照原 Vue 渲染检查立绘、气泡、输入框和默认宠物。

## 许可证

项目代码沿用 MIT 许可。内置字体及表情图标的许可说明分别位于 `resource/fonts/LICENSE-*.txt` 和 `resource/icon/LICENSE-emoji.txt`。
