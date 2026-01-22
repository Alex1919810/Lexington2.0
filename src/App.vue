<template>
  <div class="visual-wifi-container">
    <VisualWifi 
      :x="visualWifiX" 
      :y="visualWifiY" 
      :imagePath="visualWifiImagePath"
      :dialogText="dialogText"
    />
    <Sidebar 
      :illustrations="filteredIllustrations"
      :current-index="currentFilteredIndex"
      :is-refreshing="isLoading"
      :current-format="currentFormat"
      :characters="characters"
      :current-character="currentCharacter"
      @refresh="refreshIllustrations"
      @switch="switchIllustration"
      @toggle-format="toggleFormat"
      @switch-character="switchCharacter"
    />
  </div>
</template>

<script setup>
import { ref, onMounted, computed, watch } from 'vue'
import VisualWifi from './components/VisualWifi.vue'
import Sidebar from './components/Sidebar.vue'
import { createHotReloadManager, provideHotReload } from './composables/useHotReload'

const visualWifiX = ref(0)
const visualWifiY = ref(0)
const visualWifiImagePath = ref('') // 立绘图片路径，默认为空则显示默认 CSS 动画
const illustrations = ref([]) // 立绘列表
const currentIllustrationIndex = ref(-1) // 当前选中的立绘索引（在所有立绘中的索引）
const isLoading = ref(false) // 加载状态
const currentFormat = ref('gif') // 当前选择的格式：'gif' 或 'png'
const currentCharacter = ref('') // 当前选择的角色
const dialogText = ref('你好！这是一段测试对话文字。点击拖动区域可以看到对话框显示效果。') // 对话框文本，默认为空（可以设置为空字符串来隐藏对话框）

// 创建并提供热加载管理器
const hotReloadManager = createHotReloadManager()
provideHotReload(hotReloadManager)

// 注册立绘列表的热加载
hotReloadManager.register('立绘列表', async () => {
  await loadIllustrations()
})

// 获取所有角色列表
const characters = computed(() => {
  const charSet = new Set()
  illustrations.value.forEach(ill => {
    if (ill.character) {
      charSet.add(ill.character)
    }
  })
  return Array.from(charSet).sort()
})

// 根据当前角色和格式过滤立绘列表
const filteredIllustrations = computed(() => {
  if (!currentCharacter.value) {
    return []
  }
  return illustrations.value.filter(ill => 
    ill.character === currentCharacter.value && ill.type === currentFormat.value
  )
})

// 当前角色和格式立绘列表中的索引
const currentFilteredIndex = computed(() => {
  if (currentIllustrationIndex.value < 0) return -1
  const currentIll = illustrations.value[currentIllustrationIndex.value]
  if (!currentIll || 
      currentIll.type !== currentFormat.value || 
      currentIll.character !== currentCharacter.value) {
    return -1
  }
  return filteredIllustrations.value.findIndex(ill => ill.path === currentIll.path)
})

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
        // 如果没有当前角色或当前角色不存在，选择第一个角色
        if (!currentCharacter.value || !characters.value.includes(currentCharacter.value)) {
          if (characters.value.length > 0) {
            currentCharacter.value = characters.value[0]
          }
        }
        
        // 尝试保持当前选中的立绘（通过路径匹配）
        if (oldPath) {
          const matchingIndex = illustrations.value.findIndex(ill => ill.path === oldPath)
          if (matchingIndex >= 0) {
            const matchedIll = illustrations.value[matchingIndex]
            currentIllustrationIndex.value = matchingIndex
            visualWifiImagePath.value = matchedIll.path
            // 更新格式和角色
            currentFormat.value = matchedIll.type || 'gif'
            if (matchedIll.character) {
              currentCharacter.value = matchedIll.character
            }
          } else {
            // 如果找不到匹配的，选择当前角色和格式的第一个
            selectFirstOfCurrentFormat()
          }
        } else if (currentIllustrationIndex.value === -1) {
          // 如果之前没有选中，自动选择当前角色和格式的第一个
          selectFirstOfCurrentFormat()
        } else if (currentIllustrationIndex.value >= illustrations.value.length) {
          // 如果当前索引超出范围，选择当前角色和格式的最后一个
          selectLastOfCurrentFormat()
        }
      } else {
        // 如果没有立绘，清空当前选择
        currentIllustrationIndex.value = -1
        visualWifiImagePath.value = ''
        currentCharacter.value = ''
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
  console.log('App.vue：触发所有热加载项刷新')
  // 触发所有已注册的热加载项（包括立绘列表）
  await hotReloadManager.reload()
}

