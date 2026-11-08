# 脚本约定

继承根目录 [AGENTS.md](../AGENTS.md)；此处仅补充本目录约束。模块职责、接口及详细场景见 [docs/MODULES.md](../docs/MODULES.md)。

- 原生指针测试需 libX11/libXtst，优先 Xephyr + WM 隔离，同一 display 必须串行；测试从可见轮廓按下，失败也须释放鼠标并恢复位置。模拟 egui 事件不能验证实际 WM 层级、移动或穿透。
- 打包 `bash scripts/package.sh`，安装 `bash scripts/install-linux.sh`；安装先检查依赖，缺 Codex 下载固定官方原生包并核验 SHA-512、安全解包、保留完整布局，无 Node/npm；已有 CLI/登录保留，不自动登录；可选依赖取消仍保留脚本菜单，必需依赖失败终止。
- 菜单和桌面显示名固定 Lexington，桌面文件 Lexington.desktop，迁移旧 lexington.desktop 避免重复；菜单 ID 保持 lexington.desktop。用 xdg-user-dir DESKTOP 定位桌面，设置可执行并尝试标记可信，核对快捷方式指向安装版。

## 增量验证

- Windows 安装/打包使用 install-windows.ps1/package-windows.ps1；PowerShell 5 调用 File.Replace 的空备份参数用 [NullString]::Value，不能用 $null。ZIP 配置必须为空对象，不得复制源码目录的密钥；解压包的基本安装不依赖 Rust/Python。
- Windows 原生测试用 windows-smoke.py，需明确授权、同桌面串行；使用隔离配置/记录和不可用的 Codex 路径，退出恢复鼠标和焦点，仅回收测试创建的进程。GIF 画布中心未必有 alpha，抓取点必须选持续可见的实体区域。

- 仅运行改动涉及的脚本：依赖安装用 `python3 scripts/test-dependencies.py`；集成客户端用 `node scripts/test-integrations.js`；CLI/桥接用 `python3 scripts/context-smoke.py target/release/lexington`；设置用 settings-smoke；平台窗口用相关 native-smoke 模式。命令和场景详见模块文档。
- 夹具不调用模型、不安装真实依赖；安装验收仍遵守根 AGENTS 的 release、用户配置保留及安装版核对要求。
- 发布包配置使用空对象，不打包 config/roleplay.json、config/roleplay-memory.json 或 roleplay-vectors.sqlite3 及 journal/wal/shm；重复 Linux 打包还须清除旧生成目录遗留的私人配置与数据库。`bash scripts/test-package.sh` 使用假构建器和临时资源检查此行为，不访问真实配置。
