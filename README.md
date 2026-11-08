# Lexington

## 项目简介

Lexington 是一款使用 Rust 编写的桌面宠物，基于 eframe/egui + glow 绘制透明原生窗口，将角色陪伴、AI 对话和日常提醒放在桌面上。支持 Windows 和 Linux，运行桌宠无需 Node.js 或浏览器环境。

## 基本功能

- **角色陪伴**：GIF/PNG 立绘、多角色与服装切换、随机台词和打字机气泡，可自定义气泡背景。
- **桌面交互**：置顶、拖动、透明区域点击穿透，退出后记住窗口位置；右键宠物打开设置或退出。
- **AI 对话**：接入 Codex，在宠物下方输入消息并按 Enter 发送，回复流式显示，支持任务审批和中断。
- **右键发送**：通过文件管理器、VS Code 或浏览器的“看看Lexington怎么说”发送文件或选区；各应用需安装对应集成，见[集成安装说明](integrations/README.md)。
- **日常提醒**：自动或手动打卡、下班提醒、三天天气预报，以及可编辑、每天重复的自定义闹钟。

## 快速安装与启动

以下命令均在本项目根目录执行。

### Windows

源码安装需先准备 Rust stable（MSVC 工具链）、Visual Studio C++ Build Tools（含 Windows SDK）和 Python 3（构建右键集成）。在 PowerShell 运行：

```powershell
powershell -ExecutionPolicy Bypass -File scripts/install-windows.ps1
```

安装器会构建 release，安装到 `%LOCALAPPDATA%\lexington` 并创建桌面和开始菜单快捷方式。安装完成后双击 **Lexington**，或运行：

```powershell
& "$env:LOCALAPPDATA\lexington\lexington.exe"
```

已有 Windows 发布 ZIP 包时，解压后执行同一安装命令即可，无需 Rust 或 Python。再次安装会保留已有配置。

### Linux

在有 OpenGL/EGL 驱动的 X11/XWayland 桌面中运行：

```bash
bash scripts/install-linux.sh
```

安装器会检查依赖、构建并安装到 `~/.local/share/lexington`（遵循 `XDG_DATA_HOME`），创建应用菜单和桌面快捷方式，并保留已有配置。首次补齐依赖需要联网，系统依赖可能需要管理员认证。

安装后打开应用菜单中的 **Lexington**，或运行：

```bash
"${XDG_DATA_HOME:-$HOME/.local/share}/lexington/lexington"
```

已有 Linux 发布包时，解压后运行 `bash install.sh`，无需 Rust。原生 Wayland 可在启动命令后添加 `--wayland`，窗口位置和层级由桌面合成器控制。

### 首次使用 AI 对话

AI 对话需要原生 Codex CLI；Linux 安装器可在缺失时自动安装，Windows 可在宠物的“设置 → 连接”中指定已有 `codex.exe` 的完整路径。使用本机 Codex 服务前先完成登录，或在设置中配置自定义模型 API。未配置 AI 时，角色展示、台词和提醒仍可使用。

只想从源码直接启动时，准备好 Rust 和平台构建依赖后运行：

```bash
cargo run --release --locked
```
