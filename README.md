# Lexington

## 项目简介

Lexington 是一款使用 Rust 编写的桌面宠物，基于 eframe/egui + glow 绘制透明原生窗口，将角色陪伴、AI 对话和日常提醒放在桌面上。支持 Windows 和 Linux，运行桌宠无需 Node.js 或浏览器环境。

## 基本功能

- **角色陪伴**：GIF/PNG 立绘、侧栏 L 切换 Live2D、多角色与服装切换、随机台词和打字机气泡，可自定义气泡背景。在“设置 → 角色卡”新建角色，填写名称并导入至少一张 PNG/GIF 立绘，再编辑角色设定。Lexington 婚纱支持点击头部眨眼、点击花束抬花；Flower 采用 AI 补全分层，支持点击花束、手臂、草帽、蝴蝶飘带和独立蝴蝶，待机持续呼吸和头发摆动。
- **桌面交互**：置顶、拖动、透明区域点击穿透，退出后记住窗口位置；右键宠物打开设置或退出。
- **AI 对话**：接入 Codex，在“设置 → 角色卡”编辑人格、对话示例、世界书和规则。世界书支持最近对话匹配、持续与冷却；用户人设和经确认的记忆独立保存。“上下文”可检查实际加载内容、本地预览及设置临时场景。切换角色新建会话，回复流式显示，支持审批和中断。
- **历史记忆**：默认启用内置中文 Embedding 模型，在 CPU 上离线生成向量，用本地 SQLite 按角色保存成功对话并召回旧片段；可关闭、切换远程接口或清空单个角色的历史记忆。
- **人物记忆**：成功对话后，使用当前对话模型在后台额外提炼重要事件并自动保存。右键打开与“设置”并列的“人物记忆”，按时间查看、搜索、筛选和删除，可展开核对参与者、事件时间与原文依据。提炼结果不出现在聊天输出中。
- **development**：右键宠物选择“开发”打开独立任务窗口，通过“选择项目…”打开文件浏览器选择项目文件夹（也可手动输入路径），按项目管理任务、继续历史会话，查看执行命令、输出、退出码、计划和文件 diff。支持多行输入、代码复制及文件跳转；项目与历史记录保存在用户数据目录，普通运行与 `--dev` 隔离。在“设置 → 连接”开启 `development 模式` 后才注入开发指令；默认关闭，切换等待当前任务结束再新建会话。
- **开发上下文**：VS Code 可发送项目诊断；开发窗口的“诊断 / 测试 / 项目记忆”页支持问题定位、按项目配置并运行测试、查看退出码与失败记录，以及保存经核对的项目知识。项目记忆可绑定文件依据，文件变化后需重新核对；诊断和项目记忆等背景只在 development 模式中附加。
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

AI 对话需要原生 Codex CLI；Linux 安装器可在缺失时自动安装。模型、API、凭据与权限统一继承环境及本机 Codex 配置（遵循 `CODEX_HOME`），Lexington 不再提供连接参数表单，也不使用 `config.json` 中遗留的连接参数覆盖 Codex。如需指定原生程序路径，设置环境变量 `LEXINGTON_CODEX_EXECUTABLE`（Windows 使用 `codex.exe`，不使用 `.cmd/.bat`）。使用前先完成 Codex 登录或外部 API 配置；未配置 AI 时，角色展示、台词和提醒仍可使用。

Windows 会在没有代理环境变量时使用系统手动代理；代理变更后，在“设置 → 连接”重启会话。

历史向量记忆默认启用，`bge-small-zh-v1.5` INT8 模型随安装包提供，无需 GPU、地址或 Key。在“设置 → 角色卡 → 长期记忆”可关闭或调整 CPU 线程数；已有明确关闭的配置会保留。也可选择远程接口，填写兼容 `/embeddings` 的基础地址、模型及需要时的 Key 环境变量名称，远程服务会收到待处理文本。AI 对话仍需前述 Codex 配置。只记录启用后的成功对话，已有旧会话不会自动导入；数据库保存在安装目录的 `config/roleplay-vectors.sqlite3`，升级保留。

只想从源码直接启动时，准备好 Rust 和平台构建依赖后运行：

```bash
cargo run --release --locked
```
