# 项目约定

- 本项目采用纯 Rust 原生 GUI：eframe/egui + glow；禁止引入 Vue、Electron、Node、WebView 或 N-API 作为运行链。`Cargo.lock` 随代码保留。
- `src/main.rs` 管理 UI/状态和后台请求通道，`src/backend.rs` 管理配置/扫描/HTTP，`src/assets.rs` 管理图片，`src/platform.rs` 管理 Linux 窗口属性与移动，`src/visual.rs` 管理默认宠物、容器变换和气泡形状。
- 保持 `resource/` 目录结构和 `config/config.json` 的 camelCase 字段兼容。资源根目录按 `LEXINGTON_HOME`、可执行文件旁、当前目录依次寻找。安装升级须保留用户配置/API key；日志和测试结果不得泄露密钥。
- 原版立绘组件为 450×450，侧栏按钮为 40×40、间隔 10；发布窗口 500×500，`--dev` 为 800×600。气泡按全文测高、21px 行高、最大高度限制；输入框沿用组件局部坐标边界检查后再平移到窗口中心。
- GIF 需要逐帧解码，不能常驻全部 RGBA 帧（现有文件 1024×1024、67 帧，曾触发系统杀进程）。保留文件的有限/无限循环信息；缺少循环扩展表示只播放一次。Chromium 对不足 20ms 的帧延迟使用 100ms。
- 图片启用线性 mipmap 过滤，避免高分辨率背景缩小后产生锯齿。字体采用 Noto Sans CJK TTC 的简体中文 face（index 2）、Liberation 和 Noto Sans Math；不要额外用 `FontTweak.scale` 按 ascent/descent 放大，14px 中文字宽应接近 14px。输入占位文字需显式指定 #999，避免系统主题改变颜色。
- 450×450 容器的悬停缩放与浮动应同时作用于立绘、对话和输入层；侧栏独立。气泡 Area 禁用自动约束，避免 egui 自动移动原版允许溢出的输入框。对话尾尖距左边 28px，输入尾尖居中。
- Linux 默认优先 X11/XWayland；原生 Wayland 用 `--wayland`，位置/层级由合成器控制。egui 的 taskbar 参数只支持 Windows，Linux 要设置 `_NET_WM_STATE_SKIP_TASKBAR`、`SKIP_PAGER`、`ABOVE`。窗口映射后再次请求 AlwaysOnTop，避免映射时序丢失置顶。Linux 拖动通过屏幕绝对坐标及 `_NET_MOVERESIZE_WINDOW` 实现，保持窗口尺寸。
- X11 临时连接的属性/消息请求在关闭连接前需 `.check()`，只 flush 可能丢失。拖动锚点在按下时捕获屏幕坐标与窗口偏移，阈值按 DPI 换算；不能等跨过阈值再取初始位置，也不能累加窗口局部指针差值。
- 禁止恢复 eframe 的原生窗口尺寸/位置和 egui 区域缓存；只保存侧栏固定模式。窗口初始化要重新应用 800×600 开发尺寸或 500×500 普通尺寸，避免先前拖动/开发运行影响后续启动。
- 原生单行 TextEdit 按 Enter 会失焦，提交判断必须同时考虑 `has_focus()` 与 `lost_focus()`。不要启动时主动聚焦输入框。
- 侧栏点击须阻止本帧对话的“外部点击关闭”，否则已打开对话时取消打卡会立即清掉新消息。HTTP 交互测试使用临时配置/本地模拟服务，不能依赖用户真实 API 配置。
- eframe 截图事件在 context input 读锁之外发 viewport 命令；锁内发送 Close 会导致退出卡住。截图的 Color32 是预乘 alpha，导出 PNG 需 `to_srgba_unmultiplied()`。
- 验证：`cargo fmt --check`、`cargo test --locked`、`cargo clippy --locked --all-targets -- -D warnings`。测试包含现有 30 张立绘、UI 指针/键盘事件、模拟 HTTP、计时、GIF 元数据。`cargo run -- --dev --screenshot /tmp/lexington.png` 验证真实渲染。
- `python3 scripts/native-smoke.py target/release/lexington /tmp/lexington.png drag` 验证置顶/隐藏任务栏和精确拖动；模式还包括 `clock`、`input`、`pin`、`idle`，`--production` 验证普通尺寸。脚本需 libX11/libXtst，会移动鼠标并恢复位置，宜用 Xephyr + 窗口管理器隔离测试。拖动验收需核对阈值、精确位移、反向移动和尺寸。侧栏跨启动验证使用独立 `XDG_DATA_HOME`，检查两次退出后 `lexington/app.ron` 中的固定模式。
- 打包：`bash scripts/package.sh`；用户级安装：`bash scripts/install-linux.sh`，创建菜单和桌面快捷方式，使用 `xdg-user-dir DESKTOP`，并尝试标记可信。
- 未经用户明确授权，不执行 git commit 或 git push。代码任务结束前将新增且验证过的项目知识合并进本文件，避免记录临时进度。
