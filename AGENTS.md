# 项目约定

- 本项目采用纯 Rust 原生 GUI：eframe/egui + glow；禁止引入 Vue、Electron、Node、WebView 或 N-API 作为运行链。`Cargo.lock` 随代码保留；集成客户端的 JS 仅在各自应用宿主内运行。
- `src/ui/` 负责界面与交互，`src/backend/` 负责业务和 Codex 会话，平级的 `src/roleplay/` 负责角色卡、角色提示、世界书和角色配置，`src/development/` 负责工作区、开发任务、执行记录与开发指令，`src/tools/` 负责路径、解码和平台适配，`src/main.rs` 仅负责启动。后端不依赖 UI/egui；角色扮演模块不依赖后端/UI，也不启动 Codex；解码器只产出像素，UI 持有 GPU 纹理；仅平台工具允许依赖 eframe。模块与功能见 [docs/MODULES.md](docs/MODULES.md)；接口约束、实现细节、平台避坑与验证见 [docs/DEVELOPMENT_NOTES.md](docs/DEVELOPMENT_NOTES.md)。修改相关模块前阅读两份文档的对应条目及适用目录的 AGENTS.md。
- **任何代码文件改动都必须同步检查 `docs/MODULES.md` 和 `docs/DEVELOPMENT_NOTES.md`：前者仅维护模块与功能描述，后者维护接口、依赖、行为约束、实现细节、平台避坑和验证说明；仅调整内部实现时也须更新后者对应条目。** 文件增删、移动或重命名须同步模块索引与目录树；README 结构概览变化时同步更新。代码任务结束前将新增且验证过的可复用知识写入最接近所属模块的 AGENTS.md，不要求写入根文件；只有跨模块通用规则写入根 AGENTS.md。更新前阅读目标文件并合并去重，无新增知识无需添加，不记录临时进度；实现细节和测试场景清单统一写入 `docs/DEVELOPMENT_NOTES.md`，不得回填到 `docs/MODULES.md`。
- 未经用户明确授权，不执行 `git commit` 或 `git push`；提交与推送分别授权。每次推送前检查远端是否已有目标分支，一般推送授权不包含创建远端新分支。未经明确允许，不修改 makefile（含大小写变体）或 `include/` 下子模块头文件。
- 安装包及发布归档默认仅保留本地，不加入 Git、不提交、不推送，也不上传远端 Release；只有用户明确要求提交、推送或上传相应安装产物时才执行对应操作。一般的代码提交、推送授权不包含安装包；随源码维护的正式运行资源不属于安装包。
- 保持 `resource/` 布局和 `config/config.json` 的 camelCase 兼容。资源根按 `LEXINGTON_HOME` → 可执行文件旁 → 当前目录寻找。安装保留已有配置、密钥和登录；日志、Debug、错误及测试结果不得泄露凭据。

- `live2d-drafts/` 是本地临时制作目录，不加入 Git；正式角色仅提交 `live2d/<换装>/` 内 actor.json 引用的资源及说明，构建和常规测试不得依赖临时草稿。

## 目录规则

- 根规则适用于整个项目；进入目录修改前读取对应 AGENTS.md，跨目录任务同时遵守相关规则。子目录只维护局部约束，不覆盖根目录的授权、密钥保护、验证与交付要求。
- [后端](src/backend/AGENTS.md)：配置、Codex 协议、角色、计时/闹钟、外部 Inbox。
- [界面](src/ui/AGENTS.md)：输入/焦点、设置与提醒、纹理交互及 UI 回归。
- [角色扮演](src/roleplay/AGENTS.md)：角色卡、世界书、提示与用户角色配置。
- [Development](src/development/AGENTS.md)：工作区、任务历史、开发指令与隔离。
- [工具](src/tools/AGENTS.md)：GIF、X11、活动采样和平台限制。
- [脚本](scripts/AGENTS.md)：安装/打包、快捷方式、专项测试环境。
- [项目技能](.agents/skills/AGENTS.md)：开发用技能独立存放；[unified-live2d](.agents/skills/unified-live2d/SKILL.md) 用于统一角色素材制作、局部动作、预览与原生接入。

## 验证与交付

- **默认只做增量测试和受影响模块的测试；只有用户明确要求全量测试时才运行全量测试。** 根据本次改动和依赖关系选择用例/模块过滤器，跨层变化补测直接相关链路，不能因任务较长或临近结束自动扩大到全项目；受影响测试通过且无新问题后不重复执行。
- Rust 改动运行 `cargo fmt --check`、`cargo clippy --locked --all-targets -- -D warnings`，测试使用 `cargo test --locked <模块路径或用例名>`；未指定过滤器的 `cargo test --locked` 仅限用户明确要求全量时。专项 smoke/像素回归按影响范围选择；纯文档改动只检查内容、链接和差异，不运行 Rust 测试。具体入口见 [开发说明](docs/DEVELOPMENT_NOTES.md#验证入口)。
- 测试使用临时配置/夹具，不访问真实模型、凭据或用户记录；按层分工避免重复覆盖。模拟 UI 事件不能替代实际 WM、拖动或穿透验证；原生指针测试环境遵守脚本目录规则。
- **每次完成项目修改后必须运行 `bash scripts/install-linux.sh` 构建最新 release 并同步桌面安装版。** 该流程已有持续授权，无需再次询问；保留配置/API key，二进制通过同目录临时文件原子替换，不能 cp 截断运行中文件。核对安装版与 target/release/lexington 一致；运行的旧版本须安全重启验证，不强制关闭文件管理器/浏览器/编辑器。

## 外部集成

- 右键统一命名“看看Lexington怎么说”：文件管理器发本地绝对路径，VS Code 发选区/路径/起始行，浏览器发选区及 page/frame URL，只有 file URL 转本地路径；未保存/远程文档不得伪造本机路径。
- 各应用菜单分别扩展，Nautilus 脚本默认入口，顶层菜单需 python3-nautilus；Firefox 持久安装需签名，禁止修改浏览器安全策略免签；Chromium key/扩展 ID 与宿主白名单保持匹配。Snap/Flatpak 桥接需单独实测，不宣称任意软件全局菜单或默认浏览器已端到端可用。
