# 模块与功能

本文件仅列出项目模块与功能。接口约束、实现细节、平台避坑及验证记录见 [DEVELOPMENT_NOTES.md](DEVELOPMENT_NOTES.md)。

## 目录结构

```text
src/
├── main.rs
├── ui/
├── backend/
├── roleplay/
├── development/
└── tools/
    └── live2d/
integrations/
├── browser/
├── vscode/
└── nautilus/
scripts/
tests/fixtures/
resource/
config/
.agents/skills/unified-live2d/
docs/
├── MODULES.md
├── DEVELOPMENT_NOTES.md
└── agents/
```

## 启动入口

| 模块 | 功能 |
| --- | --- |
| `src/main.rs` | 解析启动参数，初始化原生应用，分派命令行和集成宿主入口，处理应用重启。 |

## 界面 `src/ui/`

| 模块 | 功能 |
| --- | --- |
| `mod.rs` | 声明界面子模块并导出应用入口。 |
| `app.rs` | 管理应用状态，编排主窗、资源、会话与子窗口。 |
| `pet.rs` | 绘制桌宠立绘、阴影和悬停效果，处理立绘交互。 |
| `sidebar.rs` | 绘制资源切换侧栏、按钮提示与动画。 |
| `dialog.rs` | 展示对话气泡、预设台词与打字机效果。 |
| `input.rs` | 管理主窗和开发窗口的消息输入、发送、中断、提问及审批。 |
| `completion.rs` | 提供斜杠指令候选与输入补全。 |
| `command_panel.rs` | 展示模型、推理强度及权限交互面板。 |
| `codex.rs` | 编排界面与 Codex 会话之间的请求、事件消费及任务状态。 |
| `settings.rs` | 展示设置窗口并提交配置操作。 |
| `connection.rs` | 展示继承的 Codex 连接来源，独立编辑并保留官方与第三方 API 配置草稿。 |
| `role_card.rs` | 编辑角色卡、世界书和规则。 |
| `new_character.rs` | 提供新建角色表单、立绘选择及导入交互。 |
| `roleplay_context.rs` | 展示角色上下文、发送清单与临时场景。 |
| `roleplay_memory.rs` | 编辑用户人设与固定记忆。 |
| `roleplay_vector.rs` | 编辑历史向量记忆设置并提供清空操作。 |
| `character_memories.rs` | 编排人物事件提炼，提供记忆浏览与删除界面。 |
| `development.rs` | 展示开发工作区、任务历史、执行记录和输入区。 |
| `development_diagnostics.rs` | 展示项目诊断及分析入口。 |
| `development_tests.rs` | 编辑测试配置，管理运行队列及结果展示。 |
| `development_review.rs` | 展示任务文件汇总和全文差异面板。 |
| `development_memory.rs` | 编辑、核对和管理项目记忆。 |
| `memo.rs` | 展示备忘录、筛选、编辑与完成操作。 |
| `schedule.rs` | 展示定时任务表单、运行状态、历史与通知。 |
| `alarm.rs` | 展示闹钟配置及到期提醒。 |
| `calendar.rs` | 展示月历、节日、加班记录与统计。 |
| `clock.rs` | 编排打卡、自动活动确认和下班提醒。 |
| `weather.rs` | 提供城市搜索、天气设置与天气展示。 |
| `assets.rs` | 管理立绘纹理、GIF 播放及 Live2D 渲染资源。 |
| `transparency.rs` | 处理主窗透明合成输出和截图像素转换。 |
| `style.rs` | 安装字体、主题和输入法样式。 |
| `tests.rs` | 验证界面组件与会话状态的协作行为。 |

## 后端 `src/backend/`

| 模块 | 功能 |
| --- | --- |
| `mod.rs` | 声明后端子模块与公共业务入口。 |
| `config.rs` | 读取、校验和保存应用配置，标识会话连接变更。 |
| `resources.rs` | 扫描角色立绘、换装、格式与战损资源。 |
| `character.rs` | 管理角色、立绘格式、换装和正常／战损选择。 |
| `codex.rs` | 发现本机或随包 Codex，管理进程、协议、线程、任务、审批和命令执行。 |
| `codex_config.rs` | 读取与保存本机 Codex 连接配置，识别官方或第三方提供方并支持凭据遮蔽。 |
| `commands.rs` | 定义客户端指令、模型选项和交互面板数据。 |
| `context.rs` | 将角色与项目上下文组织为请求发送清单。 |
| `jobs.rs` | 管理有界结构化后台任务及其生命周期。 |
| `handoff.rs` | 定义外部上下文请求、命令行交付及 Linux 本地 Inbox。 |
| `handoff_windows.rs` | 提供 Windows 本地 Inbox 与集成宿主交付。 |
| `weather.rs` | 查询城市和天气并管理请求、缓存与结果。 |
| `memo.rs` | 管理备忘录数据与持久化。 |
| `schedule.rs` | 管理定时任务、调度执行及运行历史。 |
| `alarm.rs` | 管理闹钟、到期状态与持久化。 |
| `calendar.rs` | 计算月历与节日，管理加班记录和统计。 |
| `clock.rs` | 管理打卡、工作时长和自动打卡状态。 |

## 角色扮演 `src/roleplay/`

