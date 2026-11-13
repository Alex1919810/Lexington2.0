# Lexington

## 项目简介

Lexington 是一款使用 Rust 编写的桌面宠物，基于 eframe/egui + glow 绘制透明原生窗口，将角色陪伴、AI 对话和日常提醒放在桌面上。支持 Windows 和 Linux，运行桌宠无需 Node.js 或浏览器环境。

## 基本功能

- **角色陪伴**：GIF/PNG 立绘、侧栏 L 切换 Live2D、多角色与服装切换、随机台词和打字机气泡，可自定义气泡背景。在“设置 → 角色卡”新建角色，填写名称并导入至少一张 PNG/GIF 立绘，再编辑角色设定。Lexington 婚纱支持点击头部眨眼、点击花束抬花；Flower 采用 AI 补全分层，支持点击花束、手臂、草帽、蝴蝶飘带和独立蝴蝶，待机持续呼吸和头发摆动。
- **桌面交互**：置顶、拖动、透明区域点击穿透，退出后记住窗口位置；右键宠物打开设置或退出。
- **AI 对话**：接入 Codex，在“设置 → 角色卡”编辑人格、对话示例、世界书和规则。世界书支持最近对话匹配、持续与冷却；用户人设和经确认的记忆独立保存。“上下文”可检查输入、角色与项目背景的统一发送清单，核对来源、版本和排除原因，支持本地预览及临时场景。切换角色新建会话，回复流式显示，支持审批和中断。
- **输入指令**：主输入框和开发输入框支持 `/status`（会话状态、token 用量、账号短时/每周剩余额度与重置时间）、`/model`（打开模型与推理强度选择面板）、`/permissions`（查看权限并切换无需审批）、`/help`（帮助），这些指令不调用模型；模型选择只影响当前会话，从下一轮生效，不修改本机 Codex 配置，取消保留原设置。面板在发送指令的窗口内显示；`/status` 每次向 Codex 查询最新账号额度，不支持或失败时明确提示。输入 `/` 可筛选已支持指令，↑↓ 选择，Tab/前缀 Enter 或点击补全，Esc 收起；完整指令按一次 Enter 执行。尚未接入的 CLI 指令会明确提示，未知斜杠词、绝对路径和普通消息仍按原文发送。
- **历史记忆**：默认启用内置中文 Embedding 模型，在 CPU 上离线生成向量，用本地 SQLite 按角色保存成功对话并召回旧片段；可关闭、切换远程接口或清空单个角色的历史记忆。
- **人物记忆**：成功对话后，使用当前对话模型在后台额外提炼重要事件并自动保存。右键打开与“设置”并列的“人物记忆”，按时间查看、搜索、筛选和删除，可展开核对参与者、事件时间与原文依据。提炼结果不出现在聊天输出中。
- **development**：右键宠物选择“开发”打开独立任务窗口，通过“选择项目…”打开文件浏览器选择项目文件夹（也可手动输入路径），按项目管理任务、继续历史会话，查看执行命令、输出、退出码、计划和文件 diff；每轮可查看发送清单、事件时间线，并离线回放核对记录（不会重新执行命令）。支持多行输入、代码复制及文件跳转；项目与历史记录保存在用户数据目录，普通运行与 `--dev` 隔离。在“设置 → 连接”开启 `development 模式` 后才注入开发指令；默认关闭，切换等待当前任务结束再新建会话。
- **开发上下文**：VS Code 可发送项目诊断；开发窗口的“诊断 / 测试 / 项目记忆”页支持问题定位、按项目配置并运行测试、查看退出码与失败记录，以及保存经核对的项目知识。项目记忆可绑定文件依据，文件变化后需重新核对；诊断和项目记忆等背景只在 development 模式中附加。
- **右键发送**：通过文件管理器、VS Code 或浏览器的“看看Lexington怎么说”发送文件或选区；各应用需安装对应集成，见[集成安装说明](integrations/README.md)。
- **备忘录**：右键桌宠选择“备忘录”，新增待办事项、勾选完成并划线、取消勾选恢复或彻底删除。支持全部/待办/已完成筛选，修改自动保存，重启后保留。
- **定时任务**：右键桌宠选择“定时任务”，设置单次、每天或固定间隔的提示词，时间通过时/分控件选择并自动补齐格式，按保存的工作区、角色与模式在独立 Codex 会话中后台执行，无需审批，不占用当前聊天。完成后主界面显示未读结果；“执行历史”保留最近 200 次的时间、结果和回复，跨重启保留。Lexington 需保持运行，关闭期间错过的多次执行在下次启动合并补发一次。
- **日历与加班**：右键桌宠选择“日历”，在月历下点击“加班打卡”，自动用当前时间减去预计下班时间（当天上班打卡＋配置的工作时长），将加班时长记入日期格；同一天再次点击更新记录。可切换月份查看加班总时长和天数；选中日期后点“手动填写”，输入小时和分钟并保存，可补录历史或修改当天总时长（每日 0～24 小时），无需上班打卡。支持查看详情或删除，记录跨重启保留。
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

### 制作 Linux 全依赖离线安装包

在与目标机相同发行版版本、相同架构的 Debian/Ubuntu 制作机上运行：

