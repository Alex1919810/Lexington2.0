# 工具与平台约定

继承根目录 [AGENTS.md](../../AGENTS.md)；此处仅补充本目录约束。模块职责、接口及详细场景见 [docs/MODULES.md](../../docs/MODULES.md)。

- Linux 默认 X11/XWayland，原生 Wayland 用 `--wayland`，位置/层级由合成器管理。X11 桌宠必须是受 WM 管理的 Dock，不设置 strut；Utility 会被 GNOME 约束在屏幕内。设置 SKIP_TASKBAR/SKIP_PAGER/ABOVE，映射后再次置顶；仅鼠标按下时获取焦点，启动/悬停不抢焦点。
- X11 透明绘制不等于点击穿透：只设 XShape Input（≥1.1），禁止改变 Bounding 或全窗 CursorPassthrough。每帧收集当前立绘 alpha、可见控件/菜单范围，经 DPI 换算裁到 viewport 并缓存相同遮罩；禁止加入全屏 pet Area/历史区域。隐藏输入及关闭窗口立即移除范围，独立 viewport 不加入主窗遮罩；原生 Wayland 暂无局部输入区域实现。
- 拖动锚点在按下时读取原生屏幕指针/窗口位置，阈值按 DPI 换算；禁止跨阈值后才取初始位置或累加局部指针差，避免贴边重拖跳跃。X11 临时连接请求关闭前须 .check()，只 flush 可能丢失。
- 活动后台每 2 秒采样：loginctl 校验会话，优先 Mutter IdleMonitor，原生 X11 可回退 XScreenSaver；Wayland 不得用 XWayland 空闲时钟替代。不得读取键盘内容，失败保留手动入口；命令超时清理，退出回收线程。
- GIF 逐步缓存当前文件索引/调色板，预算 96 MiB，超限释放前缀并流式回退，切换/重载释放；禁止全库或合成 RGBA 帧缓存（曾触发系统杀进程）。保留源帧 0、2、4…，跳过帧延迟归入前一显示帧，奇数结尾保留，不跨循环合并；短延迟先归一化，总时长不变。
- 跳过帧仍须正确处理 Keep/Any、Background、Previous，但不创建全尺寸 RGBA、不上传纹理/重算遮罩；完整索引缓存循环清空画布、不重开源文件。循环、延迟和 alpha 网格细节见模块文档；解码变动必须 release 对照 image 参考解码器验证像素、合并延迟和缓存回放。

## 增量验证

- Windows 穿透须逐帧比较实际 GWL_EXSTYLE 后按需更新，不能只缓存期望值；winit 在焦点或层级变化时可能重写样式。HWND 保存为指针宽度 WindowId，不能截断为 X11 的 u32。
- Windows 本地时间使用 CRT localtime_s/_mktime64；活动采样只接受未锁定的活动本地控制台会话，不读取输入内容。采样失败应禁用自动确认，保留手动入口。
- Windows GUI 启动的 CLI 可能不读取系统代理；桥接只通过子进程 Command.env 补充手动代理，显式代理环境变量（包括空值）优先，不修改全局环境或登录。WinHTTP 返回的代理、排除项和 PAC URL 均须 GlobalFree；仅有 PAC 时不能当成普通代理 URL。

- 按改动选择 `cargo test --locked tools::gif::tests::`、`cargo test --locked tools::platform::tests::` 或具体用例；GIF/透明度变化补测对应 `ui::assets::tests::`，解码变化按模块文档运行 release 参考像素专项用例。
- 平台窗口改动仅运行相关 native-smoke 模式，遵守 `scripts/AGENTS.md` 的隔离和串行要求。
