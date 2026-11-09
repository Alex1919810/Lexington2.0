Flower 正常版 · AI 补全分层模型

来源：../../png/Flower_Normal.png，保留源图，未修改 PNG/GIF。
格式：lexington-layered-actor-1，512×512；Rust 原生运行，无 Python 运行依赖。
这是本项目 Live2D 风格模型，不是 Cubism moc3。

制作方法：内置 imagegen 分别生成完整身体底板、完整头发、左右臂，以及
帽子/花束/手环/飘带/花园配饰图集。生成画布实际为 1254×1254；离线在
预乘 alpha 中配准，以互斥 ownership 恢复原画可见像素。AI 内容只补入
原遮挡后方，避免重画脸、服装或加宽原轮廓。正式文件仅保留运行引用。
完整生成源、提示词、审核图保存在忽略的 live2d-drafts/Flower_Normal。
离线制作入口 scripts/build-flower-live2d.py；默认草稿，--publish 发布。
制作依赖 NumPy/OpenCV/Pillow 及本地生成源，正式运行和测试不需要草稿。

选择：侧栏 L 进入 Live2D，▶ 切到 Flower；从 Flower PNG 进入 L 可同装切换。
点击花束或持花臂：持花臂与花束以肩为轴一起轻摆（-2.2°）。
左右均按屏幕方向；屏幕左侧持花臂的内肘皮肤完整归入手臂层，背发位于其后。
点击右侧手臂/手环：手臂和手环一起轻动（1.8°）。
点击可见草帽：帽子轻倾（-1.8°）。
点击蓝色蝴蝶飘带：飘带轻摆（2.6°）。
点击独立蝴蝶：三只蝴蝶分别绕自身中心转动（9°），不是振翅逐帧动画。
持续待机：躯干呼吸、轻微根摇摆、背发 6 秒 ±2°、花瓣小幅摆动。
脸和前发保持原画；没有增加眨眼。双臂为小幅整体刚性运动，不是肘部 IK。
独立动作可叠加，同动作最多排队一次；拖动不触发点击，待机时钟不重置。

验证入口：cargo test --locked flower_layers
及 live2d_sidebar_and_transformed_body_clicks_trigger_only_local_actions。
原生连续预览：cargo test --release --locked flower_native_review_frames -- --ignored
输出 .tools/flower-review，读取本目录正式模型，不依赖制作草稿。
