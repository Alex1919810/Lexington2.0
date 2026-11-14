# 工具与平台约定

继承根目录 [AGENTS.md](../../AGENTS.md)；此处仅补充本目录约束。模块与功能见 [docs/MODULES.md](../../docs/MODULES.md)；接口约束、实现细节及验证场景见 [docs/DEVELOPMENT_NOTES.md](../../docs/DEVELOPMENT_NOTES.md)。

- Linux 默认 X11/XWayland，原生 Wayland 用 `--wayland`，位置/层级由合成器管理。X11 桌宠必须是受 WM 管理的 Dock，不设置 strut；Utility 会被 GNOME 约束在屏幕内。设置 SKIP_TASKBAR/SKIP_PAGER/ABOVE，映射后再次置顶；仅鼠标按下时获取焦点，启动/悬停不抢焦点。
- X11 透明绘制不等于点击穿透：只设 XShape Input（≥1.1），禁止改变 Bounding 或全窗 CursorPassthrough。每帧收集当前立绘 alpha、可见控件/菜单范围，经 DPI 换算裁到 viewport 并缓存相同遮罩；禁止加入全屏 pet Area/历史区域。隐藏输入及关闭窗口立即移除范围，独立 viewport 不加入主窗遮罩；原生 Wayland 暂无局部输入区域实现。
- 拖动锚点在按下时读取原生屏幕指针/窗口位置，阈值按 DPI 换算；禁止跨阈值后才取初始位置或累加局部指针差，避免贴边重拖跳跃。X11 临时连接请求关闭前须 .check()，只 flush 可能丢失。
- 活动后台每 2 秒采样：loginctl 校验会话，优先 Mutter IdleMonitor，原生 X11 可回退 XScreenSaver；Wayland 不得用 XWayland 空闲时钟替代。不得读取键盘内容，失败保留手动入口；命令超时清理，退出回收线程。
- GIF 逐步缓存当前文件索引/调色板，预算 96 MiB，超限释放前缀并流式回退，切换/重载释放；禁止全库或合成 RGBA 帧缓存（曾触发系统杀进程）。保留源帧 0、2、4…，跳过帧延迟归入前一显示帧，奇数结尾保留，不跨循环合并；短延迟先归一化，总时长不变。
- 跳过帧仍须正确处理 Keep/Any、Background、Previous，但不创建全尺寸 RGBA、不上传纹理/重算遮罩；完整索引缓存循环清空画布、不重开源文件。循环、延迟和 alpha 网格细节见开发说明；解码变动必须 release 对照 image 参考解码器验证像素、合并延迟和缓存回放。

## 增量验证

- Windows 穿透须逐帧比较实际 GWL_EXSTYLE 后按需更新，不能只缓存期望值；winit 在焦点或层级变化时可能重写样式。HWND 保存为指针宽度 WindowId，不能截断为 X11 的 u32。
- Windows 跨应用穿透使用 WS_EX_LAYERED 与 WS_EX_TRANSPARENT 的组合；不能仅以 WM_NCHITTEST 返回 HTTRANSPARENT 代替，其继续命中仅保证同线程窗口。客户区指针按 DPI 换算后还须对齐 viewport 原点，裁剪后的零面积区域不得截获输入；不为命中测试替换 winit 的窗口过程。
- Windows 本地时间使用 CRT localtime_s/_mktime64；活动采样只接受未锁定的活动本地控制台会话，不读取输入内容。采样失败应禁用自动确认，保留手动入口。
- Windows GUI 启动的 CLI 可能不读取系统代理；桥接只通过子进程 Command.env 补充手动代理，显式代理环境变量（包括空值）优先，不修改全局环境或登录。WinHTTP 返回的代理、排除项和 PAC URL 均须 GlobalFree；仅有 PAC 时不能当成普通代理 URL。
- 向量存储保持通用命名空间接口，不读取角色卡、不调用 Embedding。sqlite-vec 静态注册一次；原文为本体、向量为可重建索引，编辑正文使索引失效，回填必须核对原文。查询先限定命名空间/模型空间/维度；未知版本或损坏库报错，禁止自动重置。
- 通用 JSON records 与向量 documents 分表、共用命名空间数据库；版本 1 到 2 仅事务新增 records 表并保留原文/向量。向量删除不得连带删除 JSON records；JSON 格式损坏报错，不自动覆盖。角色事件策略由 roleplay 实现。
- 本地 Embedding 封装只读取随包固定模型，禁止自动查 HF 缓存/下载或读取凭据。模型和 Windows CPU DLL 必须校验内置清单 SHA，相关资源禁用 Git 换行转换。动态 INT8 使用单条序列以稳定量化范围；token 分块使用原文 UTF-8 offset，关闭 tokenizer 截断后保留完整文本。显式选 CPU、限制线程并关闭 spinning/telemetry；不在 GUI 线程加载模型。
- Linux x86_64 的 ONNX Runtime 静态链接，安装版不需要外置 ORT 动态库或 Windows DLL，但仍依赖系统 libstdc++。用安装版 --check-embedding 验证固定中文检索/完整分块，不能只以 cargo check/Clippy 通过认定推理可用。

