# Lexington 项目接入

通用制作流程仍以技能正文为准。本文件只说明当前项目的接入位置，不把婚纱素材的坐标、动作或周期套用到其他角色。

- 先读 [模块文档](../../../../docs/MODULES.md) 的“原生 Live2D 风格角色”“桌面透明合成与白边修复”，修改代码前再读相应模块的 AGENTS.md。
- 婚纱原生格式是 `lexington-native-actor-1`，示例为 `resource/illustration/lexington/live2d/WeddingDress_Normal/actor.json`。模型目录名与同一换装 PNG/GIF 的前缀对应；正常/战损使用最终 `_Normal` / `_Damage` 后缀。个人角色使用 `config/characters/<角色>/live2d/`。
- 通用技能中的 `unified-actor-1` / `lexington-parametric-actor-draft-1` 是制作、预览和审计契约，与原生运行格式不同。`scripts/audit_actor.py` 检查前两者，不用于直接审计原生 actor.json，也不证明应用内交互或最终桌面透明合成正确。
- 仓库根目录的 `scripts/import-live2d.py` 用于已提供的婚纱 v4 原包格式转换，不是任意模型转换器。新角色须根据实际分层、锚点和动作契约生成原生资源；不得执行素材包里的未知脚本。Python/NumPy/Pillow/OpenCV 仅用于离线制作，不进入 Rust 应用运行链。
- 业务层 `src/backend/resources.rs` / `character.rs` 只发现、配对和选择资源；`src/tools/live2d/` 处理轨道、移动热区及 RGBA；`src/ui/assets.rs` / `pet.rs` 持有纹理、共享时钟和屏幕坐标逆变换。局部点击不重启待机，拖动不触发动作。
- alpha 边缘须检查真实原生窗口，不能只对照离屏 PNG。`src/ui/transparency.rs` 在采样和混合之后统一转换桌面预乘表示；不要裁掉发丝、提高 alpha 阈值或改坏源图来掩盖合成白边。
- 按改动选择增量验证；原生透明链路可运行 `cargo run --locked -- --check-transparency`，局部交互可筛选 `cargo test --locked live2d_sidebar_and_transformed_body_clicks_trigger_only_local_actions`。原生 WM/指针验证遵守脚本目录约定，不把模拟事件当作实际穿透或拖动验收。


## 分层角色与正式交付

- Bikini 使用 `lexington-layered-actor-1`，示例为 `resource/illustration/lexington/live2d/BiKiNi_Normal/actor.json`，由 `layered.rs` / `mesh.rs` 渲染。固定部件、连续臂网格和可选固定 IK 来自模型，不复用婚纱坐标。`breath_body_only` 限定躯干呼吸；部件 idle 使用普通正弦、共享全局时钟，不套用躯干 sin³ 包络。
- `front_mask` 声明帽沿前后归属；静态发根位于 background 时，`background_above_depth` 使后帽沿位于根层之后，前帽沿仍覆盖头部。渲染、移动部位命中和非交互发根的遮挡必须一致，不能点击透过头发触发后帽沿。
- `live2d-drafts/` 是 Git 忽略的本地临时制作目录。生成提示、配准、ownership、预览可在其中保留，但正式模型只提交 actor.json 引用的 PNG/JSON 与说明；构建、常规测试和已发布模型的快照验收不能依赖草稿存在。未引用的旧分区/补图留在草稿，不随正式模型提交。
- 仓库根 `scripts/build-bikini-live2d-v2.py` 是该服装的离线制作入口，依赖本地草稿输入，默认只输出草稿；审核后使用 `--publish`。缺输入时明确报错，不从当前运行图反推旧分区。没有制作源仍可直接编译、运行和验证已提交的正式模型。
- 运行 `cargo test --locked live2d` 覆盖模型、移动热区和 UI 点击；按改动补充具体用例。`bikini_v2_review_frames` / `bikini_native_reference_frames` / `bikini_continuous_seam_frames` 是默认忽略的原生 release 快照/连续渲染入口，读取正式模型，输出至忽略的 `.tools/`。本项目安装、提交和推送授权仍以根 AGENTS.md 与当前用户指令为准，技能不额外授予权限。
