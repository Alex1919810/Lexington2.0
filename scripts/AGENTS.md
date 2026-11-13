# 脚本约定

继承根目录 [AGENTS.md](../AGENTS.md)；此处仅补充本目录约束。模块职责、接口及详细场景见 [docs/MODULES.md](../docs/MODULES.md)。

- 原生指针测试需 libX11/libXtst，优先 Xephyr + WM 隔离，同一 display 必须串行；测试从可见轮廓按下，失败也须释放鼠标并恢复位置。模拟 egui 事件不能验证实际 WM 层级、移动或穿透。
- Linux 立绘选择器使用 libdbus/XDG Portal，无 FileChooser 后端时回退 Zenity；源码和无源码安装均检查 libdbus-1-3、Zenity 及本地推理所需 libstdc++6，不能归入可跳过的菜单扩展依赖。原生立绘测试用 --pet-point 指定当前素材持续可见的实体点，不能沿用旧 GIF 的抓取坐标验证 Live2D。
- 打包 `bash scripts/package.sh`，安装 `bash scripts/install-linux.sh`；安装先检查依赖，缺 Codex 下载官方 latest 稳定版（最低 0.166.0）原生包并核验 SHA-512、安全解包、保留完整布局，无 Node/npm；已有满足最低版本的 CLI/登录保留，旧版提示升级，不自动登录；可选依赖取消仍保留脚本菜单，必需依赖失败终止。
- Linux 全依赖离线包使用 `bash scripts/package-full.sh`，仅面向制作机相同 Debian/Ubuntu 版本及架构；APT 必须以空 dpkg status 下载完整运行依赖闭包，不能依赖制作机已安装的软件。离线引导先校验平台及文件，再用禁用在线索引的 --no-download/--no-remove 安装本地 DEB；Python 随包引导，Codex 从随包官方归档安装，不复制本机登录或 CLI。增量夹具入口为 `python3 scripts/test-package-full.py`。
- Linux 普通包、完整包及 DEB 使用 package_assets.py 的统一过滤/strip：排除资源树任意层级 live2d-drafts 和 runtime/windows-*，重复输出也清理遗留；--from-bundle 复用旧包仍须过滤。主程序来自 locked release，只对发布副本执行 strip --strip-all，不改构建产物或源码资源；release 默认并不代表符号表已去除，验收用 readelf 检查发布文件。
- 标准 DEB 使用 `bash scripts/package-deb.sh`，`--from-bundle` 可离线复用完整包；系统依赖用 Depends 声明并由配套离线套件一次交给 APT，禁止在 dpkg 维护脚本中嵌套 apt 或写用户目录。GUI 由普通用户通过 /usr/bin/lexington 启动，按载荷身份复用 --system-package 分支同步个人安装；该分支不得构建、写系统资源树或补装系统依赖，用户快捷方式须调用系统启动器才能检测后续包更新。
- 菜单和桌面显示名固定 Lexington，桌面文件 Lexington.desktop，迁移旧 lexington.desktop 避免重复；菜单 ID 保持 lexington.desktop。用 xdg-user-dir DESKTOP 定位桌面，设置可执行并尝试标记可信，核对快捷方式指向安装版。

## 增量验证

- Windows 安装/打包使用 install-windows.ps1/package-windows.ps1；PowerShell 5 调用 File.Replace 的空备份参数用 [NullString]::Value，不能用 $null。ZIP 配置必须为空对象，不得复制源码目录的密钥；解压包的基本安装不依赖 Rust/Python。
- Windows 原生测试用 windows-smoke.py，需明确授权、同桌面串行；使用隔离配置/记录和不可用的 Codex 路径，退出恢复鼠标和焦点，仅回收测试创建的进程。GIF 画布中心未必有 alpha，抓取点必须选持续可见的实体区域。

