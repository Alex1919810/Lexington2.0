# 模块职责

代码按 UI、后端功能、角色扮演和工具代码组织。`main.rs` 负责启动，UI 负责展示与交互，后端负责业务规则及基本 Codex 功能，平级的 roleplay 模块负责角色卡与角色提示，development 模块负责项目工作区、开发任务和执行证据，工具提供路径、解码、平台适配、本地 Embedding 和通用向量存储能力。

## 目录结构

```text
.agents/skills/
├── AGENTS.md
└── unified-live2d/
    ├── SKILL.md
    ├── agents/
    │   └── openai.yaml
    ├── references/
    │   ├── actor-contract.md
    │   ├── authoring.md
    │   ├── eyes.md
    │   ├── hair.md
    │   ├── lexington.md
    │   └── quality.md
    └── scripts/
        ├── actor_core.py
        ├── audit_actor.py
        └── reference_blink.py
src/
├── main.rs
├── ui/
│   ├── AGENTS.md
│   ├── mod.rs
│   ├── alarm.rs
│   ├── app.rs
│   ├── pet.rs
│   ├── sidebar.rs
│   ├── settings.rs
│   ├── role_card.rs
│   ├── new_character.rs
│   ├── roleplay_context.rs
│   ├── roleplay_memory.rs
│   ├── roleplay_vector.rs
│   ├── character_memories.rs
│   ├── dialog.rs
│   ├── clock.rs
│   ├── codex.rs
│   ├── development.rs
│   ├── development_diagnostics.rs
│   ├── development_tests.rs
│   ├── development_memory.rs
│   ├── input.rs
│   ├── assets.rs
│   ├── style.rs
│   ├── transparency.rs
│   ├── weather.rs
│   └── tests.rs
├── backend/
│   ├── AGENTS.md
│   ├── mod.rs
│   ├── alarm.rs
│   ├── config.rs
│   ├── resources.rs
│   ├── weather.rs
│   ├── character.rs
│   ├── codex.rs
│   ├── codex_config.rs
│   ├── handoff.rs
│   ├── handoff_windows.rs
│   └── clock.rs
├── development/
│   ├── AGENTS.md
│   ├── mod.rs
│   ├── diagnostics.rs
│   ├── testing.rs
│   └── memory.rs
├── roleplay/
│   ├── AGENTS.md
│   ├── mod.rs
│   ├── card.rs
│   ├── character_memory.rs
│   ├── creation.rs
│   ├── context.rs
│   ├── profile.rs
│   ├── registry.rs
│   ├── vector_memory.rs
│   └── store.rs
└── tools/
    ├── AGENTS.md
    ├── mod.rs
    ├── paths.rs
    ├── activity.rs
    ├── embedding.rs
    ├── file_dialog.rs
    ├── gif.rs
    ├── live2d/
    │   ├── mod.rs
    │   ├── layered.rs
    │   ├── mesh.rs
    │   └── render.rs
    ├── platform.rs
    ├── platform_windows.rs
    ├── proxy.rs
    ├── time.rs
    ├── vector_store.rs
    └── windows.rs
tests/
└── fixtures/
    └── codex-server.py
integrations/
├── README.md
├── browser/
│   ├── background.js
│   ├── manifest.firefox.json
│   └── manifest.chromium.json
├── vscode/
│   ├── package.json
│   └── extension.js
└── nautilus/
    ├── send.py
    └── lexington_menu.py
scripts/
├── AGENTS.md
├── build-bikini-live2d-v2.py
├── build-flower-live2d.py
├── import-live2d.py
├── install-linux.sh
├── install-windows.ps1
├── package-windows.ps1
├── test-windows.ps1
├── test-package.sh
├── windows-smoke.py
├── install-dependencies.sh
├── dependencies.py
├── test-dependencies.py
├── package.sh
├── integrations.py
├── context-smoke.py
├── test-integrations.js
├── settings-smoke.py
└── native-smoke.py
docs/
├── MODULES.md
└── agents/
    ├── AGENTS.md
    └── lexington/
        └── PERSONA.md
resource/agents/
├── characters.json
├── lexington.md
└── lexington.json
resource/models/bge-small-zh-v1.5/
├── model.onnx
├── config.json
├── tokenizer.json
├── tokenizer_config.json
├── special_tokens_map.json
├── manifest.json
└── LICENSE.txt
resource/runtime/windows-x86_64/
├── onnxruntime.dll
├── manifest.json
├── LICENSE
└── ThirdPartyNotices.txt
resource/illustration/lexington/live2d-drafts/   # 本地临时目录，Git 忽略，非仓库文件
resource/illustration/lexington/live2d/Flower_Normal/
├── actor.json / README.txt
├── original.png / garden.png / body.png
├── hair.png / left-arm.png / right-arm.png / bouquet.png / ribbon.png
├── hat-back.png / hat-front.png / bracelet-back.png / bracelet.png / sleeve.png
├── butterfly-1.png / butterfly-2.png / butterfly-3.png / petals.png
└── 各部件对应 *-mask.png；正式目录仅保留运行引用及说明
resource/illustration/lexington/live2d/BiKiNi_Normal/
├── actor.json / README.txt
├── original.png / roots.png / body.png
├── board.png / hair.png / arm.png / hat.png
├── board-mask.png / hair-mask.png / arm-mask.png / hat-mask.png / hat-front-mask.png
└── 正式目录仅保留 actor.json 引用资源及 README.txt
resource/illustration/lexington/live2d/WeddingDress_Normal/
├── actor.json
├── source-actor.json
├── README.txt
└── assets/
    ├── original.png / plate.png
    ├── left_upper.png / right_upper.png / left_lower.png / right_lower.png
    ├── bouquet.png / shoulder.png / bodice.png
    ├── eye-atlas-33.png / eye-cover-mask.png
    ├── hair-mask.png / hair-occlusion.png / hair-front.f32
    └── breath_dx.f32 / breath_dy.f32
```

## 项目开发技能

