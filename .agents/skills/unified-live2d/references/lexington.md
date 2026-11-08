# Lexington 项目接入

通用制作流程仍以技能正文为准。本文件只说明当前项目的接入位置，不把婚纱素材的坐标、动作或周期套用到其他角色。

- 先读 [模块文档](../../../../docs/MODULES.md) 的“原生 Live2D 风格角色”“桌面透明合成与白边修复”，修改代码前再读相应模块的 AGENTS.md。
- 原生运行格式是 `lexington-native-actor-1`，示例为 `resource/illustration/lexington/live2d/WeddingDress_Normal/actor.json`。模型目录名与同一换装 PNG/GIF 的前缀对应；正常/战损使用最终 `_Normal` / `_Damage` 后缀。个人角色使用 `config/characters/<角色>/live2d/`。
- 通用技能中的 `unified-actor-1` / `lexington-parametric-actor-draft-1` 是制作、预览和审计契约，与原生运行格式不同。`scripts/audit_actor.py` 检查前两者，不用于直接审计原生 actor.json，也不证明应用内交互或最终桌面透明合成正确。
- 仓库根目录的 `scripts/import-live2d.py` 用于已提供的婚纱 v4 原包格式转换，不是任意模型转换器。新角色须根据实际分层、锚点和动作契约生成原生资源；不得执行素材包里的未知脚本。Python/NumPy/Pillow/OpenCV 仅用于离线制作，不进入 Rust 应用运行链。
- 业务层 `src/backend/resources.rs` / `character.rs` 只发现、配对和选择资源；`src/tools/live2d/` 处理轨道、移动热区及 RGBA；`src/ui/assets.rs` / `pet.rs` 持有纹理、共享时钟和屏幕坐标逆变换。局部点击不重启待机，拖动不触发动作。
- alpha 边缘须检查真实原生窗口，不能只对照离屏 PNG。`src/ui/transparency.rs` 在采样和混合之后统一转换桌面预乘表示；不要裁掉发丝、提高 alpha 阈值或改坏源图来掩盖合成白边。
- 按改动选择增量验证；原生透明链路可运行 `cargo run --locked -- --check-transparency`，局部交互可筛选 `cargo test --locked live2d_sidebar_and_transformed_body_clicks_trigger_only_local_actions`。原生 WM/指针验证遵守脚本目录约定，不把模拟事件当作实际穿透或拖动验收。
