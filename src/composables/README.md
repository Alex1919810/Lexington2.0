# 热加载系统使用指南

## 概述

热加载系统提供了一个统一的接口，让任何组件都可以轻松注册自己的热加载功能。当用户点击刷新按钮时，所有已注册的热加载函数都会自动执行。

## 架构

- **HotReloadManager**: 管理所有热加载函数的注册和执行
- **useHotReload**: 组件中使用的 composable，用于注册热加载函数
- **provideHotReload**: 在 App.vue 中提供热加载管理器

## 使用方法

### 1. 在组件中注册热加载

```vue
<script setup>
import { onMounted, onUnmounted } from 'vue'
import { useHotReload } from '../composables/useHotReload'

// 定义你的加载函数
const loadMyConfig = async () => {
  // 你的加载逻辑
  const config = await window.electronAPI.getMyConfig()
  // 更新组件状态
}

// 注册热加载
const { unregister } = useHotReload('我的配置', loadMyConfig)

// 组件挂载时加载
onMounted(() => {
  loadMyConfig()
})

// 组件卸载时取消注册
onUnmounted(() => {
  unregister()
})
</script>
```

### 2. 在 App.vue 中直接注册（适用于非组件代码）

```vue
<script setup>
import { createHotReloadManager, provideHotReload } from './composables/useHotReload'

const hotReloadManager = createHotReloadManager()
provideHotReload(hotReloadManager)

// 直接注册热加载函数
hotReloadManager.register('立绘列表', async () => {
  await loadIllustrations()
})
</script>
```

## 示例

### 示例 1: 对话框配置热加载

```vue
<!-- DialogBubble.vue -->
<script setup>
import { useHotReload } from '../composables/useHotReload'

const loadConfig = async () => {
  const result = await window.electronAPI.getConfig()
  // 更新配置...
}

// 注册热加载
const { unregister } = useHotReload('对话框配置', loadConfig)

onMounted(() => {
  loadConfig()
})

onUnmounted(() => {
  unregister()
})
</script>
```

### 示例 2: 主题配置热加载

```vue
<!-- ThemeManager.vue -->
<script setup>
import { useHotReload } from '../composables/useHotReload'

const loadTheme = async () => {
  const theme = await window.electronAPI.getTheme()
  // 应用主题...
}

const { unregister } = useHotReload('主题配置', loadTheme)

onMounted(() => {
  loadTheme()
})

onUnmounted(() => {
  unregister()
})
</script>
```

### 示例 3: 多资源热加载

```vue
<!-- ResourceLoader.vue -->
<script setup>
import { useHotReload } from '../composables/useHotReload'

const loadResources = async () => {
  await Promise.all([
    loadImages(),
    loadSounds(),
    loadFonts()
  ])
}

const { unregister } = useHotReload('资源文件', loadResources)

onMounted(() => {
  loadResources()
})

onUnmounted(() => {
  unregister()
})
</script>
```

## API 参考

### `useHotReload(name: string, reloadFn: HotReloadFunction)`

在组件中注册热加载函数。

**参数:**
- `name`: 热加载项的名称（用于调试和日志）
- `reloadFn`: 热加载函数，可以是同步或异步函数

**返回:**
- `unregister`: 取消注册的函数

### `createHotReloadManager(): HotReloadManager`

创建热加载管理器实例。

**返回:**
- `HotReloadManager` 对象，包含以下方法：
  - `register(name, reloadFn)`: 注册热加载函数
  - `reload()`: 触发所有已注册的热加载函数
  - `getRegisteredItems()`: 获取已注册项的名称列表

### `provideHotReload(manager: HotReloadManager)`

在 App.vue 中提供热加载管理器，使子组件可以通过 `useHotReload` 访问。

## 注意事项

1. **自动清理**: 组件卸载时会自动取消注册，但建议手动调用 `unregister()` 以确保清理
2. **错误处理**: 热加载函数中的错误会被捕获并记录，不会影响其他热加载项的执行
3. **并行执行**: 所有热加载函数会并行执行，以提高性能
4. **命名规范**: 建议使用清晰、描述性的名称，便于调试

## 调试

在控制台中可以看到热加载的详细日志：

```
[热加载] 注册: 对话框配置，当前共 1 项
[热加载] 注册: 立绘列表，当前共 2 项
[热加载] 开始重新加载 2 项...
[热加载] ✓ 对话框配置 (15ms)
[热加载] ✓ 立绘列表 (120ms)
[热加载] 重新加载完成
```