- 项目技能独立位于 `.agents/skills/`，维护约定见其中的 AGENTS.md；Codex 仓库技能目录约定参考 [OpenAI 官方文档](https://learn.chatgpt.com/docs/build-skills#where-codex-loads-local-skills)。这是开发工作流，不注入 Lexington 角色或普通聊天，也不由桌宠启动和安装流程加载。
- `unified-live2d/SKILL.md` 保存从原画建立统一基准、共享呼吸/摇摆/头发时钟、独立局部动作、热点和原生接入的制作流程；`agents/openai.yaml` 保留中文显示名称、简介和 `$unified-live2d` 默认调用提示。来自当前用户的统一角色制作技能，仓库内为可移植副本，不链接个人目录。
- `references/authoring.md` 负责分层与原画保形，`actor-contract.md` 负责通用模型/轨道/命中契约，`hair.md` 和 `eyes.md` 负责对应局部通道，`quality.md` 负责静态/运动/编码验收；`lexington.md` 连接本项目模块规则、原生格式、换装目录与透明输出验证，避免把通用预览模型误当作任意 Cubism 或当前原生运行模型。
- `scripts/actor_core.py` 提供 normalize/sample、通道事件排队、固定 pole 两骨 IK、根矩阵及移动热区的离线参考；`audit_actor.py` 校验通用/draft 模型、受限相对资源引用、轨道归位、网格和可选 GIF 调色板/端点；`reference_blink.py` 提供参考眼睑/眼白的局部眨眼合成。核心数学只用 Python 标准库；PNG/GIF 审计需要 Pillow，眨眼合成需要 NumPy/OpenCV。这些依赖仅属于离线工具，不加入应用运行链。
- 使用技能目录内的相对引用，不提交 __pycache__、虚拟环境、个人路径或临时预览。通用审计器只支持 unified-actor-1 / lexington-parametric-actor-draft-1；当前 lexington-native-actor-1 仍由本项目 Rust 加载和运行验证，不能用审计器的通过结果宣称原生交互已验收。
- 增量验证：skill-creator quick_validate 检查入口/frontmatter；`python .agents/skills/unified-live2d/scripts/actor_core.py --self-test` 检查时钟、轨道端点、叠加、限队列、固定 IK 和移动热区；三个脚本通过 py_compile；audit_actor 使用原婚纱 draft 素材验证 18 个资源和两项动作，明确不验证运行集成。本次未修改 Rust 代码，不重复运行 Rust 测试。

## 桌面透明合成与白边修复

- egui Color32::from_rgba_unmultiplied 在线性颜色空间预乘，再编码为 gamma RGB；egui_glow 的采样/混合输出保留此表示。桌面透明窗口最终需要按颜色字节预乘的 RGB。直接提交原结果会让低 alpha 边缘颜色偏亮，旋转、缩放及 mipmap 后出现亮边/细碎毛刺。离屏 RGBA 与参考动画一致不足以排除最终桌面合成问题。
- ui/transparency.rs 在 root UI 所有绘制之后登记 Debug 层 PaintCallback：把当前帧拷贝到可复用的 RGBA8 纹理（不是 sRGB 纹理），用一次屏幕覆盖 pass 将 RGB 解码到线性、除以 alpha、重新编码到 gamma，再乘 alpha。原 alpha 保持，alpha=0 清 RGB，alpha=1 原样输出；不裁边、不提高 alpha 阈值、不改 PNG/GIF/Live2D 素材。关闭混合覆盖结果，避免又叠加一次，使用 1:1 nearest 采样；egui_glow 在回调后恢复绘制状态。
- 只转换主透明窗口，独立设置等原生 viewport 不登记回调。帧纹理仅尺寸变化时重新分配，不累积历史帧，正常绘制不做 GPU→CPU 读回。资源在 Wife::new 的真实 GL 上下文创建，on_exit 中回收；测试构造 from_root 无 GL 资源。GLSL 140/GLES 300 初始化失败保留原绘制并仅报通用错误，专用验证入口返回失败。
- egui_glow 截图事件将原始帧缓冲字节装入 Color32 容器；经过最终转换后，该容器内容已是桌面 gamma 预乘表示，不能再次按普通 Color32 解码。screenshot_rgba 按字节除 alpha 导出 straight RGBA，透明像素归零；未启用转换的普通 Color32 继续使用 to_srgba_unmultiplied。
- `cargo run --locked -- --check-transparency` 或安装版 `--check-transparency`：创建独立隐藏 GL 上下文与测试 FBO，实际执行同一着色器，对白色、肤色、蓝色 × 12 种 alpha 共 36 个像素读回校验：alpha 完全相同、RGB 不超过 alpha、与预期颜色误差至多 2/255；不读取资源根、配置、登录、记忆或真实模型。失败返回非零，结果只含验收摘要。单元过滤器 `cargo test --locked ui::transparency::tests::` 检查截图只去预乘一次、透明/不透明像素及低 alpha 白边读回。
- 本机 Windows 原生窗口链路另用独立 LEXINGTON_HOME/CODEX_HOME/HOME/USERPROFILE/LOCALAPPDATA/APPDATA/XDG_DATA_HOME、禁用自动打卡和不可用 Codex 路径，配置仅含婚纱 Live2D 素材，运行 --dev --screenshot 自动保存后退出；不注入鼠标输入、不读取真实用户记录。实际 GL 截图在深色/白色底合成检查发丝、花瓣与裙摆，透明像素 RGB 为 0；GPU 校验、截图读回单元测试及 fmt/Clippy 通过。Linux x86_64 GPU 与 X11 原生增量验收见验证入口；不据此推断其他合成器。

## 原生 Live2D 风格角色

项目技能 `.agents/skills/unified-live2d/` 的制作/头发/验收参考包含连续表面、边缘振铃、帽发遮挡、补发覆盖及中位停顿的处理；Lexington 参考区分两种原生格式和临时草稿/正式交付边界。技能不授予提交或推送权限。

- Flower_Normal 从 `png/Flower_Normal.png` 制作，保留源 PNG/GIF。内置 imagegen 分别补齐躯干、头发、左右臂和配饰图集，实际输出 1254×1254；`scripts/build-flower-live2d.py` 在预乘 alpha 中配准为 512×512 的 `lexington-layered-actor-1`。可见原画像素按互斥 ownership 分配，AI 补图仅进入原遮挡后方；透明区原 RGB 不当作可见杂色。完整生成图、提示和审核图在忽略的 `live2d-drafts/Flower_Normal`；默认草稿，`--publish` 最后原子替换 actor.json。离线制作需 NumPy/OpenCV/Pillow，运行与常规测试只依赖正式模型。
- Flower 使用原画姿态及 14 个部件：固定躯干/脸/前发与花园底板，另有背发、双臂、花束、帽子前后层、手环前后层、右袖口遮挡、飘带、三只独立蝴蝶与花瓣。花束/持花臂共用肩轴 -2.2°、2.8 秒的小幅动作；右臂/手环 1.8°、帽子 -1.8°、飘带 2.6°、独立蝴蝶 9°。保留通道叠加、限量排队、躯干呼吸及全局 6 秒 ±2° 背发摆动，不增加眨眼。正式模型没有左肩袖重绘或胸前抬花 IK；相关实验素材留在忽略的制作目录。
- 左右按屏幕方向；`left-arm.png` 为持花臂。保留内肘/前臂皮肤及近白高光归回手臂层的修复。`flower_screen_left_arm_skin_stays_in_front_of_swaying_hair` 检查原皮肤 RGBA、发层无误分皮肤、正负发摆极值与持花臂小幅动作叠加、该处点击，并固定为无 rig 的 14 部件模型。`flower_layers_keep_local_clicks_independent_and_return_to_shared_idle` 覆盖五类 alpha 命中、移动手臂命中、手环跟随、独立通道/限队列、脸部像素与归位到同一全局时刻待机；UI 既有用例显式选择 Flower，检查悬停逆变换点击和拖动不排队。
- `flower_native_review_frames` 默认忽略，从正式模型输出静止/中间/极值、正负发摆叠加与 181 帧错相触发预览至 `.tools/flower-review`，不依赖草稿。恢复版正式资源与改动前快照逐文件比对，离线脚本复现全部 PNG 字节及相同 actor.json 数据；仅做 Live2D 相关增量测试，安装按项目规则同步 release、保留配置并安全重启。

- BiKiNi_Normal 当前制作源为 `live2d-drafts/BiKiNi_Normal_v2/`；`live2d-drafts/` 已 Git 忽略，仅作本地临时制作；旧 `BiKiNi_Normal/` 草稿及旧切片脚本也在该目录存档。正式构建、运行和常规测试只依赖提交的 live2d 模型；`bikini_v2_review_frames` 也从正式目录生成快照。原图 1024×1024，模型 512×512，生成工具实际输出 1254×1254。完整板身、背发、身体清洁底板、整条扶帽臂、完整草帽分别生成；姿态参考整图仅作审核，未用整人重绘替换运行角色。
- `scripts/build-bikini-live2d-v2.py` 读取 generated PNG、实测 `alignment.json` 和固定 ownership 快照，在预乘 alpha 中做仿射/TPS 配准；默认仅导出草稿 model，`--publish` 复制审核文件并最后原子替换 actor.json。离线需要 NumPy/OpenCV/Pillow，应用无需这些依赖。可用 `.tools/bikini-authoring/bin/python` 工具环境运行；这些临时输入不随 Git 分发，缺失时离线工具明确报错，不从旧运行纹理自动生成 ownership。原脸轮廓按 RGBA 恢复，身体可见核心沿用原画；肩部、整臂与完整板面使用配准后的连续生成纹理，避免将旧切片的遮挡边界拼回皮肤/板面造成内部色缝。生成提示、原始文件与配准数据保留用于复核。
- 待机呼吸仅作用身体，整体摇摆 6 秒；冲浪板 ±3.5°，背发以 (258,145) 为轴整束刚性旋转、6 秒 ±6°，发根由固定头部遮挡。抬帽轨道保留 `hat_angle` 控制名（0..6），实际驱动固定弯曲侧两骨 IK，腕部最多上移 8px，整条手臂共享连续蒙皮网格，帽子跟随前臂刚性变换，不是完整脱帽。脸与躯干不跟随手臂动作拉伸。`pose-review.png` 展示静止/中间/极值，`review.gif` 和 `detail.gif` 直接来自原生 Rust 连续渲染，属于审核预览；交互另用实际窗口验收。

侧栏顺序调整同时同步 `scripts/native-smoke.py` 的 clock 模式：打卡按钮距首个按钮中心偏移 300 像素；pin 模式及 PNG/GIF 按钮坐标保持。Linux x86_64 已用隔离 X11 实际点击打卡入口；相关验收见验证入口。

- 侧栏在正常/Q版立绘切换按钮下方放置 L 按钮，其后依次为角色切换、打卡；存在当前角色的素材时可切换，当前模式高亮。默认仍为 GIF。目录为 resource/illustration/<角色>/live2d/<换装>_Normal/actor.json，个人角色可在 config/characters/<角色>/live2d 使用相同结构；_Damage 可选。换装前缀与 PNG/GIF 去掉最终状态后缀后的名称一致，例如 Lexington 的 WeddingDress_Normal。只扫描 actor.json 的模型目录；▶ 只遍历当前格式/状态，战损按钮仍需对应资源。
- backend/resources 只发现路径和角色/换装/状态；CharacterState::can_toggle_live2d/toggle_live2d 保持角色，优先同换装、再匹配状态，无对应换装则选择该角色已有模型。退出 L 优先同换装 PNG，再回退 GIF；普通格式按钮仍切换 GIF/PNG。同角色格式切换保持 Codex 会话。select_first 的三格式回退处理仅有 PNG 或仅有 Live2D 的下一角色，刷新保留当前模型。
- 首套资源来自用户提供的 unified-character-hair-assets.zip v4，位于 resource/illustration/lexington/live2d/WeddingDress_Normal。使用自定义 lexington-native-actor-1 格式，并非 Cubism moc3；不声称支持任意 Cubism 模型。actor.json 含相对文件引用、画布/原点、骨骼和固定 IK pole、图层、遮罩、呼吸向量场、头发参数、33 态局部眼部图集、动作轨道与热区。source-actor.json 仅保留原包描述，不由运行时解析。
- BiKiNi_Normal 的原生资源位于 `live2d/BiKiNi_Normal/`，使用 `lexington-layered-actor-1`，由 Actor 分派到 `tools/live2d/layered.rs`，既有婚纱格式仍由原分层/IK 渲染器处理。两者共用通道/轨道校验、采样与限队列控制。新格式只接受同画布 PNG 部件、分区遮罩、背景补图、静态前景和模型数值，不执行素材脚本；引用沿用 canonicalize 路径限制。部件最多 16 个、局部通道默认角度 0、范围不超过 ±15°，固定轴保形旋转；可选 `idle:{period,angle}` 部件不绑定 action/channel，要求有限正周期、幅度 ≤15° 及 `fill_background=false`，避免持续发束摆动留下旧发。后景在前景部件之前。
- Bikini 背发不再使用渐变 anchored 权重；整束 scale=1 旋转检查点间距离。部件 idle 角度使用独立的普通正弦 `angle*sin(2π*global_time/period)`，中位速度不为零；不复用躯干的 sin³ 包络，避免中位停顿。移动背发只保留 y≥160，生成头皮不混入背发；静态 `roots.png` 在头部/手臂后衔接移动发束；根部上沿沿后脑曲线延伸到太阳穴上方，不依赖 neutral 帽沿常数高度。生成旧手部留下的小片低覆盖按局部头侧支撑区延展邻近灰发纹理，边缘面积采样，避免抬帽露出矩形空洞。原画恢复的发区先排除完整臂域和静态身体所有权，防止上臂/泳装像素随头发摆动；RGB 沿有效发区距离平滑过渡，不能在旧手臂孔边界硬贴原发颜色形成亮色块。自由发梢沿用原画 RGBA，生成内容用于隐藏根部；臂/身体原遮挡区域保留生成背发，不能清成旧手臂形状的孔。
- 冠部使用已配准的完整生成头部底板，顶部下移 1px 匹配原发顶点；134..143px 仅在发区渐变接回原画，143px 以下脸部保持原画。不能沿 neutral 帽发接触线裁掉抬帽时需要露出的头发。生成内部 alpha≥240 的实心冠发归为不透明，避免后帽沿透出；外侧 AA 保持。帽子 `front_mask` 为连续几何深度区域，与帽网 alpha、补发 alpha 分离；帽沿盖住隐藏重叠，帽网孔不会变成深度孔。原画帽沿恢复排除灰发像素，帽发接触 4px 带直接使用完整帽纹理，避免混色原画像素带起灰边；深度区域的曲线边界保留 1px 连续覆盖，而非二值硬边。花心不能用肤色阈值裁掉；手指孔跟随真实臂纹理。
- 胸/腰外侧原画 alpha<255 时，身体底板 alpha 不能比原画更大；193..270px 右轮廓在原发/板背景处按原图肤色边界取样并解混最后一个像素，清掉生成肩部突起和板色残片。身体不能在原画边缘之外保留更宽的生成底板。回归同时检查恢复冠发、胸腰轮廓和发层不含原上臂肤色；抬帽 0/3/6 控制值下太阳穴支撑点 alpha≥250，帽接触纹理不含原灰发条。
- 制作缩图在预乘 alpha 中使用 OpenCV INTER_AREA 正权面积滤波；Lanczos 负瓣曾在腿外产生相隔一像素的 alpha 1..4 振铃点，反预乘后 RGB 极端。禁止通过全轮廓形态开运算/硬阈值补救，否则细边变成硬台阶；只移除微小游离连通块。330px 以下无移动物体，身体保留面积缩图后的原画完整 RGBA。新增回归覆盖虚高头皮、帽沿前景、外侧振铃点和腿部一致性。

- 固定栈在移动部件包围盒以外也必须返回背景与身体合成，不能退回 original；否则腿部/外围会重新出现已清理的原始边缘像素。旧分区模式仍保留 original 回退。回归检查包围盒外不恢复源图杂色，以及头发两侧极值保持距离。
- `LayeredActor` 支持可选 rig、部件骨骼 binding 与 mesh；`mesh.rs` 处理共享顶点的三角网格：anchored 网格按权重固定发根，arm 网格按 root/upper/lower 权重连续变形。验证顶点数 3..2048、索引数 ≤12288、有限坐标/权重及非退化三角形；arm 必须关联 rig，权重和为 1。固定 IK 分支并在通道范围采样验证腕部可达，单通道 target_delta 每单位位移不超过 3px，骨段 scale=1。共享边界像素直接写入而非重复 alpha-over，避免网格对角线色缝。
- `fixed_background=true` 时所有姿态使用同一图层栈，不按旧新多边形覆盖差扣底图；包围盒包含遮罩及实际纹理 alpha。完整帽子使用单个 `front_mask` 进行深度归属：完整帽在发束/身体后与完整帽在前两种合成结果之间插值，同一纹理只重采样一次，不分别裁切前后半透明帽沿再叠加。`background_above_depth=true` 将静态根层放到后帽沿前、其他后景部件后；前帽沿仍按遮罩盖住根层，后帽沿命中同时受根层 alpha 遮挡。此选项默认 false，只允许固定背景且有深度部件的模型。只允许一个深度部件，不与其他 foreground 部件混用。
- `breath_body_only=true` 时呼吸只在静态身体取样阶段施加，手臂/帽/板不受躯干位移场挤压；最终统一根变换。兼容旧模型的整层呼吸及固定轴部件。缓存仅最新姿态，网格临时栅格化，不缓存整段 RGBA；零 alpha RGB 归零。
- 命中逆当前根、部件或网格变换，按同一深度归属检查可见 alpha（≥8/255）；非交互发束也阻挡后方帽子，身体遮挡后景，帽子/扶帽臂触发 hat，板触发 board。点击采样已显示帧时间，待机不重启，同通道限队列，拖动排除短按。
- tools/live2d::Actor::load/render/click 不依赖 UI；加载校验画布/图集/轨道/通道范围与文件尺寸、有限浮点值，资源路径 canonicalize 后必须留在模型内，拒绝绝对路径/上级路径。动作只允许列出的缓动名，不执行 JSON 中的公式、Python 或外部程序。画布最多 1024×1024 总像素、边长最多 2048，模型描述最多 64 KiB；不扫描 Codex 配置/登录。
- Actor 只接收调用方的单调秒数，待机使用持续全局时间，动作使用事件局部时间。不同通道可叠加，同通道至多排队一次重放，不能重置当前动作；完成时清理事件，首末轨道归默认值。头部椭圆只触发 blink（0.64 秒）；花束多边形只触发 bouquet（3.8 秒、最大抬升 10 像素），热点随抬花及根变换更新。先逆 UI 屏幕变换，再逆模型根旋转/平移命中。
- tools/live2d/render.rs 使用裁剪原图层、固定方向两骨 IK 和 scale=1 的刚性臂/花束变换；预乘 alpha 双线性重采样，32 分之一像素权重。按原参考顺序恢复肩部/衣服遮罩，避免半透明层重复叠色；只对躯干呼吸，扩张/高斯平滑保护手臂、花束及独立发束。发束读取同一时钟、刚性旋转并恢复当前前景遮挡；眼部仅通过局部遮罩替换 RGB 并保留 alpha。最后统一施加一次模型根变换；缓存仅保留最近一份手部姿态及保护场，不缓存整段合成帧。
- ui/assets::Picture 将 Raster 解码器或 Live Actor 装入枚举，UI 持有纹理与单调时钟。Live 以至多 30fps 更新纹理和 alpha 输入遮罩；tools 只输出 RGBA。模型自身提供根摇摆，ui/pet 保留悬停缩放/抬升并取消额外正弦悬浮，避免双重摇摆。Live 使用完整显示图像范围接收部位点击，短按触发局部动作；拖动不触发动作，未配置部位不触发随机台词或其他动作，右键入口保持。平台最终仍仅接收可见 alpha 和控件输入范围，不把整块透明画布变为原生可点击区域。
- scripts/import-live2d.py 是本次原包的离线格式转换工具，读取 ZIP 中已验证的 NumPy rig/头发数组和 PNG，生成原生 PNG、16 位遮罩与 little-endian float32 场；不执行原包脚本，不覆盖已有输出目录。离线需要 NumPy/Pillow，参考视觉对照另需 OpenCV；应用、安装包和启动流程无 Python/Node/WebView/Cubism SDK 运行依赖。f32 资源禁止 Git 换行转换；现有资源目录复制流程自动分发模型。

验证补充：`cargo test --locked live2d` 定向覆盖两套模型、限队列/移动热区、UI 点击/拖动、原画脸/躯干保留、遮挡命中、连续 IK 骨长、网格极值无翻折、共享边 alpha、固定底图、近 neutral 连续性与随动肘部接缝；头发覆盖探针跟随发束变换，取周围仍为实心纹理的内部点，不把固定世界位置的抗锯齿自由轮廓误判为接缝。`bikini_v2_review_frames`（release、ignored）输出静止/中间/极值；`bikini_continuous_seam_frames` 输出 181 帧（0.37s 点板、1.11s 点帽、发束持续摆动），生成 full/detail 审核 GIF。非周期触发、局部放大与中间帧必须人工查看，数值或连通性通过不能证明无颜色接缝。隔离 Xephyr + GNOME WM 验证实际穿透和部位点击，不用离屏 RGBA 替代窗口。测试不读取真实配置，不运行全量测试。

验证：tools::live2d::tests:: 覆盖轨道边界、独立通道、重复点击限队列、移动热点、非法轨道及路径；ui::assets::tests::live2d_sidebar_and_transformed_body_clicks_trigger_only_local_actions 使用 egui 指针事件检查 L 入口、变换后头部/花束点击及不产生随机聊天。backend::character::tests:: 补测前缀、格式回退和模型刷新；其余受影响 Picture/GIF、pet transform 与侧栏用例定向回归，不运行全量测试。`cargo test --release --locked live2d_reference_frames -- --ignored --nocapture` 将固定五组原生像素样例写入忽略的 .tools/live2d-verify，包含待机、抬花极值、闭眼和叠加；本机对照原包 Python 参考，预乘 RGBA 的单通道平均差 ≤0.041/255，最大差 11/255，并查看并排图确认轮廓。单次 release 原生帧计算约 5.5～22.2ms，不含纹理上传/原生窗口，不作为性能保证。Linux x86_64 已补测 X11 穿透、打卡及 Xephyr + GNOME WM 拖动，见验证入口；模拟事件不能替代原生验证。

## Agent 角色设定资料

`docs/agents/` 保存产品内角色的设计，局部维护规则见 [AGENTS.md](agents/AGENTS.md)。[Lexington 设定](agents/lexington/PERSONA.md) 定义人格、语气、默认“太太”关系及交互约定。默认角色卡为 [resource/agents/lexington.json](../resource/agents/lexington.json)，兼容 [旧式 Markdown](../resource/agents/lexington.md)，映射见 [characters.json](../resource/agents/characters.json)。运行时角色资源随 `resource/` 安装和打包，设计文档目录不参与分发。

角色背景及原台词摘句参考舰R百科与页面贡献者的[列克星敦 CV-2 固定版本 125209](https://www.shipgirlfriends.moe/index.php?title=%E5%88%97%E5%85%8B%E6%98%9F%E6%95%A6%28CV-2%29&oldid=125209)，角色来自《战舰少女R》。默认太太关系、桌面日常与新增对话示例为项目改编或二创；角色文字改编按 [CC BY-NC-SA 4.0](https://creativecommons.org/licenses/by-nc-sa/4.0/deed.zh-hans) 提供，不改变其他文件许可。这些维护信息不注入模型。

### 角色映射与会话生命周期

`src/roleplay/` 是与 backend 平级的领域模块，局部规则见 [角色扮演约定](../src/roleplay/AGENTS.md)。不依赖后端、UI 或 egui，不管理 Codex 进程。

| 文件 | 职责和接口 |
|---|---|
| `roleplay/mod.rs` | 导出 RoleCard、Rule、WorldEntry、RoleplaySession、ContextPlan、UserProfile、Memory、EmbeddingSource、VectorMemorySettings、clear_vector_memory、agent_instructions、session_card、default_character、static_plan、save_role_card、create_character |
| `roleplay/registry.rs` | 按 resource/agents/characters.json 映射角色，校验目录边界和 .md 相对路径；有结构化卡时生成提示，否则读取旧 Markdown；文件上限 64 KiB，拒绝缺失、空白、错误编码与非法路径 |
| `roleplay/card.rs` | 角色名称、描述、性格、场景、关系、对话示例、用户人设、手动记忆、世界书及附加规则；保留核心设定、编译规则与分段示例，包含上下文预算和世界书持续/冷却配置；卡片最多 64 KiB，世界书和规则各最多 64 条；Debug 隐藏内容 |
| `roleplay/creation.rs` | create_character 校验名称、角色冲突、角色卡和至少一张 PNG/GIF，在 config/characters 内暂存完整卡片和图片后发布；load_created_card 提供用户角色卡读取；不覆盖已有角色、源图或全局角色配置 |
| `ui/new_character.rs` | 新建表单、原生文件选择和拖入图片；viewport 回调后启动后台创建，由 Wife 持有任务并每帧完成刷新；无立绘时禁止创建，成功后编辑新卡，保持已有活动角色/聊天 |
| `roleplay/store.rs` | 从 config/roleplay.json 的 roleCards 分组读取按角色保存的覆盖值；角色卡和用户档案复用按分组保存接口，保留未知字段和其他角色，同目录临时文件同步后原子替换，Unix 权限 0600，损坏文件拒绝覆盖；不读写基本 Codex 配置或凭据 |
| `roleplay/vector_memory.rs` | 持有按角色的向量记忆策略、本地/远程 Embedding 配置与调用、对话切分、召回及清空接口；本地调用 tools/embedding，SQLite 调用 tools/vector_store，不依赖 backend/UI 或 Codex 登录 |
| `roleplay/character_memory.rs` | 定义 Extraction、Episode；提供提炼指令/JSON Schema、字段和原文依据校验，以及按角色的 list/store_extracted/delete；人物事件仅供查看，不自动注入角色提示或固定档案 |
| `ui/character_memories.rs` | 管理独立后台提炼队列和 Client 事件缓冲，仅成功结果交给 roleplay 校验保存；提供与设置平级的独立人物记忆 viewport、角色选择、事件浏览、搜索、筛选和删除；浏览状态独立于 RoleEditor，后台结果和状态不进入正常聊天，具体边界见人物事件记忆小节 |
| `ui/role_card.rs` | RoleEditor 保存编辑对象、新建草稿、分区、错误和提示；提供新建角色入口，已有角色编辑草稿独立保留；角色卡页面编辑基础设定、世界书、规则、上下文、用户与记忆和长期记忆，支持世界书新增/删除和规则新增/启停/排序/删除；保存后才提交角色领域模块，取消/关闭/切换编辑对象丢弃草稿 |

优先级：config/roleplay.json 的用户覆盖 → config/characters/<角色>/card.json 的新建角色卡 → 映射 Markdown 同名的 .json 默认卡 → 原 Markdown。未映射且没有用户卡的角色使用新的普通会话。默认角色卡为 resource/agents/lexington.json，默认太太关系、原台词摘句、二创日常和多轮对话示例分字段保存；来源与许可等维护信息不注入模型。JSON 和 Markdown 随现有资源分发；用户角色配置不作为内置资源覆盖。

backend/character.rs 继续负责立绘、格式和台词选择；首次选择通过 roleplay::default_character 优先匹配 lexington，否则回退首个可用角色，已有有效选择保持。角色领域数据无需增加 Rust 角色专属分支。

backend/codex.rs 的 Client::start(config, root, character, wake) 取得组装了用户档案的角色卡并创建 RoleplaySession，通过 with_memory(root, character) 附加启用中的历史记忆。静态计划通过 thread/start.developerInstructions 发送一次，本轮计划作为 turn/start.input 的独立文本块，原始用户消息仍为第一个文本块，审批和沙箱流程保持。实际写入请求后回传 Context 报告；预览不发送模型请求。已知当前环境/提供方 API Key 在进入角色历史、Embedding 和持久化之前遮蔽，原始用户消息仍按原文发送给 Codex；后端不直接实现向量存储或 Embedding 请求。

ui/app.rs 保存 codex_character、codex_role_card 和角色配置文件时间戳；设置窗口新增第 5 号“角色卡”页面（ui/settings.rs），操作在 viewport 回调结束后执行。每 500ms 检查 config/roleplay.json 和 config/roleplay-memory.json；仅当前角色卡与会话快照不同时排队重建，任务结束后应用，文件错误保留现有会话并展示错误。当前角色没有启动过 Codex 时不因编辑而启动真实服务。

切换显示角色（ui/sidebar.rs 或资源刷新回退）立即结束旧任务并创建新会话，清理回复、气泡、审批及重启标记，保留未发送草稿；同角色换装、战损与格式变化保持会话。ui/codex.rs 在启动时记录新的角色卡和文件快照。手动重启重新读取默认资源；默认资源编辑不自动热加载。

定向验证：`cargo test --locked role_card` 覆盖结构化默认/覆盖、配置保留与损坏保护、世界书匹配、真实协议分块、设置规则操作及保存/取消/延后重启。相关回归使用 `cargo test --locked backend::codex::tests::`、`cargo test --locked backend::character::tests::` 和 `cargo test --locked ui::tests::`。测试仅用临时资源与模拟服务。Codex 模拟服务统一经 mock_options → launch_options 启动，复用生产进程隔离；不再保留未被使用的 Unix 专用 launch 包装，以保持 Linux all-targets Clippy 无 dead_code。

### 上下文规划 场景与记忆

- `roleplay/context.rs` 定义 PromptBlock、ContextPlan、ContextSettings、SceneState 和 RoleplaySession。静态提示按核心、规则、记忆、场景、分段示例构建；核心身份、性格、关系、用户人设与已启用规则完整保留，其余内容按字符预算选取。超额核心完整保留并在检查页提示。预算统计含标题和本轮背景标记，不等同于模型 token 或完整 Codex 历史预算。
- 世界书扫描本轮输入与最近成功对话（最多 32 条，每条最多 8192 字符）；支持全局/单条深度、优先级、始终启用、持续与冷却，扫描深度 0 关闭关键词扫描。优先级高的条目先分配本轮预算；超额条目记录拒绝原因，不触发持续效果。持续 N 轮指首次触发后的 N 轮，不因重复命中续期；随后冷却 N 轮。默认 Lexington 扫描 6 条，持续 2 轮，冷却 1 轮。
- RoleplaySession 在后端通信线程中使用。接受发送时先保存快照、生成计划；成功结束后将用户输入和按 item ID 去重的最终回复加入最近历史。任务失败或中断恢复快照，被拒绝或初始化前取消的输入不推进轮次。预览克隆会话后规划，不改变历史、轮数与时效，也不调用模型。静态计划只在 thread/start 携带，本轮场景和世界书另行携带；已进入 Codex 历史的旧背景不由本程序删除。
- `roleplay/profile.rs` 的 UserProfile/Memory 将用户人设与记忆独立保存到 config/roleplay-memory.json 的 profiles 分组，按角色隔离；session_card 合并有效档案后启动会话。旧角色卡的 userPersona/memory 只在没有独立档案时兼容读取；旧式 Markdown 角色添加档案后仍保留原人格。候选默认未确认，不加载；仅 confirmed && enabled 的非空记忆进入提示。记忆的来源元数据仅供用户核对，不注入模型。文本编辑会撤销确认；保存失败保留原文件。
- `ui/roleplay_context.rs` 在“角色卡 → 上下文”显示上次实际发送与预览报告，逐块显示选中状态、类别、字符数、作用域及原因，支持查看内容；只检查本软件附加内容。临时场景支持当前活动、风格、尚待完成，应用/清空不重启模型，下轮起使用，新会话清空。预览使用当前已知状态，不预测进行中任务的后续回复。
- `ui/roleplay_memory.rs` 在“用户与记忆”管理独立草稿，支持手动新增或从最近输入创建候选、确认、撤销、启停和删除，通过独立保存按钮应用；保存操作在 viewport 回调之后执行。保存候选/来源等不影响有效提示时不重启，已确认内容变化待当前任务结束重建；关闭或取消丢弃未保存草稿。该固定记忆编辑器不会自动推断事实或生成摘要；自动事件提炼由独立人物记忆页提供。
- 新 Event::Context 通过标准通道回传至 UI，实际与预览分开保存；ContextPlan 的 Debug 不含提示文本，已知当前环境/提供方 API Key 在事件出口统一遮蔽。向量记忆关闭时最近对话仅在内存；启用后成功结束的用户输入/最终回复另外写入历史数据库。临时场景和检查报告不持久化。
- Linux `scripts/package.sh` 写入空配置，并清理旧输出遗留的 roleplay.json、roleplay-memory.json、roleplay-vectors.sqlite3 及 journal/wal/shm；`scripts/test-package.sh` 用临时工程和假构建器验证重复打包不会携带私人配置或向量数据，角色默认资源仍完整。此夹具可在 Git Bash 运行，不代替 Linux 桌面实测。

### 新建角色卡与立绘导入

- 入口为“设置 → 角色卡 → 新建角色卡”，填写名称与身份描述，使用原生多选文件对话框或拖入 PNG/GIF；支持移除所选图片。至少选择一张立绘才可创建，领域接口再次验证；每次 1～32 张，单张最多 64 MiB、总计最多 256 MiB、每张最多 16 × 1024 × 1024 像素。校验格式/尺寸及现有 tools/gif 解码器可读取首帧，不改解码时序或纹理流程，不保证提前验证 GIF 所有后续帧。
- 名称裁首尾空白后作为角色标识；最多 200 字节，拒绝控制字符、路径/Windows 非法文件名字符、隐藏名、尾点/空格及常用系统保留名。内置和个人角色目录按 ASCII 不区分大小写检查冲突，同时拒绝已有角色卡或映射；原文件名与正常/战损后缀保留，单格式内同名文件拒绝导入。
- 每个新角色独立保存在 config/characters/<名称>/card.json 和 png/、gif/，不改 resource 布局或 characters.json，不覆盖 config/roleplay.json。全部图片复制、限额和首帧验证、紧凑 JSON 保存及 sync 完成后才将 .creating-* 临时目录 rename 为角色目录；失败自动清理暂存，扫描忽略隐藏目录。新建卡片通过 RoleCard::load 的既有接口加载；后续编辑保存仍写 roleplay.json 的覆盖分组，身份关系和提示不继承其他角色。
- 文件选择和实际导入在工作线程完成，设置 viewport 回调后才启动；CharacterCreation 的任务通道由 Wife 持有，关闭设置只丢弃未提交草稿，已请求创建仍可完成并刷新角色列表。有原活动角色时保持选择/聊天；原无角色时可显示首次导入的立绘，测试构造不会启动真实 Codex。设置仍打开时转入新卡编辑，可继续设置关系、世界书和规则；以后切换显示角色沿用原有新建 Codex 会话流程。
- Linux 源码与无源码安装的必需依赖检查包含 libdbus-1-3 和 Zenity（本地推理另检查 libstdc++6）：rfd 优先使用 XDG Portal FileChooser，缺少服务/后端时回退 Zenity；`--skip-optional` 不跳过文件选择支持。`python3 scripts/test-dependencies.py` 用假包管理器验证两种安装模式、缺失依赖和已有 Zenity 不重复安装，不访问真实系统依赖。
- 新增个人目录被 Git 忽略，升级安装保留 config；发布包只含空配置，不包含个人卡片/立绘。Linux 重复打包先核对解析后的清理路径位于项目 dist 内，再清除旧生成包的 config/characters，避免旧发布目录中的个人数据进入新包。原生文件选择参考 [rfd 官方说明](https://github.com/PolyMeilex/rfd/tree/0.17.2)；Linux 最小桌面若无可用 Portal/Zenity，仍可拖入文件，Linux x86_64 已在隔离 X11 中通过同一 pick_illustrations 接口实际打开 Zenity 回退并选择 PNG；其他 Portal 后端仍需各桌面验证。

增量验证：roleplay::creation::tests:: 覆盖无图、损坏图、PNG/GIF、非法名称、重名保护、失败无半成品、未知字段/源图保留、编辑覆盖及坏配置拒绝；ui::new_character::tests:: 使用模拟选择结果验证入口、无图禁止、创建后可选择与关闭设置后完成。补测 roleplay::、backend::character::tests::、ui::role_card::tests:: 和 sidebar_switches_damage_and_format_and_refresh_preserves_selection；scripts/test-package.sh 验证重复打包排除个人立绘，均不调用真实模型或原生文件选择器。

### 历史对话向量记忆

- 代码分层：`tools/vector_store.rs` 只实现通用命名空间文档存储与向量查询；`roleplay/vector_memory.rs` 决定角色隔离、记录范围、Embedding 与召回策略；`roleplay/context.rs` 把召回片段纳入本轮预算。backend 只调用会话接口，UI 只编辑角色卡草稿及在 viewport 回调之后请求清空。
- RoleCard 的 camelCase `vectorMemory` 配置：`enabled` 默认 true、`source` 默认 local（可选 remote）、`cpuThreads` 默认 2（1～4），`recallLimit` 默认 3（1～8），`maxDistance` 默认 0.5（余弦距离 0～2，越小越严格）；已有 enabled=false 不会被升级强制开启，未知字段保留。远程字段 baseUrl/model/apiKeyEnv 保留，仅启用 remote 时校验地址/模型/环境变量名；本地模式不读取 Key，不要求地址。旧配置缺少 source 时采用 local。配置不进入角色提示；保存当前有效卡片变化按既有规则重开会话。不会从 Codex/OAuth 读取 Embedding 凭据。
- 数据文件为资源根的 `config/roleplay-vectors.sqlite3`，不提交 Git、不随发布包分发，安装升级保留；向量功能仅在实际启用发送时创建；独立人物事件功能也可创建同一数据库。不会扫描或导入既有 Codex 会话。成功完成后最多取用户及最终助理文本各前 8192 个 Unicode 字符；本地模式按实际 tokenizer 的最多 432 个正文 token 切分，预留发言者标签，再记录发言者和 Unix 时间；远程保留 1000 字符切分。按角色最多保留最新 2000 片段。失败、中断、忙碌拒绝和预览均不归档；历史片段不自动成为已确认的固定记忆，也不会生成摘要。
- 本地调用 tools/embedding 的 LocalEmbedding，按需在通信工作线程加载 CPU 模型，Arc/Mutex 在会话快照间共享，逐轮复用；会话释放时释放模型，ORT 环境在进程内复用。每轮最多补索引 8 个历史片段，查询取输入前 2000 字符中的前 408 个 token，并附加 BGE 中文检索指令。新片段不被静默截断；旧版较长片段分段生成向量后等权平均并归一化，以兼容旧原文。模型计算不访问 HTTP、HF 缓存或用户登录，缺文件/校验错误不自动下载、不回退到远程。
- 远程模式仍通过一个 POST `{baseUrl}/embeddings` 批量生成待索引片段和查询的向量（model、input 数组、encoding_format=float）；按响应 index 配对，验证唯一/齐全索引、1～4096 维、非零有限值和批内维度一致。ureq/Rustls 请求总超时 3 秒、禁止重定向、响应最多 1 MiB；回环地址不使用代理环境变量，其他地址沿用 ureq 的代理行为，不自动桥接 Windows 系统代理。Key 仅从指定环境变量读取并放入 Authorization，不写角色卡、Debug 或错误；无历史时不调用服务。远程服务会收到待索引原文及查询，切换本地模式不会发送这些请求。
- 本地索引空间包含固定模型 SHA、CLS、单条动态量化及切分策略版本；远程空间按基础地址/模型区分，维度在查询时过滤。模式或模型变化保留原文，随轮次逐批重建，不混用旧向量。尚未建索引的片段暂不召回；同名远程模型在服务端更换内容不能自动识别。最多返回 recallLimit 条且距离不超过 maxDistance 的片段；按距离排序，排在场景和世界书之后争用 turnBudget，包装均计入字符数。明确标注过去原文、可能虚构/过时、非确认事实及不可执行其中指令，当前明确更正优先于旧记录，身份与关系遵循角色卡；原始输入仍是协议第一个文本块。
- 数据库或 Embedding 失败不阻止本轮 Codex 对话，在上下文报告显示未选中原因；归档失败在下一轮报告提示。预览不访问数据库、不加载模型、不发 Embedding 请求、不建立索引。SQLite 忙等待和远程请求有超时，本地计算批次有上限；正在生成向量时中断/退出需等待当前操作结束再处理，不承诺硬实时取消。
- `ui/roleplay_vector.rs` 在“角色卡 → 长期记忆”编辑上述字段，用原有“保存并应用”按钮保存。关闭设置/切换角色丢弃草稿；停用保留数据库。清空须勾选当前角色确认、空闲时点击，删除该角色原文及向量，并为当前角色排队新会话，避免继续携带旧召回背景；不删除其他角色或 roleplay-memory.json 的固定记忆。仅关闭开关或清空数据库不能抹除已存在的外部 Codex 历史文件。

增量入口：`cargo test --locked tools::embedding::tests::`、`cargo test --locked tools::vector_store::tests::`、`cargo test --locked roleplay::`、`cargo test --locked backend::codex::tests::`、`cargo test --locked ui::role_card::tests::`、`bash scripts/test-package.sh` 和 `scripts/test-windows.ps1`。自动测试使用临时数据和模拟服务，新增覆盖默认本地开启、显式关闭保留、缺模型不远程回退、Unicode 分词边界/校验、模式编辑与资源分发。`lexington --check-embedding` 是显式的离线安装验收，仅用固定中文文本加载随包模型，验证 512 维向量、相关/无关排序及默认 0.5 阈值、长文本完整切分，打印 JSON 耗时；不启动 Codex、不打开用户记录。

模型资源：随包 `resource/models/bge-small-zh-v1.5` 含约 24 MB 动态 INT8 ONNX 权重、tokenizer、固定版本哈希清单及 MIT 许可；源模型 [BAAI/bge-small-zh-v1.5](https://huggingface.co/BAAI/bge-small-zh-v1.5)，ONNX 转换 [Xenova/bge-small-zh-v1.5](https://huggingface.co/Xenova/bge-small-zh-v1.5/tree/75c43b069aac4d136ba6bc1122f995fedcfd2781)。Windows x64 随包 `resource/runtime/windows-x86_64/onnxruntime.dll` 为官方 ONNX Runtime 1.24.4 CPU 库，来自经 SHA256 校验的官方发行 wheel，仅提取原生库及许可/第三方声明，不携带 Python 运行链；运行库自身约 16 MB。Linux 构建使用 ort 校验下载并静态链接的 CPU 运行库；x86_64 安装版已通过离线检查，无外置 ORT 动态库依赖，系统仍需 libstdc++6。`.gitattributes` 禁止已校验模型 JSON/权重和 Windows 运行库换行转换，避免 Windows checkout 后 SHA 改变。两端打包前检查模型资源完整，Windows 同时检查运行库；所有资源随现有 resource 复制，源码及无源码安装均保留用户配置与数据库。

Windows x64 增量验收：27 项相关 Rust 测试、fmt 和 Clippy 通过；Windows 安装/升级/无源码包夹具与 Linux 打包隐私夹具通过。安装版与 release、模型和 CPU DLL 哈希一致，已有配置逐文件哈希保留；实际 ZIP 核验模型、运行库、许可和空配置齐全。安装版 `--check-embedding` 在仅保留 System32 的 PATH 下成功，固定中文样例本机观测加载约 361 ms、样例处理约 29 ms；这是单次安装检查，不代表任意文本延迟或召回质量基准。Linux x86_64 实机推理已补充验收，见验证入口；其他 Windows 架构尚未验收。

参考酒馆的 [Prompt Manager](https://docs.sillytavern.app/usage/prompts/prompt-manager/)、[世界书](https://docs.sillytavern.app/usage/core-concepts/worldinfo/) 与[摘要可修正设计](https://docs.sillytavern.app/extensions/summarize/)。本实现保留 Codex 管理完整历史，不假设能够照搬任意历史插入位置。

定向验证：`cargo test --locked roleplay::`、`cargo test --locked backend::codex::tests::`、`cargo test --locked ui::tests::`、`cargo test --locked ui::role_card::tests::`，以及 `bash scripts/test-package.sh`。场景包括预算及溢出原因、最近助理消息触发、持续/冷却、预览无副作用、失败/中断回滚、记忆确认与撤销、旧式迁移、规则与设置交互。均使用临时配置和模拟服务，不连接真实模型。

### 可查看的人物事件记忆

- 人物记忆入口在右键菜单，与“设置”平级，使用独立原生窗口；不提供开关或说明文案，成功对话后始终自动提炼。RoleCard 不再定义 characterMemory 配置，旧字段作为未知数据保留但不控制提炼。按用户授权，模型提炼结果直接保存，无逐条确认；不修改原有固定记忆的确认规则，不自动把事件作为事实注入正常模型上下文。
- `ui/codex.rs::poll_codex` 只在正常对话成功完成后调用 queue_character_memory，从实际发送的 last_input 和本轮最终 codex_messages 生成提炼输入；失败、中断、空回复及待重启的旧会话不排队。取输入/回复各前 4000 Unicode 字符，保留角色归属及记录时间。每次最多一个独立提炼 Client、最多 4 项待处理；队列满时跳过该轮并仅保留内部状态，不阻塞正常对话。每项绑定连接快照，连接变化丢弃旧队列/中止旧任务，避免把旧文本发送给新提供方。
- `ui/character_memories.rs::poll_character_memories` 使用通用 Client::start_structured，沿用当前模型和登录/API 配置，额外发起一次模型请求；处理流式/最终结果只写内部缓冲，完成后交给 roleplay 校验保存。提炼成功/失败/超时/交互请求都不写 Wife.text、codex_messages、input、busy 或普通 codex_status；后台状态不在浏览窗口展示，结果仅保存在人物记忆列表，正常聊天会话不包含提炼提示或结果。总计 90 秒后停止，结果缓冲最多 32 KiB；关闭设置或记忆窗口不取消已排队任务。Wife::from_root 不运行此服务，仅真实 Wife::new 打开后台处理，测试不会启动真实模型。
- `backend/codex.rs::Client::start_structured(config, root, instructions, schema, wake)` 是通用结构化任务接口，不负责事件业务。通过 tempfile 在独立临时 cwd 启动现有 stdio app-server，复用原有 API/proxy/密钥遮蔽和进程树清理。initialize 后 config/read 获取有效 MCP 名称，thread/start.config.mcp_servers 使用嵌套对象逐个设置 enabled=false、required=false（键名带点也支持）；不能用带引号的扁平路径，也不能回传含 null 的整份 MCP 配置。关闭 shell、apps/plugins、multi-agent、browser/computer/image、skill_search、tool_suggest、web_search 和 update_plan，thread/start 设置 ephemeral、read-only、never，turn/start 携带 outputSchema 和禁止网络的 readOnly sandboxPolicy。工具或交互请求返回错误并终止提炼；出现非消息/推理的执行条目也停止。会话退出先回收进程，再清理临时目录，不修改正常会话权限或用户 Codex 配置。
- roleplay 提炼指令要求仅提炼值得记住的事件，寒暄/测试/普通问答可返回空数组，每轮最多 3 条。事件包含标题、描述、类别（共同经历/计划与约定/偏好/关系/成就与转折/其他）、发生时间文字、参与者、重要程度 1～3 和视角（用户陈述/角色扮演）；模型不能把计划写成已发生。evidence 必须逐字来自对应原文，用户陈述只可引用用户文本；助手虚构只能标为角色扮演。标题最多 80 字符、描述 1000、依据 500、时间/参与者名称 80、参与者最多 8；非法字段、无依据或非 JSON 结果整批拒绝。仍可能存在模型归纳错误，事件详情提供提炼依据供核对和删除，不宣称事实认证。
- 人物记忆保存在同一 config/roleplay-vectors.sqlite3 的通用 records 表，namespace 为角色，id 为 character-memories；JSON Journal 保存事件与删除标记，未知字段保留。每角色保留最近 256 件事件，最多 2048 个删除指纹；原文、类别、视角、发生时间和记录日组成指纹，用于去重及阻止相同结果删除后立即回写。删除只移除该人物事件及其对应对话副本，不删除原始 Codex 历史、向量 documents 或其他角色。无旧对话/向量片段自动导入，也不自动回溯提炼过去任务。
- 人物记忆窗口使用带原生标题栏、可缩放、置顶的独立 viewport，Embedded 后端用 Foreground Window 回退并注册输入范围；首次打开选择当前角色，可切换浏览其他角色，按记录时间倒序展示可展开的事件卡片，显示重要程度、参与者、发生时间、提炼依据和对应对话；支持关键词搜索、事件类别与只看重要筛选，首批 50 条、手动显示更多和刷新。读取/删除走 roleplay，不调用模型或 Embedding；删除按钮无需额外确认，在 viewport 回调后执行，并记录删除标记。浏览状态独立于角色卡草稿和设置窗口，切换浏览角色清理筛选、重新打开默认当前角色，后台任务仍绑定来源角色；浏览和删除不会重启正常聊天。
- tools/vector_store 新增 load_json/save_json 通用命名空间 JSON 记录接口，单条最多 8 MiB；数据库从版本 1 事务升级到版本 2，仅添加 records 表，不改原文或向量；新数据库也完成此结构初始化。读坏记录拒绝覆盖，高版本或损坏数据库仍报错。私人数据仍由原有数据库路径/发布包排除规则保护，无新增私人文件名。

增量验证：`cargo test --locked roleplay::`、`cargo test --locked tools::vector_store::tests::`、`cargo test --locked structured_background_threads_use_schema_disable_inherited_mcp_and_reject_interactions`、`cargo test --locked ui::character_memories::tests::`、`cargo test --locked ui::role_card::tests::`，以及正常 UI 流式和外部请求排队直接链路。夹具覆盖自动保存、逐字依据、角色隔离、去重/删除不复活、固定提示隔离、旧库保留、结构化临时线程/禁用 MCP/交互拒绝、后台结果不进入聊天、失败/无效结果保护、搜索删除、旧开关不再影响提炼、设置/记忆窗口隔离、顶层菜单入口及连接切换。使用本机原生 CLI + 临时 CODEX_HOME 离线验证嵌套 MCP 禁用和临时只读 thread/start，不发送 turn 或访问真实模型/凭据；生成质量未通过真实模型抽取实测。协议参考 [OpenAI Docs：App Server](https://learn.chatgpt.com/docs/app-server)。

## Development 工作区与开发任务

- `src/development/mod.rs` 与 backend/ui/roleplay/tools 平级，不依赖后端或 egui。集中定义 Workspaces/Workspace/Task/Turn、ExecutionItem/FileChange/Activity、开发指令、Git HEAD 分支读取及私人历史原子存储。界面、选择状态和 Codex 调用编排在 `src/ui/development.rs`；不引入第二套执行器或 WebView。
- Workspaces::add canonicalize 项目目录并去重；route 优先显式 workspace 并验证所有文件仍在目录内，再按已登记目录的最长前缀或文件 Git 根识别，跨项目/无法确定报错。UI 只有空闲时允许切换，路径与历史记录按项目隔离；VS Code 远程/未保存文档不提供本地 workspace。
- 每个项目有任务列表和 currentTask，每个任务保存来源角色、developmentMode、threadId 及多轮输入/回复/执行证据。点击历史只改变浏览目标，继续按钮才切换活动任务；窗口明确显示实际发送目标。新任务新建线程；切换工作区或重启应用恢复当前任务线程。角色/模式不同禁止恢复，模式切换待当前任务结束后清除各项目活动任务；旧任务仍供查看。恢复失败明确显示错误，不宣称成功续聊。
- 默认不注入任何 Lexington 开发指令。设置 → 连接中的 development 模式仅保存 codex.developmentMode；开启才附加 `[Lexington development]` 指令，不替换角色人格、不扩大权限。开启时不使用按角色的向量历史，也不自动提炼开发对话为人物事件；普通聊天路径保持。
- development 窗口为独立可缩放不透明原生 viewport（1040×740，最小 740×500），不受宠物窗口尺寸、透明 pass 或 XShape 限制；嵌入后端回退 Window。顶部为项目工具栏，左侧任务列表，右侧详情与固定输入卡片；统一 14px 正文、留白、浅色分组、中文执行状态与独立代码块。右键入口显示“开发”，内部模块及窗口名保留 development。包含项目名称/路径/分支、文件浏览器选择项目、折叠的手动路径输入、搜索/新建/继续/删除任务、多行编辑及 Ctrl+Enter、可选中文本/代码复制、命令输出/退出码/耗时、计划与红绿 diff、文件的 VS Code 跳转。项目草稿在进程内隔离，用户关闭窗口不终止任务；任务区显式限制高度并取消默认最小滚动高度（min_scrolled_height(0)），输入框限高滚动，Ctrl+Enter 只在编辑框聚焦时发送，并在 TextEdit 消费按键前处理，避免被当作换行吞掉。外部请求在历史中展示选区/来源/路径，不展示协议 JSON；分支检查最多每 5 秒一次。
- 项目选择器通过 tools::file_dialog::pick_project_directory 在工作线程运行，默认定位当前项目，UI 回调后启动，返回路径仍由 Workspaces::add 校验和去重。等待期间不重复打开选择器、不允许手动切换；取消不修改当前项目/草稿，断开显示可重试错误，后台任务中途开始则保留所选目录至空闲后切换。development viewport 使用普通窗口层级，避免遮挡无父句柄的原生目录选择器；关闭 development 窗口后仍消费结果。
- Activity 按 item ID 合并；输出增量可能早于 started，completed 的 aggregatedOutput 替换增量而非再次追加；patch 更新文件变更，turn/diff/updated 保存累计 diff。最终 turn.items 再补齐遗漏通知，回复也按 item ID 替换去重。命令只保留最后 256 KiB 且显示截断；任务 completed 仅表示本轮结束，不推断检查成功，真实结果以退出码与输出为准。
- 私人记录为用户数据目录中的 development.json（--dev 为 development-dev.json），不在项目或发布资源内；版本 2（读取版本 1 时保留旧任务并迁移），32 MiB 上限，损坏/新版本拒绝覆盖，同目录临时文件 sync 后原子替换、Unix 0600。发送/结束立即保存，流式变化至多每 2 秒落盘；恢复时 running 转 interrupted，退出也保存中断。已知环境/提供方 Key 在用户输入副本和所有协议输出进入历史前遮蔽，原始请求仍按用户原文发送。
- 增量测试：`cargo test --locked development` 覆盖 canonical 路由/嵌套项目/跨项目拒绝、分支读取、私密存储/损坏保护/恢复、乱序执行/最终去重/UTF-8 截断；Unix UI 模拟协议覆盖 A→B→A 的真实 thread/resume、任务记录/代码渲染、模式忙碌延期与关闭后的无开发指令新线程。fixture 不连接模型或读取真实凭据。Linux 本机已在隔离 Xephyr + GNOME WM 中验证独立窗口、显式 workspace 外部路由、执行/累计 diff 落盘、真实编辑器输入/Ctrl+Enter 与关闭子窗不退出宠物，并检查测试窗口截图；本次布局增量还实测 Zenity 目录浏览器的取消、当前项目定位、添加/切换新目录及窗口堆叠，并在 740×500 下通过真实键盘输入/Ctrl+Enter 完成发送；截图核对目录选择器未被遮挡、输入区与发送按钮可见。其他 Portal 后端及其他平台的实际窗口行为需对应原生环境验证。

### 诊断、测试管理与项目记忆

- 主应用类型统一为 `ui::Wife`，分文件 `impl Wife` 继续共享 UI 状态，启动为 `Wife::new`、隔离测试为 `Wife::from_root`；角色绘制文件 pet.rs 和原生窗口标题不随 Rust 类型改名。界面页面分别位于 ui/development_diagnostics.rs、development_tests.rs、development_memory.rs；领域模型位于平级 development 的 diagnostics.rs、testing.rs、memory.rs。
- VS Code 1.2.0 按本地 workspace 收集 getDiagnostics 快照，选区/文件发送附带快照，独立命令“发送项目诊断到开发”只同步问题、不触发模型 turn。最多 100 文档/200 问题，诊断快照预算 512 KiB，文本截断保留完整 Unicode 字符，保存严重级别、消息、来源/代码、URI/本地路径、起止行列（1 起算）、编辑器版本、未保存标记与时间；只收集所属项目，不伪造远程路径。空快照清除旧问题，超量显式标记截断。
- development/diagnostics 校验坐标、大小和真实路径（含软链接）的项目归属；接收时记录磁盘时间/长度戳，来源变化的已保存文档不再自动进入背景，未保存文档明确标记版本。诊断页支持搜索/只看错误、手动检查来源、跳转 VS Code 行列及显式分析。快照仅随本地请求同步，不监听剪贴板或后台扫描整个系统。
- 测试定义为 name/argv/timeout，单项目最多 64 项；UI 每行一个参数，不隐式调用 shell。点击运行/重跑失败项后，backend/codex 通过同一 app-server 的 command/exec 执行，不发模型 turn、不创建第二套执行器，结构化后台提炼 Client 拒绝运行独立测试；cwd 为当前工作区，省略 sandboxPolicy/env 以继承本机执行策略，不借无需审批开关扩大权限。processId 关联取消；timeout 为 1～3600 秒，每路输出最多 256 KiB，ACK 的 30 秒握手限制不适用于长时间 command/exec。结果由 exitCode/stdout/stderr/错误与真实耗时判定，取消和重启不记成功；实际返回前不会把命令视为通过。单项目保留最近 20 次记录，配置修改或代码后续变化不改写历史结果，失败分析仅显式请求时使用输出尾部。
- 测试队列绑定项目，串行复用执行通道，与普通模型任务互斥；停止清除后续队列，项目改变取消旧队列。配置保存保留 argv 边界、禁止 NUL/空程序；已知凭据不得直接保存到参数（使用外部环境）。命令结果先遮蔽已知环境/提供方 Key，再保存至私人历史。
- 项目记忆支持手动编辑和从任务回复摘录候选，按项目保存标题/类别/正文/来源、可选依据文件、确认/启用状态。默认候选未经确认；编辑撤销确认，只有已确认且启用的条目可进入背景。依据只能是项目内相对文件，拒绝跨目录/软链接逃逸；最多读取 1 MiB 并记录 SHA256，文件变化或缺失使该条失效，需重新核对。单项目最多 128 条，正文最多 6000 字，本轮记忆预算 10000 字；检查页报告当前的可载入资格及排除原因，不把预览计划冒充已发送的模型上下文。保存先验证并原子持久化，失败保留原数据与编辑草稿；不写仓库 AGENTS.md 或改动角色记忆。
- 上述项目背景仅 developmentMode 开启时以独立 turn/start 文本块附加，用户原始输入始终是第一块。测试命令只作可选入口，不自动运行；诊断注明时间/编辑器版本，历史结果不能当作当前事实。记忆编辑增 revision，载入文本另存哈希 stamp；停用/删除或文件依据变化后新建干净线程，旧 stamp/revision 的任务禁止继续恢复，避免旧 Codex 历史带回失效记忆。关闭模式不附加诊断、测试登记或记忆，普通角色/审批流程保持。
- 私人状态版本 2 为 Workspace 增加 diagnostics/tests/testRuns/memories/memoryRevision，Task 增 memoryRevision/memoryContextStamp；读取版本 1 只补默认值并保留旧线程/对话，未知版本拒绝覆盖。恢复时 running 测试与对话均改为 interrupted。所有配置与记录保留在 development.json / development-dev.json，沿用同目录原子保存和 Unix 0600，不加入发布包。
- 增量验证使用 development 模块过滤器、Codex 协议和 Inbox 过滤器、UI 直接链路与 JS 客户端夹具；覆盖范围包括模式开关/独立背景块、依据变更新线程、损坏存储保护、V1 迁移、快照范围/清空/截断、真实退出码/失败重跑/取消且无聊天 turn。另以原生 Codex + 临时 CODEX_HOME 的离线提供方配置验证 command/exec 继承工作区沙箱、argv/cwd、失败退出/输出及取消，不访问真实模型或凭据。本机隔离 Xephyr 原生验证了诊断纯同步不发模型 turn、测试配置编辑保存/运行/失败重跑、记忆停用/编辑/确认保存，以及 740×500 任务输入发送；检查真实协议输入包含三个项目来源而未自动执行登记测试。另验证退出 app-server 进程组不遗留独立测试子进程。接口参考 [OpenAI Docs: App Server](https://learn.chatgpt.com/docs/app-server)。

## 启动与依赖方向

- `main.rs`：先处理 `--check-transparency` 隐藏 GL 上下文的透明输出验收，再处理 `--check-embedding` 固定文本离线验收和 `--ask-files` / `--ask-json` / `--native-messaging` 无 GUI 入口，再取得 Linux/Windows 普通/开发模式独立实例锁并创建 eframe 原生窗口、选择 X11/Wayland/Win32 后端、加载窗口图标并构造 `ui::Wife`。普通尺寸 500×500，`--dev` 为 800×600。Linux X11/XWayland 使用 Dock 桌面浮层类型，不设置 strut、不占用工作区；避免 GNOME 将透明窗体整块限制在屏幕内，使立绘中心可到屏幕边缘。
- `ui` 可调用 `backend`、`roleplay`、`development` 和 `tools`；`backend` 可消费 `roleplay` 的提示和世界书结果，不依赖 `ui`、egui 或 GPU 纹理；`roleplay` 可调用 `tools` 的通用存储，不依赖 `backend`、UI 或 egui；`tools` 不依赖应用 UI 或业务模块。
- `development` 不依赖 backend/UI/roleplay/egui；backend 仅消费开发指令和执行事件类型，不负责工作区界面。
- `tools/platform.rs` 是平台与窗口框架的适配边界，可依赖 eframe/egui/winit；路径和解码工具不依赖这些框架。
- UI 内通过 `impl Wife` 分文件组织相关交互，必要的共享字段与方法限制在 UI 模块内。不要为了目录分层增加只有转发作用的抽象。

## UI

Windows 同样启用 UI 的 Inbox、原生位置保存/恢复、显示器检测、指针轮询、拖动及输入区域；独立设置窗口初始位置与闹钟中央提醒使用 Win32 显示器矩形，设置按原生按下锚点绝对移动。主窗口启动使用 active=false，不主动抢焦点。浏览器宿主可通过 `lexington-native-host.exe` 别名直接进入协议入口，不能把该别名当 GUI 再次启动。

| 文件 | 职责与主要接口 | 依赖及约束 |
|---|---|---|
| `ui/mod.rs` | 声明 UI 子模块（含天气），对入口导出 `Wife` | 具体组件保持在 UI 模块内部 |
| `ui/app.rs` | 定义 Wife 与 eframe::App，编排资源、主窗、Codex、提醒及独立设置/人物记忆/development viewport；加载 development 工作区，退出时保存任务中断状态 | 测试 from_root 不启动 CLI/监测或访问用户记录；普通/--dev 的位置和任务文件分别隔离。热加载失败保留有效配置；developmentMode 改变等待当前任务结束再新建会话。主窗 input_regions 只包含实际可见控件，独立 viewport 不加入 XShape；主窗最终执行透明颜色转换。窗口位置、角色创建和资源加载约束沿用相关小节 |
| `ui/pet.rs` | `pet_ui` 绘制立绘/默认宠物和阴影，设置宠物、对话、输入层的共同变换，处理圆形命中、点击与拖动；默认宠物几何和变换测试也在此 | 使用 `Picture` 和平台拖动；将当前帧的归一化 alpha 覆盖区域映射到立绘实际位置并应用共同变换，默认宠物按身体轮廓生成输入区域，阴影不拦截点击；返回尺寸、变换和点击结果供 app 编排。容器 450×450，侧栏不参与共同缩放；拖动阈值 5 个逻辑像素；按下时由平台层读取原生屏幕指针/窗口坐标捕获锚点，不使用 egui 可能过时的窗口局部指针 |
| `ui/settings.rs` | 独立 464×390 设置 viewport，常规/连接/打卡/闹钟/天气/角色卡导航；连接页仅展示继承环境状态、development 模式、任务窗口入口及会话重启；保存开关、刷新资源 | 无边框不透明 Dock/置顶，按原生指针锚点拖动，控制区不拖窗，嵌入后端回退 Foreground Window。操作在 viewport 回调后执行；无模型/API/Key 表单或凭据草稿。developmentMode 保存成功后 reload_config，忙碌结束再建立新会话；手动重启为当前项目新建任务，不恢复旧线程。保留未知配置与旧密钥；设置关闭只销毁子窗口 |
| `ui/sidebar.rs` | `sidebar` 绘制七个按钮、悬停提示与动画，报告本帧是否处理点击；↻ 调用 `toggle_damage` 切换同服装正常/战损，▶ 调用 `next_illustration` 在当前状态队列循环；`clock` 将持久化后的手动打卡结果转为对话，仅成功开始计时后排队查询天气，取消不查询，失败可从打卡设置查看 | 用 `damage_counterpart` 判断切换按钮是否可用，缺少同角色/格式/服装配对时禁用；战损时 ↻ 为橙色，悬停提示显示正常/战损切换目标；其他按钮统一使用“下一张立绘”“切换正常/Q版立绘”“切换live2d”“切换角色”，不附加实现说明。显示顺序按动作索引 [0,1,2,3,6,4,5] 排列，保留各动作动画 ID。选择变更后通过 app 重载纹理；按钮 40×40、间隔 10，仅按钮自身（随悬停缩放）加入输入区域、间隙穿透；侧栏点击须阻止同帧外部点击关闭 |
| `ui/dialog.rs` | `say` 初始化消息与打字机状态，`click_pet` 处理预设台词回退/显示全文/关闭，`bubble` 测量和绘制对话及滚动区 | 台词来自 `CharacterState`；气泡为空时，仅在非 busy 且 codex_messages 无非空 AI 文本时才随机读取 texts 台词，等待生成、已有回复被关闭或自动隐藏时均不回退预设台词；AI 显示全文/关闭操作保持。只检查当前轮次的输出，后续失败且未输出的新轮次或新会话仍可回退；使用共享样式与背景纹理。每 50ms 一个字符，按全文保留高度，14px 字号、21px 行高、10/14px 内边距；自动关闭与外部点击由 app 协调；只在对话显示时提交经过层变换的气泡输入区域 |
| `ui/codex.rs` | 启动/恢复工作区会话，消费文本、thread ID、执行事件、完成/失败/审批并更新气泡及任务记录；审批与用户问题仍复用原协议 | 使用选中项目作为运行目录；仅相同角色和 developmentMode 的当前任务允许恢复线程。模式改变清除所有项目活动任务；失败、中断、退出和角色切换准确终结运行中记录。执行事件仅进入运行中的任务；文本按 item ID 去重，任务输出已遮蔽凭据。development 模式不排队人物记忆提炼；development 开发指令与角色指令在后端分开组装 |
| `ui/development.rs` | development 原生任务窗口、项目选择通道、按项目草稿、任务浏览与发送 | 顶部项目栏、左任务列表、右详情和固定编辑卡片；选择项目使用后台原生目录选择器，支持取消与空闲后切换，任务窗口使用普通层级，避免遮挡目录浏览器。内部域与记录在 development 模块；右键菜单显示“开发” |
| `ui/development_diagnostics.rs` | 诊断列表、过滤、来源检查与分析入口 | 只读项目快照；分析在 viewport 回调后发送，模式关闭不自动附加 |
| `ui/development_tests.rs` | 测试配置编辑、运行队列、历史结果及失败分析 | 复用 Client::run_test；模型任务和本地测试互斥，状态绑定项目/run ID，重启保存中断 |
| `ui/development_memory.rs` | 项目记忆草稿、核对/停用/删除、任务摘录候选及来源状态 | 保存先写盘再更新内存；变更记忆新建干净线程，角色记忆独立 |
| `ui/alarm.rs` | `AlarmUi` 管理表单和错误状态；`load_alarms` 加载持久化记录；`alarm_settings_ui` 提供 HH:MM、提示语、每天重复、新增/编辑/删除及启停；`alarm_popup_ui` 每秒检查到期并显示独立原生窗口 | 依赖 AlarmState、clock 本地时间和 paths；弹窗 420×260、无边框不透明置顶，按桌宠所在显示器中央定位（X11 用 RandR，支持负坐标和 DPI），不移动桌宠。X11 使用 Dock 层，普通 AlwaysOnTop 可能被桌宠 Dock 遮挡；映射后 800ms 一次性 Focus/AlwaysOnTop（晚于主窗启动置顶），不复用桌宠 XShape 遮罩。多个到期提醒合并，长文本滚动，固定确认按钮；确认或系统关闭先持久化清除 pending，保存失败保留弹窗。非原生 egui 集成降级为居中 Window 并加入输入区域；Wayland 位置由合成器决定 |
| `ui/clock.rs` | `poll_clock` 消费系统采样、驱动自动确认及提醒；`clock_settings_ui` 展示自动开关、检测状态、起点和编辑；`clock_action` 保存/撤销/手动操作；`clock_notice_ui` 提供修改和撤销入口 | 调用后端 ClockState 和工具 Monitor；自动打卡成功后排队天气查询（忙碌时也排队但不覆盖聊天）；自动提示加入原生输入区域，关闭立即移除；过期采样不确认，自动开关/确认时长热加载会重置候选；加载失败禁用更改并显示错误，避免覆盖损坏记录 |
| `ui/input.rs` | 普通输入和 development 输入共用 send_message；poll_handoff 顺序消费外部请求，校验并切换项目后发送，保留宠物输入草稿；输入框、焦点、加载及错误显示 | 原始用户输入只裁首尾空白，不附加路径；持久化副本由 Client::sanitize 遮蔽。发送成功才建立任务记录；模式重连待处理且空闲时先重连再提交。VS Code workspace 优先，其他文件按登记目录/Git 根路由，跨项目或来源不明不发送到活动项目；尚无工作区的旧入口保留普通聊天行为。忙碌继续排队，无焦点抢占或重复提交；原生悬停、失焦及错误输入约束沿用 UI AGENTS |
| `ui/assets.rs` | `Picture::load/size/texture` 将解码像素上传到 GPU，复用纹理并更新动画帧；`opaque_regions` 保存当前合成帧的归一化 alpha 覆盖条带（最多 256×256 网格，按格内最大 alpha ≥ 8 判断，逐格扫描命中即停止，使用 ceil 反推像素边界以等价保留非整除尺寸细节，不额外常驻 RGBA 帧）；`icons` 加载内置侧栏图标；保留 GIF 抽帧后的像素、循环、跳过帧的局部处置、输入遮罩随显示帧透明度更新、非整除尺寸 alpha 覆盖等价性及纹理复用回归测试 | 调用 `tools::gif::Decoder`，读取当前 RgbaImage 并保持只读；颜色转换和 alpha 区域计算仍在 UI 层，不实现循环策略；立绘/背景使用线性 mipmap。字体和图标的 `include_bytes!` 路径相对 `src/ui/` |
| `ui/transparency.rs` | Transparency::new/paint/destroy 管理主透明窗口的最终 glow 输出转换；screenshot_rgba 还原桌面预乘读回，check 提供无用户数据的真实 GPU 验收 | UI 层独占 GPU 资源；最终屏幕拷贝纹理采用 RGBA8/nearest，避免额外 sRGB 解码和再次滤波。回调排在 Debug 顶层，不注册输入区域，不改 alpha 或原始素材；细节与验证见下节 |
| `ui/style.rs` | `install` 注册字体、浅色主题及全 viewport 的 IME 提交兼容回调；`font`、`bubble_tail`、`draw_bubble`、`draw_shadow` 提供共同绘制规则 | 使用 egui 与 `Picture`；CJK TTC 使用简体中文 face index 2；对话尾尖距左边 28px，输入尾尖居中。气泡 Area 的溢出策略由对话/输入组件保持 `constrain(false)`。egui 0.31 对无预编辑的连续 Commit 沿用旧候选光标，首词后会丢字；未处于预编辑的 Commit 转为 Text，让 TextEdit 按当前光标/选区插入。正常 Enabled/Preedit/Commit 流程保留；组合状态按 viewport 与聚焦控件隔离，失焦或 Disabled 清除，同一 Context 重装样式不重复注册。单/多行回归覆盖连续四次中文候选、跨帧预编辑及 Unicode 选区替换，入口 `cargo test --locked ui::style::tests::` |
| `ui/weather.rs` | `WeatherUi` 保存城市搜索、请求 ID 和待展示天气；`weather_settings_ui/weather_action` 提供搜索、选择保存、停用和主动查看；`queue_clock_weather/poll_weather` 连接手动/自动打卡与对话输出 | 只在打卡成功或用户主动搜索/查看时联网，平时不轮询、不预取；未配置城市维持原打卡消息。工作线程回调只请求重绘；结果按请求 ID、城市、打卡起点及本地日期关联，取消/换城/跨日丢弃，生成中或已有其他消息时延后，错误不回滚打卡；主动查看不要求打卡。显示全文并复用气泡滚动区；测试用夹具验证设置点击、自动/手动触发、闲置不联网、消息避让和失效回复 |
| `ui/tests.rs` | 使用真实 egui 指针和键盘事件验证组件协作：侧栏、正常/战损对应切换、下一张的状态隔离、跨格式保留服装、角色与格式、对话、打卡、输入、外部请求在任务中排队/同一会话多轮/来源路径和行号保留/完成不覆盖草稿、普通/开发窗口悬停立绘唤出输入、点击聚焦输入、原生失焦后草稿/错误隐藏且内容保留、再悬停恢复及生成中显示、Codex 流式回复/失败、配置热加载/错误保留/忙碌延迟重建、设置开关点击/持久化/同会话连续切换、API 表单本机夹具自动填充/读取不落盘/继承保存不复制凭据和模型/滚动保存/密码遮蔽及按住显示与松开或失焦隐藏/非法地址拒绝/成功保存及忙碌延迟重连、空白背景/隐藏输入框不命中与关闭窗口输入区域立即清理、右键菜单关闭项在立绘外持续接收原生输入并触发退出、自动打卡提示撤销、时间编辑、自动开关持久化和热加载、闹钟表单校验、编辑/保存、启停/删除、弹窗外点击不消失和确认持久化、位置保存/恢复/开发模式隔离/无效或越界位置回退及中文排版；真实立绘下的连续中文输入、输入三个字后异步错误仍可继续、长文本水平滚动与 IME 提交；侧栏资源用例统一验证图片加载、格式/正常战损切换，并在每次切换后调用 `refresh` 验证路径、状态和图片加载保留；资源数量与配对完整性统一由 backend/character.rs 检查 | 使用模拟 Codex 子进程和临时配置，不访问真实模型或凭据；GIF 和几何专项测试靠近各自 UI 实现 |

中文输入专项另用隔离 Xephyr、独立 D-Bus/Fcitx5 与 XIM 原生键盘事件验证：旧版连续选词只保留“你好”，修复后主聊天与 development 均分四次选词得到完整“你好世界中文测试”，分别用 Enter / Ctrl+Enter 发送到临时协议桩，核对实际收到的全文；不访问真实模型或用户聊天记录。

## 后端功能

| 文件 | 职责与主要接口 | 依赖及约束 |
|---|---|---|
| `backend/mod.rs` | 声明后端子模块（含天气、外部上下文 handoff 和本机 Codex 配置读取），集中保留配置、审批/自动打卡开关无损保存、确认时长边界及错误保护、扫描的测试及共享临时目录夹具 | 协议测试位于 codex.rs，资源扫描和配置测试不需要外部服务 |
| `backend/handoff.rs` | Request 定义 text/files/source/line 与可选 workspace/diagnostics；校验大小和绝对路径，生成分析提示；Endpoint/Inbox/deliver/native_host 维持本地有界 FIFO 与 ACK | JSON ≤1 MiB、最多 32 路径及 16 待处理；workspace 只能为本地绝对目录，真实路径归属由 development 处理。Unix socket 使用私有目录/0600、flock 与 SO_PEERCRED；Windows 使用用户私有命名管道。ACK 仅表示入队，交付后不重试；无网络监听或剪贴板监听 |
| `backend/config.rs` | Config/CodexConfig 与尺寸、打卡、天气配置；read/load；save_no_approval/save_development_mode/save_auto_clock/save_weather 无损原子保存 | config.json 保持 camelCase；随包 config.json 仅保留 noApproval/developmentMode（默认均 false）；CodexConfig 仅反序列化这两个开关；executable/working_directory 仅运行时使用。旧 executable/workingDirectory/model/useCustomApi/baseUrl/apiKey 忽略且无损保留，不覆盖环境。connection_changed 包含 developmentMode，排除审批开关。保存失败不更新内存，损坏 JSON 拒绝覆盖，同目录同步后原子替换，Unix 0600；尺寸/计时及天气校验保持 |
| `backend/resources.rs` | 定义 `Illustration`（含服装名 `outfit` 和战损标记 `damaged`）；`illustrations` 扫描 resource/illustration 与 config/characters 的PNG/GIF 立绘及 live2d/*/actor.json 模型并解析配对元数据；跳过隐藏暂存目录，`background` 选择首张支持的背景，`texts` 读取非空角色台词 | 标准库文件读取和 serde_json；只解析文件名最后的 `_Normal` / `_Damage` 后缀，ASCII 大小写不敏感；无后缀按正常处理，其完整文件名主体作为服装名。保留 `resource/` 布局，按目录识别角色与格式，拒绝台词路径穿越，不解码图片 |
| `backend/character.rs` | `CharacterState` 持有资源列表、角色、格式、正常/战损状态、选中索引与台词；`filtered` 返回当前角色/格式/状态的队列，`damage_counterpart` 查同服装的相反状态，`toggle_damage` 双向切换；`refresh` 保留当前路径并恢复失效选择，`next_illustration` 循环当前队列，`toggle_format`/`toggle_live2d`/`next_character` 切换格式/角色，`selected_path` 提供路径 | 仅依赖资源扫描模块和标准库；默认 GIF + 正常。配对严格限制同角色、同格式、服装名 ASCII 大小写不敏感，缺少对应版本时不切换；格式切换尽量保留同服装。选取回退顺序为当前格式/当前状态 → 另一格式/当前状态 → 当前格式/另一状态 → 另一格式/另一状态。含现有 15 对资源双向匹配、独立队列循环、后缀大小写、缺失配对及删除恢复测试；只选择路径，纹理和默认宠物由 UI 处理 |
| `backend/codex_config.rs` | LocalCodexConfig::read 只读当前 profile/provider 的 API Key，供输出遮蔽 | 依赖 toml/serde_json；不展示模型/地址/凭据，不保存或覆盖 Codex 配置。支持 env_key、experimental_bearer_token、OpenAI 环境/登录文件中的 API Key，不借用 OAuth tokens。解析错误不带源文本，测试使用临时目录与注入环境 |
| `backend/codex.rs` | Client::start_in_workspace/start_structured/submit/submit_with_context/run_test/interrupt/reply/poll/sanitize；app-server stdio、线程恢复、审批和执行事件 | 本机 0.167.0 Schema；initialize → initialized → thread/start 或 thread/resume。模型/API/登录继承环境与本机配置；LEXINGTON_CODEX_EXECUTABLE 指定程序，Windows 优先使用环境 exe 并拒绝 cmd/bat；无模型/提供方 -c 覆盖。developmentMode 开启才附加 development::INSTRUCTIONS，并禁用按角色向量历史的混入；结构化后台提炼保持独立临时 cwd、只读和禁用工具。thread ID 回传任务；turn/start 明确传原始审批策略或 never，不扩大沙箱。关联 thread/turn/request，ACK 30 秒，流式生成不限时；命令输出、item 生命周期、plan、累计 diff 和 patch 更新转 Activity，agentMessage 只走文本通道。完整环境/提供方 Key 在事件和持久化前遮蔽。Unix 独立进程组、Windows Job 清理整个子进程树，关闭 stdin 后 join 读取；不记录协议原文或凭据 |
| `backend/weather.rs` | `City::validate` 校验选定地点；`Client::start/send/poll` 通过有界通道在单个工作线程中搜索城市和查询三天预报；解析 WMO 天气代码并生成中文文本 | ureq + Rustls HTTPS 直连 Open-Meteo 地理编码/预报接口，不经过 Codex，不依赖 egui；请求总超时 10 秒、禁重定向、响应限制 1 MiB、待处理队列 4。按城市及请求日缓存 30 分钟，更新失败可显示同城同日 6 小时内旧数据并注明；缓存只在内存，含更新时间/时区和来源。退出丢弃队列并回收线程，最多等待当前有超时请求。测试用本地 HTTP 夹具覆盖编码、缓存/换城、超时/非法/超大数据及无损配置保存 |
| `backend/alarm.rs` | `AlarmState::load/save/toggle/delete/poll/dismiss` 管理多个本地时间闹钟、调度与待确认提醒 | 不依赖 UI；时间严格 HH:MM，提示语 1～500 字符；一次性闹钟响后关闭，每天重复按 libc mktime 计算下一本地日期，不能固定加 86400 秒，跳过 DST 不存在时间。设置时间已过时安排下一次；启用重新计算；到期（含休眠/停机错过）每个闹钟只补一次并推进到未来，pending 跨重启保存；同时到期合并，重启不重复。普通/开发记录隔离，0600 临时文件同步后原子替换，保存失败不提交内存，损坏数据拒绝覆盖。含同时触发、重复/错过、重启、校验、编辑启停删除与存储失败测试 |
| `backend/clock.rs` | `ClockState::load/observe/toggle/edit/undo/poll` 管理本地日期的自动确认、手动打卡、时间修正、撤销和一次性下班提醒；`now/local_time` 处理时间戳与本地时间 | 依赖配置、serde、标准库及 tools/time 跨平台 libc 本地时间转换，不依赖 UI。自动开始须持续活跃默认 180 秒，回溯首次操作；空闲满 60 秒、锁屏/不可用样本、采样间隔超过 10 秒、时钟跳变均重置确认，确认终点须有新操作。每天自动一次，手动取消、撤销或提醒后当天不再自动开始；跨本地日期重置。记录立即原子保存，失败不提交内存状态；起点、已处理和已提醒状态跨启动恢复，开发模式隔离；编辑拒绝未来或不存在的本地时间。含模拟时钟、断档、持久化、撤销、编辑和存储错误测试 |

## 工具代码

### Windows 后端与平台接口

显式 .cmd/.bat 配置在启动前拒绝（不区分扩展名大小写），防止 Windows Command 的批处理特殊路径隐式引入 shell/Node；启动参数测试使用含空格与中文的真实工作目录。安装夹具同样覆盖空格/中文源及目标，并从 ZIP 解压后验证无需源码、Rust 或 Python 的基本安装。

独立设置窗口在 Windows 与 X11 共用按下锚点逻辑；Windows screen_pointer/window_at_pointer/move_window 仅定位当前进程同标题的原生顶层窗口，使用指针宽度 WindowId 保存 HWND，并以 SetWindowPos 绝对移动，避免 StartDrag 在跨阈值后丢失最初几像素。

Windows 桌宠映射后清除 APPWINDOW 并设置 TOOLWINDOW，以原生窗口样式补强 eframe 的任务栏隐藏要求；独立设置窗口不复用此样式。

穿透样式每帧与实际 GWL_EXSTYLE 比较后按需更新，不缓存期望布尔值：winit 在焦点/置顶变化时可能重写原生样式，缓存会使无事件透明区域永久失去穿透或控件永久不可点击。

原生 smoke 的穿透断言失败时仅报告窗口几何、样式和缩放，不读取真实配置或协议内容。

windows-smoke 的 --artwork 可复制指定 GIF/PNG 到临时资源根，验证纹理 alpha 路径；默认验证缺图回退。启动焦点先以真实点击激活独立底层窗口再比较，避免操作系统拒绝无输入激活导致误报；设置拖动检查两轴，关闭按钮使用真实点击。

指定资源时用 --pet-point X Y 选择 500×500 逻辑窗口中持续可见且处于拖动圆内的实体点，不能假定 GIF 画布中心有 alpha；边缘拖动将实体点移至四角附近，验证原始偏移与负窗口坐标并保持该点可再次抓取。

Windows 非阻塞字节管道使用 PeekNamedPipe 区分暂时无数据与断开，ACK 后等待客户端关闭再断开服务端，避免丢弃未读确认；独占绑定的 ACCESS_DENIED/PIPE_BUSY 均表示已有实例。Python 协议夹具显式使用 UTF-8，不依赖 Windows 系统代码页。

协议夹具以 samefile 校验工作目录身份，兼容 Windows canonicalize 的扩展路径前缀与 Python getcwd 的普通路径表示，不以字符串相等判断同一目录。

- `backend/handoff_windows.rs`：实现与 Linux 相同的 `Endpoint::bind/start`、`Inbox::poll` 和 `deliver`；管道名包含当前用户 SID 和 normal/dev，DACL 仅当前用户和 SYSTEM，拒绝远程客户端，以 first-pipe-instance 保证独占。非阻塞轮询、有界读写截止时间、16 条 FIFO；ACK 仅入队，发帧后不重发。宿主自动启动同目录 `lexington.exe`，避免别名递归。测试覆盖独占、排队、退出释放和 UI 忙碌/草稿保持。
- `tools/windows.rs`：无 eframe 依赖；`Handle` 负责 Win32 句柄生命周期，`user_sid/wide` 处理身份与 UTF-16，`Job::attach/terminate` 将挂起的子进程加入 kill-on-close 作业后恢复线程。`idle_duration` 仅活动本地控制台且 WTS 未锁定时返回 GetLastInputInfo 的空闲时长，失败保留手动入口；不采集输入内容。
- `tools/proxy.rs`：Windows 专用，无 UI 依赖。`backend/codex.rs::command` 在继承环境模式调用 `inherit_system_proxy`；经 WinHTTP 只读当前用户的系统手动代理，释放三个系统分配的字符串，通过 Command.env 向新建 Codex 子进程补 HTTP_PROXY/HTTPS_PROXY，不修改系统代理、用户 Codex 配置或全局环境。任一代理环境变量（含小写、空值）已存在时完全保留；NO_PROXY 已存在时也不覆盖。支持统一 host:port 或 http=/https= 分项，HTTPS 目标默认仍用 HTTP CONNECT；拒绝含账号密码、查询、片段和路径的代理。无手动代理/仅 PAC 或自动发现时不注入；不执行 PAC。排除项支持字面主机/IP、域名后缀和 IPv4 前缀子网，默认排除本机回环；其他 Windows 通配符和 `<local>` 不能完整映射到 NO_PROXY，不转换。代理变更后须重启会话。纯夹具测试覆盖协议选择、环境优先、无效值拒绝及排除项，不连接模型。
- `tools/time.rs`：`local` 调用 Unix localtime_r / Windows localtime_s，`timestamp` 调用 Unix mktime / Windows CRT _mktime64（libc 未导出 Windows mktime）；打卡与每日闹钟共享本地日历/DST 语义，测试时间戳往返和极值拒绝。
- `tools/platform_windows.rs`：平台边界依赖 eframe/winit/windows-sys；`DragWindow` 读取 HWND、全局指针和客户区原点，以按下锚点和 DPI 阈值绝对移动；置顶不激活，点击才聚焦。EnumDisplayMonitors 主屏优先且支持负坐标；指针轮询按当前 alpha/控件矩形切换 WS_EX_TRANSPARENT/WS_EX_LAYERED，不裁剪绘制区域；实际跨进程穿透及拖动必须以 Windows 原生 smoke 验证，模拟事件不能替代。

| 文件 | 职责与主要接口 | 依赖及约束 |
|---|---|---|
| `tools/mod.rs` | 声明路径、活动监测、解码、原生 Live2D、原生文件选择、时间、平台、Embedding、向量存储及 Windows 代理模块 | 不承载业务状态 |
| `tools/file_dialog.rs` | pick_illustrations 返回 PNG/GIF 路径；pick_project_directory(initial) 返回用户选择的项目文件夹，默认定位有效的 initial；取消返回 None | rfd 0.17.2，仅启用 xdg-portal；Windows 原生文件对话框，Linux 使用 libdbus/XDG Desktop Portal 或 Zenity，不增加 WebView/JS/GTK 编译链。UI 工作线程调用，项目选择在 Linux 同时设置 starting_directory 与带末尾 / 的 file_name，以兼容 rfd 0.17.2 的 Zenity 回退只读取 file_name 的行为；本工具不读图、不创建纹理、不访问角色配置或工作区记录 |
| `tools/embedding.rs` | `LocalEmbedding::load/split/embed_documents/embed_query` 封装离线中文向量推理；`check` 提供固定文本的离线安装检查 | fastembed 7.6.1 关闭默认特性、HF 下载和 GPU；CLS + L2 归一化输出 512 维。动态 INT8 每次仅推理一条序列，避免不同批量量化范围影响向量；tokenizer 关闭截断/填充后按原文 byte offset 切分，模型每次最多 512 token。默认 2 个 CPU 线程，限制 1～4、关闭 ORT spinning/telemetry。Windows 通过绝对路径加载经内置清单 SHA256 校验的 1.24.4 CPU DLL，忽略外部 ORT_DYLIB_PATH；Linux 静态链接。首次需要时加载并检查模型/分词文件哈希，错误只给通用提示、不输出正文或原生错误，不读取角色、数据库或凭据；不做任何网络请求 |
| `tools/vector_store.rs` | `VectorStore::open/put/pending/set_embedding/search/delete/prune/load_json/save_json` 保存命名空间文档、可重建向量和独立 JSON records；`Document` 保存原文/时间，`SearchHit` 返回文档与距离 | rusqlite 0.37 bundled SQLite + 静态 sqlite-vec 0.1.6，无数据库服务/动态扩展/Python/Node 运行依赖。使用普通 SQLite 表和 sqlite-vec 标量余弦距离精确扫描，按命名空间、模型空间和维度过滤后排序；不伪称 ANN。原文和向量同一行，改正文使索引失效，回填必须匹配当前原文；删除/裁剪同时去掉向量，id=None 表示清空该命名空间，同秒记录按插入顺序稳定裁剪。SQLite 事务语义，busy_timeout 250ms、secure_delete=ON，Unix 新文件 0600，版本 2（从版本 1 事务新增 records 表并保留旧向量）；JSON 单条最多 8 MiB，namespace/id 非空且最多 200 字节，解析错误不重置，向量删除不影响 records；损坏、未知或高版本数据库报错而不重置，不记录 SQL/正文或凭据。最多 4096 维且拒绝 NaN/Inf/零向量，通用层无角色、Embedding 或提示逻辑 |
| `tools/paths.rs` | `resource_root` 定位资源根目录；`clock_state_file` / `alarm_state_file` 返回普通/开发模式独立的用户数据记录路径 | 顺序：`LEXINGTON_HOME` → 可执行文件旁存在 `resource/` 的目录 → 当前目录；闹钟（alarms.json / alarms-dev.json）及打卡状态使用 XDG_DATA_HOME，默认 ~/.local/share/lexington；Windows data_root 使用 LOCALAPPDATA/lexington，回退 USERPROFILE/AppData/Local，不能写入资源目录 |
| `tools/activity.rs` | `Monitor::start/poll` 在后台每 2 秒采样系统空闲时间和会话可用性，返回带单调时间戳的 Sample；退出停止并回收线程 | Linux 用 loginctl 排除锁屏、远程和非活动会话，优先 gdbus 调用 Mutter IdleMonitor，原生 X11 可回退 XScreenSaver；Wayland 禁止回退 XWayland 空闲时钟。子进程有超时并清理，UI 不阻塞；不记录键鼠内容；不可用样本重置自动确认，保留手动入口；测试可注入采样 |
| `tools/gif.rs` | `Decoder::load/advance` 返回当前 RgbaImage；支持 GIF/静态图片，控制隔帧播放、时序、循环次数、索引缓存与超限流式回退 | 不依赖 UI，不创建 GPU 纹理，不缓存合成帧。源帧 0、2、4…输出，各跳过帧先独立归一化延迟再合并到前一显示帧，奇数结尾保留，不跨循环合并，总时长不变。首轮缓存索引帧/调色板，96 MiB 预算仅针对单个当前 GIF，超限释放前缀并保持流式；完整缓存回放不重读源文件，循环时清空处置画布。保留透明索引、局部/全局调色板、交错帧及 Keep/Background/Previous：跳过 Keep 直接更新处置画布，跳过 Background 仅清区域，Previous 保留原画布，无需合成/上传被跳过帧的 RGBA。Finite(0) 视为无限，显式有限次数保留，原帧低于 20ms 按 100ms。回归覆盖无源文件回放、缓存超限、奇数/变延迟、跳过帧依赖、有限循环和 UI 纹理/透明遮罩；全量参考像素与合并延迟对比用 cargo test --release --locked bundled_frames_match_reference_decoder_pixel_for_pixel -- --ignored；frame_sampling_performance 用 --release --ignored --nocapture --test-threads=1 比较同等循环的原帧率/抽帧取帧，不含 UI/纹理开销 |
| `tools/platform.rs` | `configure` 设置平台窗口属性；Linux `DragWindow::new/arm/update/configure/focus` 应用 X11 窗口提示和绝对坐标拖动；`window_at_pointer` 从原生指针下窗口层级查找当前进程/指定标题并返回真实位置；`move_window` 对指定子窗口发送经确认的 _NET_MOVERESIZE_WINDOW，无窗口管理器时直接 ConfigureWindow；`position` 获取原生窗口屏幕坐标；`monitors` 返回 RandR 活动显示器矩形（主屏优先、含负坐标，失败回退根窗口尺寸），统一换算逻辑坐标；`screen_pointer` 获取屏幕绝对指针和左键状态供独立设置拖动；`pointer_position` 获取原生窗口局部指针，`set_input_regions` 应用 XShape Input 区域，含 DPI/拖动锚点回归测试 | 使用 eframe，Linux 另依赖 winit/x11rb（shape、randr、screensaver 特性）；XShape ≥ 1.1 时仅更新 Input、不能裁剪 Bounding/绘制区域；逻辑坐标按 DPI 换算后裁到 viewport，缓存相同输入区域以避免重复请求，使用 checked 请求。原生 Wayland 不应用该 X11 遮罩；Windows 导出 platform_windows.rs 实现，其他平台保留无操作配置入口。X11 请求关闭临时连接前执行 `.check()`；按下时同时查询原生屏幕指针与窗口位置捕获锚点，避免窗口移动后 egui 缓存的局部坐标滞后导致贴边重新拖动跳跃；阈值换算 DPI，不累加局部位移；独立设置使用根窗口指针查询实现 Dock 标题栏拖动，不依赖 WM 的 StartDrag |

| `tools/live2d/mod.rs` / `render.rs` / `layered.rs` / `mesh.rs` | Actor 分派婚纱 IK 与分层网格/刚性部件模型，提供原生分层动画、共享轨道控制、移动 alpha 热区与 RGBA 渲染，接口及限制见原生 Live2D 小节 | 仅标准库、serde、image，不依赖 UI/GPU 或 Python；所有角色坐标和动作数据来自模型文件 |

## 外部右键客户端与安装脚本

GUI smoke 使用 LEXINGTON_CODEX_EXECUTABLE 指向不可用程序，并隔离 CODEX_HOME；旧 config.json 的 codex.executable 不再具有测试隔离作用。

- `scripts/install-windows.ps1`：源码或解压包用户级安装，参数 InstallDir/SkipBuild/NoShortcuts/NoIntegrations/NoRestart/Python。只安全关闭目标路径的旧进程，拒绝强杀；同目录临时 exe 经 File.Replace 原子替换并核对 SHA256，PowerShell 5.1 以 NullString 传递无备份参数；保留既有 config/config.json，更新资源与原生宿主别名。HKCU 注册 Explorer 经典菜单及浏览器宿主，Chromium key 计算 ID 与白名单保持一致，WScript 创建快捷方式；不修改 Codex 登录，不自动安装 CLI。
- `scripts/package-windows.ps1`：构建 locked release（SkipBuild 供夹具使用），Python 标准库生成集成产物，ZIP 包携带二进制、资源、安装器及空配置 `{}`；绝不打包源目录中的用户凭据。
- `scripts/test-windows.ps1`：临时目录夹具验证含空格路径的初次/重复安装、资源更新、配置逐字保留、二进制/别名校验、临时文件清理、危险目标拒绝以及 ZIP 无凭据/VSIX 完整性；不写真实注册表或桌面。
- `scripts/windows-smoke.py`：Python 标准库 ctypes 创建独立进程的 Win32 底层窗口，真实指针验证跨进程穿透/立绘及侧栏点击、启动不抢焦点、置顶/任务栏样式、桌宠/设置拖动、四边出屏及重新拖回、关闭重启的位置恢复、release JSON/原生宿主别名帧。临时 LEXINGTON_HOME/LOCALAPPDATA/APPDATA/CODEX_HOME 和不可用 exe 阻止读取真实记录/连接模型；已有桌宠时拒绝运行，串行执行；finally 释放按钮/恢复指针并仅回收所创建的进程。原生指针会短暂操作当前桌面，运行需明确授权；多显示器、锁屏/远程会话及浏览器实际安装仍需单独验证。
- `scripts/install-linux.sh`：非 Linux 主机提前失败并指向 Windows 安装器，避免在 MSYS/Windows 修改 Linux 桌面目录。

| 文件 | 职责与主要接口 | 依赖及约束 |
|---|---|---|
| `integrations/vscode/package.json` / `extension.js` | 1.2.0 选区/文件右键及“发送项目诊断到开发”入口，通过无 shell 子进程 stdin 发送 JSON；本地文档额外携带 getWorkspaceFolder 的本地目录 | 多选文件须属于同一工作区；未保存/远程 URI 不伪造本地目录。扩展只运行于 VS Code 宿主，不增加主程序 Node 链；executable 为集成启动路径，非 Codex 连接配置；默认用户级安装位置，Windows 使用 windowsHide |
| `integrations/browser/background.js` / 两份 manifest | Firefox/Chromium 选区右键菜单，通过 `com.lexington.context` Native Messaging 发送选区及 page/frame URL；file URL 转本地路径，含 Windows file:///C:/ 盘符 | 仅 contextMenus/nativeMessaging 权限，无页面监听/网络服务；超过大小不发送；失败工具栏 ! 与悬停提示；Chromium public key 固定 ID，Firefox ID 固定且正式包需签名；沙箱浏览器桥接仍需平台支持 |
| `integrations/nautilus/send.py` / `lexington_menu.py` | 脚本子菜单入口按 URI 解码本地路径，读取安装位置 JSON 并调用 `--ask-files`；可选 Nautilus 4.0 顶层菜单提供器转交同一脚本 | Python 3 标准库；顶层提供器额外依赖 nautilus-python/gi。不解析 shell、不读剪贴板；失败用 notify-send 提示；最多 32 个本地文件；缺少系统扩展支持仍保留脚本入口 |
| `scripts/integrations.py` | `build` 用标准库生成 VSIX、Firefox XPI 和 Chromium 目录；`install` 安装 Nautilus 脚本/提供器、Native Messaging 宿主与白名单，检测 code 自动安装 VSIX | 以 UTF-8 读取元数据，build 支持 Windows，install 仍仅 Linux。无 npm；Firefox 未签名只可临时加载，浏览器安装需按 integrations/README.md；遵循 XDG 数据/配置路径，不更改浏览器签名策略、不关闭文件管理器 |
| `scripts/install-linux.sh` / `package.sh` | release 构建、保留配置的安装及打包，并带上集成客户端；先调用依赖安装，源码模式构建，发布包模式直接复制二进制且跳过 Rust；安装脚本创建桌面快捷方式、注册菜单/通信桥；发布包提供 install.sh 和安装脚本 | 安装二进制先复制到同目录临时文件再原子替换，避免运行中 ETXTBSY；桌面名 Lexington；升级后按项目约定安全重启旧实例 |
| `scripts/install-dependencies.sh` / `dependencies.py` | Python 引导、Ubuntu/Debian 系统依赖检查/授权安装、源码安装的缺失 Rust 工具链安装，以及缺失 Codex 官方原生包下载安装 | 默认固定已验证 CLI 0.166.0；只在缺失时下载，保留现有 CLI/登录配置，无法运行的现有 CLI 明确报错。原生发行包来自 @openai/codex 官方 registry，SHA-512 完整性校验、安全解包、保留完整 vendor 布局，用户级安装 ~/.local/bin/codex，无 Node/npm；Rust rustup-init 用官方 SHA-256 校验。--runtime-only 跳过构建依赖，--skip-optional 跳过 Nautilus 等可选依赖；系统包调用 sudo/pkexec，可选取消保留脚本菜单，必需依赖失败阻止后续安装；不运行 Codex 登录 |
| `scripts/test-dependencies.py` | 用临时 HOME、下载/包夹具验证缺失安装、已有 CLI 保留、资源布局、完整性/路径穿越拒绝及用户配置保留 | 不访问网络、不写系统目录、不安装真实 CLI，不替代真实平台安装验证 |
| `scripts/context-smoke.py` / `test-integrations.js` | 验证真实 CLI/native framing/Nautilus URI 路径和非法输入隔离；用宿主桩验证 VS Code 选区/路径/远程/未保存及浏览器菜单/来源/失败提示 | 测试不调用模型、不修改剪贴板/用户配置；前者 Python 3，后者开发机 Node；Rust UI 测试另验证 Inbox → Codex 完整队列与草稿保护 |
| `integrations/README.md` | 各客户端安装、适用范围与 native bridge 协议说明 | 记录顶层菜单依赖、浏览器签名与沙箱限制，不宣称覆盖任意软件内部菜单 |

## 新代码放置与文档维护

根 [AGENTS.md](../AGENTS.md) 维护全项目约束；目录规则分别位于 [后端](../src/backend/AGENTS.md)、[界面](../src/ui/AGENTS.md)、[工具](../src/tools/AGENTS.md)、[脚本](../scripts/AGENTS.md)。跨目录任务读取相关规则，模块知识在对应目录维护。

- 视觉、输入、焦点、纹理及窗口界面编排放入 UI；角色卡、角色映射、世界书及提示组装放入 roleplay；配置、资源选择、计时及基本 Codex 会话业务规则放入后端；不涉及业务的路径、解码及平台适配放入工具。
- 新增功能优先扩展所属模块，相关逻辑保持内聚；不要仅因代码只用一次就抽取新的辅助函数或文件。
- 后续任何代码文件改动，须同步检查并更新本文件对应的职责、接口、依赖、行为或约束；新增、删除、移动、重命名文件同时更新目录树。只改内部实现时，也应维护受影响的行为或约束说明。工作规则由根及对应目录 `AGENTS.md` 分层维护；新增可复用知识写入最接近所属模块的 AGENTS.md，仅跨模块通用规则写入根文件，更新前阅读并合并去重，无新增知识不必添加。
- 安装、打包及原生验证脚本仍放在 `scripts/`；原资源布局和配置格式不随 Rust 模块拆分改变。

## 测试夹具

`tests/fixtures/codex-server.py` 用 Python 3 模拟 stdio app-server，供后端与 UI 测试共享；覆盖初始化、Unicode 输入、流式/最终消息去重、同一会话多轮与延迟 ACK、模型/提供方启动参数与 Key 环境/命令行隔离及完整 Key 遮蔽、无需审批/原策略逐轮恢复（含 granular）、沙箱保持、命令/文件审批、提问、未支持请求、中断、失败恢复、崩溃和持有 stdout 的启动器后代清理。还覆盖结构化后台线程、嵌套 MCP 禁用、JSON Schema、无效提炼/失败与交互拒绝；只在测试中通过 `include_str!` 使用，不作为发布版运行依赖；不访问用户 Codex 登录信息或真实模型。

## 验证入口

`cargo test --locked ui::dialog::tests::` 验证预设台词在无 AI 输出时的原有循环、等待 AI 时不抢占、正常回复关闭后反复点击不回退，以及后续无输出失败轮次可恢复回退；使用临时资源和模拟 app-server，不访问真实模型/登录。

### Linux 新功能增量验收（2026-11-09）

从 4cb3c3f 快进到 d834807 后，在 Linux x86_64 上完成以下定向验证；不运行全量测试，不调用真实对话模型，不读取用户记忆或凭据。

- 清理 Codex 已无调用的 Unix 测试包装函数，`cargo fmt --check`、all-targets Clippy 均通过。46 项定向 Rust 测试通过：backend::codex/character、roleplay、tools::embedding/vector_store/live2d、ui::role_card/new_character/character_memories/dialog/transparency 及 assets 的 Live2D 用例；默认忽略的参考帧专项未运行。
- 依赖安装与发布包隐私夹具通过。源码和无源码安装均补齐 libdbus、Zenity 与 libstdc++6；`--skip-optional` 保留必需依赖检查。真实 rfd 文件选择在隔离 Xephyr、禁用可用会话总线时成功经 Zenity 回退选择 PNG；测试不依赖 GTK 开发包，不代表所有 Portal 后端已验证。
- 安装版 `--check-embedding` 通过：固定中文相关/无关排序、512 维向量、完整 Unicode 长文本分块；单次本机加载约 177 ms、样例推理约 25 ms。`ldd` 确认没有外置 ONNX Runtime 动态库依赖；不作为通用性能保证。
- 安装版 `--check-transparency` 的 36 个 GPU 像素校验通过。隔离 Xephyr 使用婚纱 Live2D 运行 native-smoke 的 click-through/clock；实体抓取点显式设为 `--pet-point 400 250`。穿透、实体/侧栏保留输入和打卡入口通过，检查真实 GL 截图。drag 模式另在 Xephyr + GNOME WM 中通过阈值、双轴位移、反向移动及尺寸保持；无 WM 的 X Server 不消费桌宠 EWMH 移动消息，不可据此判定拖动失败。
- 执行 Linux release 安装并核对二进制及资源；保留原安装配置，安全关闭旧桌宠并启动新版，桌面快捷方式指向安装版。

范围限制：原生 Wayland 局部穿透、多显示器混合 DPI、其他架构及 Portal 后端未在本次复测。

### Windows 适配验收（2026-11-08）

本机 Windows x64/MSVC，依赖与 Rust 工具链使用清华镜像；测试使用临时配置和模拟服务，不调用真实模型或读取登录凭据。

- `cargo fmt --check`、`cargo clippy --locked --all-targets -- -D warnings` 通过。
- `cargo test --locked`：65 passed、0 failed、2 ignored；两项默认忽略的 GIF 全资源像素比对与性能专项再用 `cargo test --release --locked -- --ignored` 执行，2 passed。
- `node scripts/test-integrations.js`：VS Code/浏览器来源、路径及未保存/远程文档边界通过；Python 集成产物构建成功。
- `scripts/test-windows.ps1 -Python <python.exe>`：初装、升级、配置逐字保留、二进制/宿主哈希、安全目标拒绝、中文空格路径、安全 ZIP 及无源码/Rust/Python 基础安装通过。
- `cargo build --release --locked` 与完整资源 Windows ZIP 打包通过。用户安装版和原生宿主别名与 release SHA256 一致；桌面/开始菜单、Explorer 命令及五种浏览器本机宿主注册核验通过。
- `scripts/windows-smoke.py`：缺图回退和实际 GIF 均通过；最终安装版使用 `--artwork resource/illustration/lexington/gif/BiKiNi_Normal.gif --pet-point 200 225`，真实跨进程穿透、焦点/置顶/任务栏、双轴桌宠/设置拖动、四边重抓、关闭重启恢复和 JSON/宿主交付通过。

未覆盖范围：多显示器混合 DPI、锁屏/RDP、真实浏览器扩展选区端到端及真实模型请求；Linux 原生回归需要 Linux/WM，不能从本机 Windows 测试推断。`bash scripts/install-linux.sh` 在本机按设计拒绝执行，已使用 Windows 安装入口完成等价 release 同步。Linux 专用依赖安装夹具的 shebang 进程不能在 Windows 运行，不记为通过。浏览器注册/产物测试不等于各浏览器已完成真实端到端安装。

### 测试选择

UI 外部 Inbox 排队测试须先等模拟进程返回首轮流式文本，再发送中断；握手尚未结束时的中断会正确取消未发出的首轮，不能据此假设后续回复从第三轮开始。Windows 中文及空格路径按 JSON 编码后的字符串验证。

默认只运行本次增量及受影响模块/直接依赖链路的测试；仅用户明确要求时运行全量测试。模块或用例过滤器必须按改动选择，以下为入口示例，不要求逐条执行；纯文档改动只检查内容、链接和差异。Rust 改动仍运行格式检查及 Clippy。

```bash
cargo fmt --check
cargo test --locked backend::codex::tests::  # 示例：仅 Codex 后端测试
cargo test --locked sidebar_switches_damage_and_format_and_refresh_preserves_selection  # 示例：单用例
cargo clippy --locked --all-targets -- -D warnings
cargo run --locked -- --dev --screenshot /tmp/lexington.png  # 涉及真实渲染时
```

不带过滤器的 `cargo test --locked` 仅在用户明确要求全量测试时使用。专项 smoke、GIF 参考像素及性能测试按改动影响选择，受影响检查通过且无新问题后不重复运行。

`scripts/settings-smoke.py` 在临时配置/资源根目录下验证设置标题栏和正文空白拖动、按钮手势不拖窗、独立设置不移动桌宠、关闭/重开和失焦、开关点击、1×/1.5× DPI；不调用真实模型，显示默认粉色宠物，宜在隔离显示中运行，退出恢复指针并回收测试进程。

`scripts/native-smoke.py` 验证原生窗口尺寸、层级、拖动与交互；`position` 拖动后正常关闭并再次启动，断言位置精确恢复、透明边距可出屏及尺寸不变；`click-through` 创建真实底层窗口验证透明空白点击传递、立绘及侧栏保留点击；`drag-edges` 用真实指针逐步拖动可见立绘抓取点到四角，断言透明边距出屏、释放不回弹、重新按下拖回和尺寸保持；失败清理时也释放鼠标按钮。涉及窗口、缩放或拖动的改动还应在隔离 X11 桌面运行该脚本；可用模式、环境要求和调用方式见下文。


### 原生与集成专项验证

以下入口仅选择与改动直接相关的脚本/模式。

```bash
python3 scripts/native-smoke.py target/release/lexington /tmp/lexington.png drag
python3 scripts/settings-smoke.py target/release/lexington
python3 scripts/context-smoke.py target/release/lexington
node scripts/test-integrations.js
python3 scripts/test-dependencies.py
```

- native-smoke 模式为 `drag`、`drag-edges`、`click-through`、`position`、`clock`、`input`、`pin`、`idle`，`--production` 验证普通尺寸。需要 libX11/libXtst，优先 Xephyr + 窗口管理器隔离，同一 display 的指针测试必须串行；脚本移动并恢复鼠标。GIF/Live2D 画布或旧 GIF 抓取坐标可能透明；`--pet-point X Y` 可指定原生窗口像素中的实体抓取点（如开发窗口婚纱 Live2D 用 `400 250`），拖动和穿透用例共用此点；验收包括阈值、精确位移、反向移动和尺寸保持，失败也须释放鼠标按钮。
- native-smoke 按 `_NET_WM_PID` 找到窗口后还须等待 X11 `IsViewable`，避免冷启动尚未映射时 SetInputFocus 触发 BadMatch。其 click-through 模式在透明区点击底层测试接收窗后，重新压低该接收窗并抬高桌宠，再验证人物保留输入；GNOME 可能因接收窗被点击改变测试堆叠，不能把底窗遮住桌宠误判为 alpha 穿透。
- 侧栏模式跨启动验证使用独立 `XDG_DATA_HOME`，两次正常退出后检查 `lexington/app.ron` 中的固定模式。
- settings-smoke 用无资源临时根目录显示粉色测试形象，不代表安装资源丢失；覆盖 1×/1.5× DPI。集成和依赖脚本使用夹具，不调用模型或安装真实依赖；Node 仅是 JS 客户端测试的开发依赖。

## 实现补充与兼容细节

以下细节与模块表配合阅读，避免在项目操作规则中重复维护。

- 配置：旧 `llm` 与 Codex 连接字段仅兼容保留，不再用于覆盖；模型、API 和登录由外部 Codex 环境负责。developmentMode 是 Lexington 独立开关，不改变本机 Codex 权限。
- UI：设置两栏为“常规 / 连接 / 打卡 / 闹钟”导航与分组滚动内容，说明用悬停提示；设置拖动在 primary_pressed 时取得 interaction_snapshot 命中 ID，再于锁外 read_response 排除其他 click/drag 控件，禁止锁内重入；无 WM 时直接 ConfigureWindow。气泡/输入局部坐标检查后再平移到窗口中心。字体组合为 Noto Sans CJK TTC 简体 face index 2、Liberation、Noto Sans Math；14px 中文字宽应接近 14px，不额外 FontTweak.scale 放大。
- 立绘与 GIF：`Normal_Normal` 的服装名为 `Normal`；随包 GIF 7 对、PNG 8 对。现有 1024×1024、67 帧 GIF 索引约 67 MiB；完整 RGBA 全库缓存曾导致系统杀进程。LZ4 压缩 RGBA 本机解压慢于索引合成，不作为默认方案；性能测试不代表整程序 CPU/RSS，后者需另行采样。跳过 Keep/Any 帧均更新处置画布，UI 不额外保存合成帧副本。
- 状态文件：打卡为 `$XDG_DATA_HOME/lexington/clock.json` / `clock-dev.json`（默认 `~/.local/share`），闹钟为同目录 `alarms.json` / `alarms-dev.json`，含 alarms/pending。Unix 本地时间使用 libc；打卡提醒后保留起点并立即保存已提醒状态，取消、撤销、午休和重启当天不再自动开始。
- 安装：用 `xdg-user-dir DESKTOP`，桌面文件为 `Lexington.desktop`，迁移小写旧文件，菜单 ID 仍为 `lexington.desktop`；设可执行并尝试可信标记。缺 Rust 时安装最小稳定工具链；发布包 `install.sh` 仅安装运行依赖。Codex 原生包须保留 `bin/codex-code-mode-host`、`codex-resources`、`codex-path`、`codex-package.json` 完整布局后链接 `~/.local/bin/codex`；`--skip-optional` 可跳过可选系统包。


### 天气增量验证

```bash
cargo test --locked backend::weather::tests::
cargo test --locked ui::weather::tests::
```

天气测试使用固定 JSON 和本地 HTTP 夹具，不连接真实天气服务或模型。配置、打卡或设置联动变化再选择对应模块/用例补测。