// 选择当前角色和格式的第一个立绘
const selectFirstOfCurrentFormat = () => {
  const filtered = filteredIllustrations.value
  if (filtered.length > 0) {
    const firstIll = filtered[0]
    const index = illustrations.value.findIndex(ill => ill.path === firstIll.path)
    if (index >= 0) {
      currentIllustrationIndex.value = index
      visualWifiImagePath.value = firstIll.path
    }
  } else {
    // 如果当前格式没有立绘，尝试切换到另一个格式
    if (currentCharacter.value) {
      const otherFormat = currentFormat.value === 'gif' ? 'png' : 'gif'
      const otherFormatIlls = illustrations.value.filter(ill => 
        ill.character === currentCharacter.value && ill.type === otherFormat
      )
      if (otherFormatIlls.length > 0) {
        currentFormat.value = otherFormat
        const firstIll = otherFormatIlls[0]
        const index = illustrations.value.findIndex(ill => ill.path === firstIll.path)
        if (index >= 0) {
          currentIllustrationIndex.value = index
          visualWifiImagePath.value = firstIll.path
          return
        }
      }
    }
    currentIllustrationIndex.value = -1
    visualWifiImagePath.value = ''
  }
}

// 选择当前角色和格式的最后一个立绘
const selectLastOfCurrentFormat = () => {
  const filtered = filteredIllustrations.value
  if (filtered.length > 0) {
    const lastIll = filtered[filtered.length - 1]
    const index = illustrations.value.findIndex(ill => ill.path === lastIll.path)
    if (index >= 0) {
      currentIllustrationIndex.value = index
      visualWifiImagePath.value = lastIll.path
    }
  } else {
    currentIllustrationIndex.value = -1
    visualWifiImagePath.value = ''
  }
}

// 切换立绘（在同格式的立绘中切换）
const switchIllustration = (filteredIndex) => {
  const filtered = filteredIllustrations.value
  console.log('App.vue：切换立绘，过滤后索引:', filteredIndex, '过滤后列表长度:', filtered.length)
  if (filteredIndex >= 0 && filteredIndex < filtered.length) {
    const illustration = filtered[filteredIndex]
    // 找到该立绘在所有立绘中的索引
    const fullIndex = illustrations.value.findIndex(ill => ill.path === illustration.path)
    if (fullIndex >= 0) {
      currentIllustrationIndex.value = fullIndex
      // 处理图片路径
      if (illustration.path) {
        console.log('切换立绘:', illustration.name, '路径:', illustration.path, '格式:', illustration.type)
        visualWifiImagePath.value = illustration.path
      } else {
        console.warn('立绘路径为空:', illustration)
      }
    } else {
      console.warn('找不到立绘:', illustration)
    }
  } else {
    console.warn('过滤后索引超出范围:', filteredIndex, '过滤后列表长度:', filtered.length)
  }
}

// 切换格式（gif <-> png），只在当前角色内切换
const toggleFormat = () => {
  const newFormat = currentFormat.value === 'gif' ? 'png' : 'gif'
  console.log('App.vue：切换格式，从', currentFormat.value, '到', newFormat, '角色:', currentCharacter.value)
  currentFormat.value = newFormat
  
  // 切换到当前角色新格式的第一个立绘
  selectFirstOfCurrentFormat()
}

// 切换角色
const switchCharacter = () => {
  if (characters.value.length === 0) {
    console.warn('没有可用的角色')
    return
  }
  
  const currentIndex = characters.value.indexOf(currentCharacter.value)
  let nextIndex = currentIndex + 1
  if (nextIndex >= characters.value.length) {
    nextIndex = 0
  }
  
  const newCharacter = characters.value[nextIndex]
  console.log('App.vue：切换角色，从', currentCharacter.value, '到', newCharacter)
  currentCharacter.value = newCharacter
  
  // 切换到新角色的第一个立绘（当前格式）
  selectFirstOfCurrentFormat()
}

// 监听格式变化，如果当前立绘不是新格式，自动切换
watch(currentFormat, (newFormat) => {
  if (currentIllustrationIndex.value >= 0) {
    const currentIll = illustrations.value[currentIllustrationIndex.value]
    if (currentIll && (currentIll.type !== newFormat || currentIll.character !== currentCharacter.value)) {
      selectFirstOfCurrentFormat()
    }
  }
})

// 监听角色变化，如果当前立绘不是新角色，自动切换
watch(currentCharacter, (newCharacter) => {
  if (currentIllustrationIndex.value >= 0) {
    const currentIll = illustrations.value[currentIllustrationIndex.value]
    if (currentIll && currentIll.character !== newCharacter) {
      selectFirstOfCurrentFormat()
    }
  }
})

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