- 按改动选择 `cargo test --locked tools::gif::tests::`、`cargo test --locked tools::platform::tests::` 或具体用例；GIF/透明度变化补测对应 `ui::assets::tests::`，解码变化按开发说明运行 release 参考像素专项用例。
- 平台窗口改动仅运行相关 native-smoke 模式，遵守 `scripts/AGENTS.md` 的隔离和串行要求。
- 原生文件/项目目录选择只返回路径（取消为 None），调用线程与窗口层级由 UI 管理；不在工具中加载角色卡、写入导入资产或创建纹理。测试注入选择结果，不能弹出真实选择器阻塞自动化。
- 原生 Live2D 只产出 RGBA 和模型参数，不持有 GPU/UI；待机时钟与局部轨道分离，同通道限量排队，热区按当前根变换和持物位置逆变换。资源引用必须留在模型目录，公式字符串不可执行；图层在预乘 alpha 中变换，局部眼部只替换遮罩内 RGB，不改变 alpha。

- 原生角色入口按明确格式分派：婚纱 IK 与 `lexington-layered-actor-1` 刚性部件共用轨道控制，不用婚纱的固定五层骨架伪装其他服装。刚性部件命中逆当前根/部件变换后采样 alpha，身体前景遮挡后景命中；只缓存最新局部姿态，待机始终按全局时钟采样。

- 刚性部件的持续待机使用独立 `idle` 数值声明并读取全局时间，不借用点击轨道；发束 `fill_background=false` 避免在原位置回填旧发。旧分区回填模式只更新当前角度非零的部件包围盒；固定底图模式在 neutral 和动作期间沿用同一分层栈，范围必须包含部件遮罩及纹理余量。

- `fixed_background` 合成不能按旧/新多边形覆盖差抹掉固定底图；遮罩包围空白区域不等于当前纹理不透明。带隐藏余量的部件包围盒须同时覆盖实际纹理 alpha，避免余量被裁掉；新增接缝修复要覆盖非周期触发的中间帧。

- 完整手臂使用共享顶点连续蒙皮，不能用独立肘部切片拼接；发根锚定区需检查正负极值的三角形面积。三角形共享边直接写像素，不能重复 alpha-over。完整帽沿的前后归属插值两套完整深度合成，不分别裁切/重采样半透明纹理，否则会产生 alpha 接缝。
- 身体呼吸只影响身体层；网格和持物按当前骨骼命中，非交互头发同样遮挡后方帽子。关节接缝探针跟随骨骼，固定屏幕坐标可能落到正常移动轮廓外。

- 固定图层栈的包围盒外也必须使用背景/身体层，不可回退 original，否则素材清理会在腿部及外部失效。无翻折不代表发束保形；原画自由发束优先单位尺度刚性旋转，回归检查点间距离而非仅面积符号。

- 静态发根放在 background 时，需声明它与后帽沿的深度：background_above_depth 使后帽沿先于根层合成，前帽沿仍位于头部前。点击的后帽沿回退也须受根层 alpha 遮挡；不能只改渲染顺序而允许点击穿过头发。

- 持续往返的发束不能复用 sin³ 躯干包络：sin³ 在中位同样零速，会产生额外停顿。部件 idle 使用普通正弦并持续读取全局时间，保留端点平滑换向；回归要检查中位附近的逐帧位移，不只比较周期首尾和极值。

- rfd 0.17.2 的 Linux Zenity 目录选择回退只读取 file_name，不读取 starting_directory；项目选择须同时设置 set_directory 和带末尾 / 的绝对 file_name，才能沿用当前项目目录。

- GIF 以单调截止时间推进，调用次数不能影响播放速度；小幅迟到保留相位，新截止时间仍已过期时须丢弃整帧时间积压，避免 UI 阻塞恢复后高速追帧。保留当前索引、处置和循环次数，不为追赶反复解码或缓存 RGBA。