- 仅运行改动涉及的脚本：依赖安装用 `python3 scripts/test-dependencies.py`；集成客户端用 `node scripts/test-integrations.js`；CLI/桥接用 `python3 scripts/context-smoke.py target/release/lexington`；设置用 settings-smoke；平台窗口用相关 native-smoke 模式。命令和场景详见模块文档。
- 夹具不调用模型、不安装真实依赖；安装验收仍遵守根 AGENTS 的 release、用户配置保留及安装版核对要求。
- 发布包配置使用空对象，不打包 config/roleplay.json、config/roleplay-memory.json 或 roleplay-vectors.sqlite3 及 journal/wal/shm；重复 Linux 打包还须清除旧生成目录遗留的私人配置与数据库。`bash scripts/test-package.sh` 使用假构建器和临时资源检查此行为，不访问真实配置。
- 内置 Embedding 的模型、tokenizer、哈希清单及许可必须随 resource 分发，打包缺文件即失败；Windows x64 还携带经校验的原生 CPU DLL 及其许可/第三方声明，不引入 Python/Node 运行链。安装/无源码发布包测试用假模型与假 DLL 验证完整复制；显式 `--check-embedding` 仅用固定文本验收随包本地模型，不读取用户记录或启动对话模型。
- 个人角色卡及立绘保存在 config/characters，升级须保留，发布包须排除；重复 Linux 打包清除旧生成包内的该目录前，先核对解析后的目标位于项目 dist 内，不删除源配置。
- import-live2d.py 只作离线原包格式转换，输出目录须不存在，不执行 ZIP 内脚本；其 NumPy/Pillow/OpenCV 工具环境不可加入原生应用运行链。Live2D 原生 PNG/JSON/f32 随 resource 分发，数值资源禁止换行转换。

- GNOME 原生穿透测试点击底层接收窗后可能改变测试窗堆叠；检查人物保留输入前须重新压低接收窗并抬高桌宠。原生 input shape 正确但指针子窗口仍为接收窗时，先核对堆叠，不能直接判为素材 alpha 穿透错误。

- X11 原生测试发现 PID 窗口后仍须等待 `IsViewable`，创建窗口不等于完成资源加载和映射；不要对未映射窗口立即 SetInputFocus。

- Bikini v2 使用 build-bikini-live2d-v2.py，默认仅生成草稿，审核后 --publish；旧 preview-bikini-live2d.py 保留在忽略的本地草稿中，不用于 v2 重建。配准及 ownership 快照属于忽略的本地固定输入，不从运行目录反推；缺少制作源时明确报错，正式运行不需要这些源文件。NumPy/OpenCV/Pillow 仅离线需要。

- 分层素材的清理应在原画像素恢复后执行；灰发杂色修复限定灰发层，不能对全角色套饱和度阈值。生成持物仍需检查自由轮廓，完整隐藏纹理不应在肘旁变成游离碎片。

- 带透明度的角色缩图先预乘，再使用正权面积滤波；Lanczos 负瓣会在轮廓外产生低 alpha 振铃和反预乘杂色，不用全轮廓开运算/阈值抹边补救。帽子的深度遮罩按几何区域制作，不能直接复用帽网 alpha 或补发 alpha。

- Flower 的离线入口为 build-flower-live2d.py，生成源保存在忽略的 live2d-drafts/Flower_Normal，默认草稿，--publish 发布。AI 补图配准后，可见原画像素须按互斥 ownership 分层，完整隐藏纹理限定在原不透明遮挡后方；直接叠加生成物体的自由轮廓会产生双帽沿、变粗手臂及漂浮茎叶。原 PNG 的 alpha=0 像素可有杂色 RGB，检查必须按真实 alpha 合成。
- Flower 部件左右按屏幕方向；持花臂内肘到前臂须沿实际皮肤轮廓分区，粗直线会把皮肤小块划进背发。即使头发绘制在手臂后方，错误归属仍会在摆动时露出手臂缺口；先检查源部件内容，再改遮挡顺序或幅度。局部肤色筛选必须保留近白皮肤高光，不能只接受暖色差，否则会挖掉臂内高光。
- 角色图层实验先保留完整运行模型快照；恢复外观须一并恢复 PNG、actor.json、制作脚本、测试及说明，不能仅降低动作幅度。数值通过不等于原画观感合格；回退用文件比对和脚本重建验证，避免残留实验图层再次覆盖恢复结果。

- GUI smoke 阻止真实 Codex 时使用 LEXINGTON_CODEX_EXECUTABLE 和隔离 CODEX_HOME；旧 config.json 的 codex.executable 已不生效，不能用它保证测试不访问真实服务。Linux 还须使用权限 0700 的独立 XDG_RUNTIME_DIR 隔离 Inbox 实例锁；只隔离资源根和 XDG_DATA_HOME 仍会与正在运行的普通/开发实例冲突。
