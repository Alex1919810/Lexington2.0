<template>
  <div class="visual-wifi-container">
    <VisualWifi :x="visualWifiX" :y="visualWifiY" :imagePath="visualWifiImagePath" />
    <Sidebar 
      :illustrations="illustrations"
      :current-index="currentIllustrationIndex"
      :is-refreshing="isLoading"
      @refresh="refreshIllustrations"
      @switch="switchIllustration"
    />
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue'
import VisualWifi from './components/VisualWifi.vue'
import Sidebar from './components/Sidebar.vue'

const visualWifiX = ref(0)
const visualWifiY = ref(0)
const visualWifiImagePath = ref('') // 立绘图片路径，默认为空则显示默认 CSS 动画
const illustrations = ref([]) // 立绘列表
const currentIllustrationIndex = ref(-1) // 当前选中的立绘索引
const isLoading = ref(false) // 加载状态

// 加载立绘列表
const loadIllustrations = async () => {
  if (!window.electronAPI || !window.electronAPI.scanIllustrations) {
    console.warn('electronAPI.scanIllustrations 不可用')
    return
  }
  
  isLoading.value = true
  try {
    const result = await window.electronAPI.scanIllustrations()
    if (result.success) {
      const oldIndex = currentIllustrationIndex.value
      const oldPath = oldIndex >= 0 && oldIndex < illustrations.value.length 
        ? illustrations.value[oldIndex]?.path 
        : null
      
      illustrations.value = result.illustrations
      
      // 如果有立绘
      if (illustrations.value.length > 0) {
        // 尝试保持当前选中的立绘（通过路径匹配）
        if (oldPath) {
          const matchingIndex = illustrations.value.findIndex(ill => ill.path === oldPath)
          if (matchingIndex >= 0) {
            currentIllustrationIndex.value = matchingIndex
            visualWifiImagePath.value = illustrations.value[matchingIndex].path
          } else {
            // 如果找不到匹配的，选择第一个
            switchIllustration(0)
          }
        } else if (currentIllustrationIndex.value === -1) {
          // 如果之前没有选中，自动选择第一个
          switchIllustration(0)
        } else if (currentIllustrationIndex.value >= illustrations.value.length) {
          // 如果当前索引超出范围，选择最后一个
          switchIllustration(illustrations.value.length - 1)
        }
      } else {
        // 如果没有立绘，清空当前选择
        currentIllustrationIndex.value = -1
        visualWifiImagePath.value = ''
      }
    } else {
      console.error('加载立绘列表失败:', result.error)
    }
  } catch (error) {
    console.error('加载立绘列表异常:', error)
  } finally {
    isLoading.value = false
  }
}

// 刷新立绘列表
const refreshIllustrations = async () => {
  console.log('App.vue：刷新立绘列表')
  await loadIllustrations()
}

// 切换立绘
const switchIllustration = (index) => {
  console.log('App.vue：切换立绘，索引:', index, '立绘列表长度:', illustrations.value.length)
  if (index >= 0 && index < illustrations.value.length) {
    currentIllustrationIndex.value = index
    const illustration = illustrations.value[index]
    // 处理图片路径
    if (illustration.path) {
      console.log('切换立绘:', illustration.name, '路径:', illustration.path)
      visualWifiImagePath.value = illustration.path
    } else {
      console.warn('立绘路径为空:', illustration)
    }
  } else {
    console.warn('索引超出范围:', index, '立绘列表长度:', illustrations.value.length)
  }
}

onMounted(() => {
  // 初始化位置
  visualWifiX.value = 0
  visualWifiY.value = 0
  
  // 加载立绘列表
  loadIllustrations()
})
</script>

<style scoped>
.visual-wifi-container {
  width: 100%;
  height: 100%;
  overflow: visible;
  user-select: none;
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  /* 移除整个容器的拖动，改为只在圆形区域内拖动 */
  -webkit-app-region: no-drag;
}

/* VisualWifi 组件内部已经移除了 drag，改为只在圆形区域内拖动 */
.visual-wifi-container :deep(.visual-wifi) {
  -webkit-app-region: no-drag;
  pointer-events: auto;
  /* 确保立绘区域不会阻止按钮点击 - 通过 z-index 控制层级 */
  z-index: 1;
}

/* 侧边栏区域禁用拖拽，允许点击 */
.visual-wifi-container :deep(.sidebar) {
  -webkit-app-region: no-drag !important;
  pointer-events: auto !important;
  z-index: 10000 !important;
  cursor: default !important;
  /* 创建新的 stacking context，确保按钮在立绘之上 */
  position: fixed;
  isolation: isolate;
}

.visual-wifi-container :deep(.sidebar-button) {
  -webkit-app-region: no-drag !important;
  pointer-events: auto !important;
  z-index: 10001 !important;
  cursor: pointer !important;
  /* 确保按钮可以接收点击事件 */
  position: relative;
  isolation: isolate;
}

.visual-wifi-container :deep(.sidebar *),
.visual-wifi-container :deep(.sidebar-button *) {
  -webkit-app-region: no-drag !important;
  pointer-events: auto !important;
}
</style>
