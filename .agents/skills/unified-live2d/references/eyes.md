# 可选眼部通道

仅在用户要求眨眼或模型需要眼部动作时使用；眼睛不自动绑定到手部动作。将局部眼睛合成到模型/头部坐标，再统一施加根摇摆；有头部骨骼运动时，眼部跟随头部。

保留原睁眼画面，生成或整理同画布、同姿态的自然全闭参考。不能用通用光流直接混合两张脸：这曾产生双瞳、睁眼残影和轮廓收缩。原眼白/瞳孔纹理不缩放，仅通过上下眼睑开口逐渐显露。

`scripts/reference_blink.py` 是本次提炼的可选合成器，依赖 numpy、Pillow（调用方）和 opencv-python-headless。构造：

```text
ReferenceBlink(open_rgb, closed_rgb, rig, opened_alpha=open_a, closed_alpha=closed_a)
```

两图 RGB 必须同尺寸；alpha 为同画布的0..1浮点平面。参考颜色保持 straight RGB，配准用独立中性观察图，闭眼几何重采样预乘 alpha。ECC 的失败回退会暴露 warning，必须检查联系图，不能视为配准成功。

`rig` 是每图数据：

| 字段 | 含义 |
| --- | --- |
| canvas_size | 可选 `[width,height]`；标定画布，缩放后同步所有坐标 |
| face_box | `[left,top,right,bottom]`，整数像素边界，右/下不含 |
| registration | `ecc` 或 `none` |
| eyes | 可见眼睛列表；不为被头发完全遮住的眼睛伪造数据 |
| eyes[].poly | 包含睫毛而不吞入头发的局部替换多边形，坐标在输入画布 |
| eyes[].x / upper / lower | 等长、x递增的原睁眼上下眼睑采样 |
| eyes[].closed_search | 本眼闭睫毛的 y 搜索带，不含眉毛/头发 |
| matte | 可选 closed_dark_threshold、open_dark_threshold、open_red_margin；默认仅是已验证深色偏暖睫毛起点，按画风检查 |

`render(openness)` 返回 face_box 内的 RGB；`aperture(spec,openness)` 提供当前显露权重，可在101个开合程度上检查单调。合成时只通过眼部覆盖遮罩替换目标 RGB，保持目标 alpha；不要把整个矩形脸部当成不透明层叠上去。openness=1 时使用原始睁眼像素，或完全跳过补丁。不得将既有案例的眼部坐标应用于其他图片或不同裁剪尺寸。

## 保留的修复

- 用配准后的真实闭眼睫毛形状，避免自己沿眼眶底部画通用弧线造成斜闭眼/笑眼。
- 完整睫毛 matte 保留边角；不对整块眼眶做破坏头发的修补。近完全闭合保留原生闭眼参考，避免反复采样让细边变薄。
- 末段使用原睁眼睫毛，连续收敛到原图，不能在最后一帧突然换回更窄轮廓。
- 闭合线低于粗略下缘时，`upper=min(upper_guide,closed)`、`lower=max(lower_guide,closed)`；开口随 openness 只扩大，避免刚露出的外眼角眼白再被遮掉。
- 预乘重采样缓冲满足 `0≤RGB≤255×alpha`，透明区 RGB 为0；保存 straight-alpha 文件前反预乘，防止白晕或黑边。
- 260/100/280ms 可作为本次舒缓眨眼预设，具体角色速度按用户反馈调整。

可烘焙局部 atlas/遮罩供原生渲染，但它们仍属于同一角色的眼部素材。输出必须检查编码后的闭眼末段、睁眼末段、眼角、透明边缘和颜色；只有源码断言通过不足以证明视觉自然。