```bash
bash scripts/package-full.sh
# 可选：指定输出目录
bash scripts/package-full.sh --output-dir /path/to/output
# 可选：指定 Codex 稳定版，版本不得低于 0.166.0
bash scripts/package-full.sh --codex-version 0.166.0
```

制作机需有 Rust、Python 3.10+、APT/dpkg、已更新的软件源索引和网络。脚本构建 release，下载完整系统运行依赖及制作时最新稳定版的官方原生 Codex（最低 `0.166.0`），连同立绘、中文 Embedding 模型和右键集成打入 `dist/lexington-full-<发行版>-<版本>-<架构>.tar.gz`。支持 amd64/arm64；制作时不安装系统软件，不打包个人配置、密钥、角色卡或登录数据。失败时保留上一次成功的同名安装包。原 `scripts/package.sh` 仍用于需要联网补依赖的普通包。

所有 Linux 发布入口均使用 release 主程序，并通过制作机的 binutils `strip` 去除发布副本中的符号；不修改 `target/release` 构建产物。资源排除制作草稿 `live2d-drafts` 和 Windows 运行库，保留正式立绘、Live2D、模型及许可。复用旧完整包生成 DEB 时也会重新过滤和去除符号，源码资源保持完整。

目标机解压后执行 `bash install.sh`，即可离线补齐运行依赖、安装程序并创建快捷方式；无需 Rust、Node 或预装 Python，系统依赖安装需管理员认证。可先用 `bash scripts/install-offline-dependencies.sh --check` 核对平台和 SHA-256。已有符合最低版本要求的 Codex、登录及应用配置会保留；旧版 Codex 会提示先升级。完整包始终包含可选桌面集成依赖。

离线包只适用于相同发行版版本和架构；目标机需有 Bash、coreutils、APT/dpkg 及可用的图形桌面。包内包含 Mesa 通用驱动，内核、桌面环境和专有显卡驱动由目标机提供。安装过程中禁止下载依赖或移除系统包；本机较新的包导致版本冲突时会中止，应在匹配的系统上重新制作包。AI 对话仍需另行配置登录/API 和模型网络连接。

### 制作 DEB 程序包和配套离线套件

```bash
bash scripts/package-deb.sh
# 复用已有完整包，不重新构建或下载
bash scripts/package-deb.sh --from-bundle dist/lexington-full-ubuntu-24.04-amd64.tar.gz
```

输出 `dist/lexington_<版本及构建修订>_<架构>.deb` 和 `dist/lexington-deb-offline-<发行版>-<版本>-<架构>.tar.gz`。支持 `--output-dir`；全新制作时也可用 `--codex-version` 指定不低于 0.166.0 的稳定版。

单独的 `.deb` 包含程序、资源、模型和原生 Codex，系统依赖由标准 `Depends` 字段声明；可用软件安装器打开，或执行 `sudo apt install ./路径/lexington_版本_架构.deb`，缺失的系统依赖由 APT 联网补齐。离线安装则解压配套套件，运行 `bash install.sh`，一次将程序和全部依赖交给 APT；`bash install.sh --check` 可先校验。不要只用 `dpkg -i` 安装程序包并期待它下载依赖。

安装后从应用菜单打开 **Lexington**，或运行 `lexington`。系统文件位于 `/usr/lib/lexington`；首次启动及包内容更新后，以当前普通用户同步到原有的个人安装目录并创建桌面快捷方式、注册右键集成，保留已有配置和登录。无需以 root 启动 GUI；dpkg 安装脚本不写入用户目录，也不嵌套调用 APT。DEB 要求相同发行版版本和架构。卸载系统 DEB 时保留个人安装及数据，个人快捷方式需自行清理。

### 首次使用 AI 对话

AI 对话需要原生 Codex CLI；Linux 安装器可在缺失时自动安装。模型、API、凭据与权限统一继承环境及本机 Codex 配置（遵循 `CODEX_HOME`），Lexington 不再提供连接参数表单，也不使用 `config.json` 中遗留的连接参数覆盖 Codex。如需指定原生程序路径，设置环境变量 `LEXINGTON_CODEX_EXECUTABLE`（Windows 使用 `codex.exe`，不使用 `.cmd/.bat`）。使用前先完成 Codex 登录或外部 API 配置；未配置 AI 时，角色展示、台词和提醒仍可使用。

Windows 会在没有代理环境变量时使用系统手动代理；代理变更后，在“设置 → 连接”重启会话。

历史向量记忆默认启用，`bge-small-zh-v1.5` INT8 模型随安装包提供，无需 GPU、地址或 Key。在“设置 → 角色卡 → 长期记忆”可关闭或调整 CPU 线程数；已有明确关闭的配置会保留。也可选择远程接口，填写兼容 `/embeddings` 的基础地址、模型及需要时的 Key 环境变量名称，远程服务会收到待处理文本。AI 对话仍需前述 Codex 配置。只记录启用后的成功对话，已有旧会话不会自动导入；数据库保存在安装目录的 `config/roleplay-vectors.sqlite3`，升级保留。

只想从源码直接启动时，准备好 Rust 和平台构建依赖后运行：

```bash
cargo run --release --locked
```
