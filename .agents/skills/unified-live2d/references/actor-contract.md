# 统一角色数据与运行逻辑

## 最小模型

JSON 使用相对路径，所有动作共享同一 canvas、neutral、root_pivot、channels 和 assets。下面仅演示参数格式，动作可替换，不要求任何角色自动眨眼：

```json
{
  "format": "unified-actor-1",
  "canvas": [480, 720],
  "neutral": "neutral.png",
  "root_pivot": [240, 680],
  "shared_idle": {
    "waveform": "sine_cubed",
    "breathing": {"period": 3, "max_dy_px": 1.2},
    "sway": {"period": 6, "rotation_clockwise_degrees_per_wave": 0.2, "translation_x_px_per_wave": 0.6}
  },
  "channels": {
    "eye_openness": {"default": 1, "range": [0, 1]},
    "hand_lift": {"default": 0, "range": [0, 10]}
  },
  "actions": {
    "blink": {
      "duration": 0.64,
      "tracks": {"eye_openness": [
        {"t": 0, "v": 1, "ease": "smoothstep"},
        {"t": 0.26, "v": 0},
        {"t": 0.36, "v": 0, "ease": "smoothstep"},
        {"t": 0.64, "v": 1}
      ]}
    },
    "lift": {
      "duration": 3.8,
      "tracks": {"hand_lift": [
        {"t": 0, "v": 0, "ease": "smootherstep_squared"},
        {"t": 1.5, "v": 10},
        {"t": 2.1, "v": 10, "ease": "smootherstep_complement_squared"},
        {"t": 3.8, "v": 0}
      ]}
    }
  },
  "assets": {}
}
```

示例周期和幅度来自已验证案例，只是起点。每条轨道从 t=0 开始、到 action.duration 结束，时间严格递增，数值在 channel.range 内，首末等于该通道 default。动作只能写自己的轨道；不需要的通道保持默认。

`actor_core.py` 同时兼容本次 `lexington-parametric-actor-draft-1` 中 close/hold/open 与 rise/hold/fall 的缩写。新模型优先使用通用 tracks，不执行 JSON 中的任何公式字符串。缓动定义：S3=3u²−2u³，S5=10u³−15u⁴+6u⁵；squared 为 S5²，complement_squared 的插值权重为 `1−(1−S5)²`。

## 两类时间

```text
global_time -> shared_idle -> breath_wave, root rotation/translation
global_time - event.start -> local action tracks
```

呼吸/摇摆时钟永不因动作重启。每个局部动作归零时，应与**同一 global_time 的待机**重合；不是和全局 t=0 的世界坐标强行重合。不同周期的预览若要求首末一致，需设计共同的显示周期，不能复制最后一帧掩盖不连续运动。

helper 的 `sample()` 是纯离线采样：返回参数与全局待机值；`queue_event()` 按通道冲突限量排队，独立通道可同时触发。生产控制器要清理已完成事件；历史事件列表仅用于离线预览。真正的动作打断需要从当前值与速度混合，不能直接从默认值重播，更不能淡化两张完整角色图像。

`sine_cubed` 是此次为中性相位缓停而采用的波形，不是通用要求；也支持 sine。低于显示精度的小值可量化为0，数值零点稳定后再检查首末图像，不能以此掩盖可见的动作跳变。

## 图层与资源

- 背景、上臂、躯干前景遮挡、前臂、持物、眼部，按明确合成顺序组织；根变换最后施加一次。
- 关键角色图层默认全 canvas；裁剪图层必须额外带局部矩形/UV/偏移，不能把不同大小纹理都当作全画布。
- PNG/JSON/图集引用留在模型包内；source 参考、遮罩、眼部 atlas 和呼吸网格应一同保存。眼睛 atlas 是局部素材，不是整个角色的一套独立帧。
- 对同一半透明衣服反复 alpha-over 会改变颜色与不透明度；原画遮挡层需明确 replace-under-mask 或正确分层合成，不能用颜色漂移掩盖接缝。
- 呼吸网格不驱动手臂缩放；骨段默认 scale=1。用固定 pole 的两骨 IK，超出可达范围时调整动作或素材，不偷偷拉长胳膊。
- 头发作为共享待机扩展时，保存原画发束层、精确 alpha/分层遮罩、局部轴、角度单位与方向、周期/相位、根部覆盖及当前前景遮挡顺序。参考 [hair.md](hair.md)。`actor_core.py` 内置的 idle 采样只处理呼吸和根摇摆；头发扩展需要参考渲染器或宿主实现，审计工具不会自动证明它已渲染正确。

若交付目标是可复现的素材包，除了文件列表，还要记录各层坐标/锚点、骨骼绑定、图集 UV、遮罩替换与合成算法，或附带已经验证的参考渲染器。只有文本合成顺序与动作参数时应标为 draft；不能承诺任意宿主可直接加载。骨长保持只证明几何约束，不等于像素轮廓始终等粗，仍需查看分层与实际解码帧。

## 点击坐标

1. 把屏幕点逆 viewport/hover/缩放/平移，再逆角色 root 矩阵。
2. 依当前骨骼/物件变换测试局部热点；花束随抬花上移，不能还测旧位置。
3. 再按 priority/前景绘制顺序选区域并触发动作。头部可为椭圆，持物可为多边形。

`actor_core.hit_test()` 支持固定 root、刚性部件矩阵与通道驱动的局部平移；非线性网格热点要由宿主提供更新后的顶点或网格逆映射，不能声称这个简化工具已解决任意变形命中。

热点示例：

```json
{"id":"prop","shape":"polygon","points":[[0,0],[20,0],[20,20],[0,20]],"translation":[0,{"channel":"hand_lift","scale":-1}],"action":"lift","priority":10}
```

## 接入现有原生应用

在现有 UI 渲染层持有角色纹理、网格和动画控制器，业务层不引入 GPU/UI 依赖。使用既有的统一悬浮层或模型根变换，避免两边同时烘焙同类摇摆；鼠标命中采用对应逆变换。此案例的目标是 Rust / eframe / egui / glow，不加入 WebView、Node 或 Python 运行链；离线工具可以生成素材与验证数据。

运行验收应覆盖中途触发、独立通道叠加、同通道重复点击、动作结束回待机与热区随动。模拟触发视频只能证明渲染/参数设计，不替代真实应用交互测试。