| 模块 | 功能 |
| --- | --- |
| `mod.rs` | 导出角色卡、角色会话、上下文与记忆接口。 |
| `registry.rs` | 读取角色映射并定位角色资源。 |
| `card.rs` | 定义角色卡、规则与世界书，组装角色提示。 |
| `creation.rs` | 校验并创建个人角色卡与立绘资源。 |
| `store.rs` | 读取和保存用户角色卡覆盖。 |
| `context.rs` | 规划角色上下文预算、世界书匹配、最近历史与临时场景。 |
| `profile.rs` | 管理用户人设、固定记忆及角色会话配置。 |
| `vector_memory.rs` | 管理历史对话归档、向量召回与 Embedding 配置。 |
| `character_memory.rs` | 定义人物事件提炼、依据校验、存储和删除策略。 |

## 开发任务 `src/development/`

| 模块 | 功能 |
| --- | --- |
| `mod.rs` | 管理工作区、开发任务、多轮记录、开发指令和私人历史存储。 |
| `context.rs` | 定义上下文发送清单及项目背景选择。 |
| `journal.rs` | 记录与回放开发事件和检查点。 |
| `review.rs` | 汇总任务文件变更，保存文件快照并生成全文差异。 |
| `diagnostics.rs` | 管理项目诊断快照与来源信息。 |
| `testing.rs` | 定义测试配置、运行请求、结果和分析背景。 |
| `memory.rs` | 管理项目记忆、文件依据、确认状态和版本。 |

## 工具 `src/tools/`

| 模块 | 功能 |
| --- | --- |
| `mod.rs` | 声明通用工具与平台适配模块。 |
| `paths.rs` | 定位资源根目录与各类用户状态文件。 |
| `activity.rs` | 后台采样系统活动、空闲与会话状态。 |
| `file_dialog.rs` | 提供原生立绘文件和项目目录选择器。 |
| `gif.rs` | 解码 GIF、推进播放时序并管理帧索引缓存。 |
| `embedding.rs` | 加载本地 Embedding 模型，分块文本并生成向量。 |
| `vector_store.rs` | 提供通用向量及 JSON 记录存储。 |
| `platform.rs` | 提供跨平台窗口接口与 Linux 窗口、指针、穿透实现。 |
| `platform_windows.rs` | 实现 Windows 窗口定位、拖动、命中、桌面透明合成和透明区域穿透。 |
| `proxy.rs` | 将 Windows 系统手动代理传递给 CLI 子进程。 |
| `time.rs` | 提供跨平台本地时间与时间戳转换。 |
| `windows.rs` | 封装 Windows 句柄、用户身份、进程 Job 和空闲查询。 |
| `live2d/mod.rs` | 加载原生角色模型并管理动画轨道、采样和热区交互。 |
| `live2d/layered.rs` | 实现分层角色的部件动画、合成与命中。 |
| `live2d/mesh.rs` | 实现角色网格变形与连续蒙皮。 |
| `live2d/render.rs` | 实现原生角色像素渲染。 |

## 外部集成 `integrations/`

| 模块 | 功能 |
| --- | --- |
| `vscode/` | 提供选区、文件及项目诊断的编辑器交付入口。 |
| `browser/` | 提供 Firefox 与 Chromium 选区右键菜单及宿主通信。 |
| `nautilus/` | 提供文件管理器脚本和菜单扩展。 |

## 安装、打包与验证 `scripts/`

| 模块 | 功能 |
| --- | --- |
| `install-linux.sh` / `install-windows.ps1` | 构建并安装桌面应用、资源与快捷方式。 |
| `package-windows.ps1` / `package-windows.py` | 制作 Windows MSI 安装包及可选便携 ZIP。 |
| `windows_codex.py` | 获取、校验并准备 Windows 随包 Codex 原生发行文件。 |
| `package.sh` / `package_assets.py` | 制作普通 Linux 发布包并统一处理发布资源。 |
| `package-full.sh` / `package-full.py` | 制作包含系统依赖的 Linux 离线安装包。 |
| `package-deb.sh` / `package-deb.py` / `launch-deb.sh` | 制作 DEB 及离线套件，并提供系统安装后的启动入口。 |
| `dependencies.py` / `install-dependencies.sh` / `install-offline-dependencies.sh` | 检查并安装系统与 Codex 依赖。 |
| `integrations.py` | 构建与安装各应用集成客户端和通信宿主。 |
| `import-live2d.py` | 转换离线 Live2D 素材。 |
| `build-bikini-live2d-v2.py` / `build-flower-live2d.py` | 制作并发布对应换装的分层角色资源。 |
| `test-dependencies.py` / `test-package.sh` / `test-package-full.py` | 验证依赖安装与 Linux 打包流程。 |
| `test-windows.ps1` / `test-windows-msi.ps1` | 验证 Windows 构建、打包及 MSI 生命周期。 |
| `test-windows-codex.py` | 验证 Windows Codex 分发准备及完整性保护。 |
| `test-windows-transparency.py` | 验证 Windows 桌面像素透明合成及状态恢复。 |
| `windows-smoke.py` / `native-smoke.py` / `settings-smoke.py` | 验证原生窗口、交互与设置功能。 |
| `context-smoke.py` / `test-integrations.js` | 验证外部上下文交付与集成客户端。 |

## 资源、技能与文档

| 模块 | 功能 |
| --- | --- |
| `resource/` | 提供立绘、角色映射、角色卡、字体和本地模型等应用资源。 |
| `config/` | 保存应用设置、个人角色卡与角色记忆配置。 |
| `docs/agents/` | 保存角色设定的来源资料。 |
| `.agents/skills/unified-live2d/` | 提供统一角色素材制作、局部动作、预览与原生接入工作流。 |
| `tests/fixtures/codex-server.py` | 提供 Codex 协议模拟服务。 |
| `docs/MODULES.md` | 提供项目模块与功能索引。 |
| `docs/DEVELOPMENT_NOTES.md` | 汇总开发约束、实现细节、平台避坑与验证说明。 |
