<template>
  <div 
    class="sidebar"
    :class="{ 'always-visible': isAlwaysVisible, 'hover-only': !isAlwaysVisible }"
    :style="sidebarStyle"
  >
    <!-- 固定显示切换按钮 -->
    <button 
      class="sidebar-button pin-button" 
      @click.stop.prevent="handleTogglePin"
      @mousedown.stop.prevent
      @mouseup.stop.prevent
      type="button"
      :title="isAlwaysVisible ? '点击后：鼠标经过才显示' : '点击后：一直可见'"
    >
      <span class="button-icon">{{ isAlwaysVisible ? '📌' : '📍' }}</span>
    </button>
    
    <!-- 刷新按钮 -->
    <button 
      class="sidebar-button refresh-button" 
      @click.stop.prevent="handleRefresh"
      @mousedown.stop.prevent
      @mouseup.stop.prevent
      :disabled="isRefreshing"
      type="button"
      :title="isRefreshing ? '刷新中...' : '刷新列表'"
    >
      <span class="button-icon">↻</span>
    </button>
    
    <!-- 切换按钮 -->
    <button 
      class="sidebar-button switch-button" 
      @click.stop.prevent="handleSwitch"
      @mousedown.stop.prevent
      @mouseup.stop.prevent
      :disabled="illustrations.length === 0"
      type="button"
      :title="illustrations.length === 0 ? '暂无立绘' : '切换到下一个立绘'"
    >
      <span class="button-icon">▶</span>
    </button>
    
    <!-- 格式切换按钮 -->
    <button 
      class="sidebar-button format-button" 
      @click.stop.prevent="handleToggleFormat"
      @mousedown.stop.prevent
      @mouseup.stop.prevent
      type="button"
      :title="currentFormat === 'gif' ? '切换到 PNG 格式' : '切换到 GIF 格式'"
    >
      <span class="button-icon">{{ currentFormat === 'gif' ? 'G' : 'P' }}</span>
    </button>
    
    <!-- 角色切换按钮 -->
    <button 
      class="sidebar-button character-button" 
      @click.stop.prevent="handleSwitchCharacter"
      @mousedown.stop.prevent
      @mouseup.stop.prevent
      type="button"
      :disabled="characters.length === 0"
      :title="characters.length === 0 ? '暂无角色' : `切换角色 (当前: ${currentCharacter || '无'})`"
    >
      <span class="button-icon">👤</span>
    </button>
    
    <!-- 打卡按钮 -->
    <button 
      class="sidebar-button clock-in-button" 
      :class="{ 'clocked-in': isClockedIn }"
      @click.stop.prevent="handleClockIn"
      @mousedown.stop.prevent
      @mouseup.stop.prevent
      type="button"
      :title="isClockedIn ? '点击终止计时' : '上班打卡'"
    >
      <span class="button-icon">⏰</span>
    </button>
  </div>
</template>

<script setup>
import { ref, onMounted, computed, onUnmounted } from 'vue'

const props = defineProps({
  illustrations: {
    type: Array,
    default: () => []
  },
  currentIndex: {
    type: Number,
    default: -1
  },
  isRefreshing: {
    type: Boolean,
    default: false
  },
  currentFormat: {
    type: String,
    default: 'gif',
    validator: (value) => ['gif', 'png'].includes(value)
  },
  characters: {
    type: Array,
    default: () => []
  },
  currentCharacter: {
    type: String,
    default: ''
  },
  isClockedIn: {
    type: Boolean,
    default: false
  }
})

const emit = defineEmits(['refresh', 'switch', 'toggleFormat', 'switchCharacter', 'clock-in'])

// 侧边栏固定显示状态（默认开启，一直可见）
const isAlwaysVisible = ref(true)

// 窗口尺寸（用于响应式更新）
const windowSize = ref({ width: 0, height: 0 })

// 更新窗口尺寸
const updateWindowSize = () => {
  windowSize.value = {
    width: window.innerWidth,
    height: window.innerHeight
  }
}

// 侧边栏样式计算（包含坐标校验）
const sidebarStyle = computed(() => {
  // 立绘宽度450px，居中显示
  // 按钮宽度50px（包括padding）
  // 按钮需要在立绘正左方40px
  // 按钮右边缘 = 立绘左边缘 - 40px = (50% - 225px) - 40px = 50% - 265px
  // 按钮左边缘 = 按钮右边缘 - 按钮宽度 = (50% - 265px) - 50px = 50% - 315px
  const baseLeft = 'calc(50% - 315px)'
  
  // 获取窗口尺寸
  const windowWidth = windowSize.value.width || window.innerWidth
  const windowHeight = windowSize.value.height || window.innerHeight
  
  // 如果窗口尺寸还未初始化，使用默认calc值
  if (windowWidth === 0) {
    return {
      left: baseLeft,
      top: '50%',
      transform: 'translateY(-50%)'
    }
  }
  
  // 侧边栏宽度（包括padding）
  const sidebarWidth = 50
  
  // 计算侧边栏左边缘位置（相对于窗口）
  // calc(50% - 315px) 在窗口宽度的一半减去315px
  const calculatedLeft = windowWidth / 2 - 315
  
  // 校验：确保侧边栏不超出左边界
  let finalLeft = calculatedLeft
  if (calculatedLeft < 0) {
    // 如果超出左边界，调整到距离左边缘10px的位置
    finalLeft = 10
  }
  
  // 校验：确保侧边栏不超出右边界（虽然不太可能，但也要检查）
  if (calculatedLeft + sidebarWidth > windowWidth) {
    finalLeft = windowWidth - sidebarWidth - 10
  }
  
  // 如果位置需要调整，使用固定像素值；否则使用calc
  if (Math.abs(finalLeft - calculatedLeft) > 1) {
    return {
      left: `${finalLeft}px`,
      top: '50%',
      transform: 'translateY(-50%)'
    }
  }
  
  return {
    left: baseLeft,
    top: '50%',
    transform: 'translateY(-50%)'
  }
})

// 从 localStorage 加载设置
const loadSettings = () => {
  try {
    const saved = localStorage.getItem('sidebarAlwaysVisible')
    if (saved !== null) {
      isAlwaysVisible.value = saved === 'true'
    }
  } catch (error) {
    console.warn('加载侧边栏设置失败:', error)
  }
}

// 保存设置到 localStorage
const saveSettings = () => {
  try {
    localStorage.setItem('sidebarAlwaysVisible', String(isAlwaysVisible.value))
  } catch (error) {
    console.warn('保存侧边栏设置失败:', error)
  }
}

// 切换固定显示状态
const handleTogglePin = (event) => {
  if (event) {
    event.preventDefault()
    event.stopPropagation()
    event.stopImmediatePropagation()
  }
  isAlwaysVisible.value = !isAlwaysVisible.value
  saveSettings()
  console.log('侧边栏：切换固定显示状态，当前状态:', isAlwaysVisible.value ? '一直可见' : '鼠标经过才显示')
}

// 窗口大小变化时重新计算位置
const handleResize = () => {
  updateWindowSize()
}

// 组件挂载时加载设置
onMounted(() => {
  loadSettings()
  updateWindowSize() // 初始化窗口尺寸
  window.addEventListener('resize', handleResize)
})

onUnmounted(() => {
  window.removeEventListener('resize', handleResize)
})

const handleRefresh = (event) => {
  if (event) {
    event.preventDefault()
    event.stopPropagation()
    event.stopImmediatePropagation()
  }
  if (props.isRefreshing) {
    console.log('侧边栏：正在刷新中，忽略点击')
    return
  }
  console.log('侧边栏：刷新按钮被点击')
  emit('refresh')
}

const handleSwitch = (event) => {
  if (event) {
    event.preventDefault()
    event.stopPropagation()
    event.stopImmediatePropagation()
  }
  if (props.illustrations.length === 0) {
    console.warn('侧边栏：没有立绘可以切换')
    return
  }
  // 切换到下一个立绘（同格式），循环切换
  let nextIndex = props.currentIndex + 1
  // 如果当前索引为-1（未选择）或超出范围，切换到第一个
  if (props.currentIndex < 0 || nextIndex >= props.illustrations.length) {
    nextIndex = 0
  }
  console.log('侧边栏：切换到下一个立绘，当前索引:', props.currentIndex, '下一个索引:', nextIndex, '立绘名称:', props.illustrations[nextIndex]?.name, '格式:', props.illustrations[nextIndex]?.type)
  emit('switch', nextIndex)
}

const handleToggleFormat = (event) => {
  if (event) {
    event.preventDefault()
    event.stopPropagation()
    event.stopImmediatePropagation()
  }
  console.log('侧边栏：切换格式按钮被点击，当前格式:', props.currentFormat)
  emit('toggleFormat')
}

const handleSwitchCharacter = (event) => {
  if (event) {
    event.preventDefault()
    event.stopPropagation()
    event.stopImmediatePropagation()
  }
  if (props.characters.length === 0) {
    console.warn('侧边栏：没有角色可以切换')
    return
  }
  console.log('侧边栏：切换角色按钮被点击，当前角色:', props.currentCharacter)
  emit('switchCharacter')
}

const handleClockIn = (event) => {
  if (event) {
    event.preventDefault()
    event.stopPropagation()
    event.stopImmediatePropagation()
  }
  console.log('侧边栏：打卡按钮被点击，当前状态:', props.isClockedIn ? '已打卡' : '未打卡')
  emit('clock-in')
}
</script>

<style scoped>
.sidebar {
  position: fixed;
  /* 
   * 位置计算说明：
   * - 立绘宽度450px，居中显示，立绘左边缘在 50% - 225px
   * - 按钮宽度50px（包括padding）
   * - 按钮需要在立绘正左方40px
   * - 按钮右边缘 = 立绘左边缘 - 40px = (50% - 225px) - 40px = 50% - 265px
   * - 按钮左边缘 = 按钮右边缘 - 按钮宽度 = (50% - 265px) - 50px = 50% - 315px
   * 注意：按钮 z-index 设置为 10000，位于立绘（z-index: 1）上方，即使重叠也能正常点击
   * 注意：实际位置通过 sidebarStyle computed 属性动态计算，包含边界校验
   */
  width: 50px;
  z-index: 10000;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  padding: 10px 5px;
  -webkit-app-region: no-drag !important;
  pointer-events: auto !important;
  isolation: isolate;
  will-change: transform;
  /* 确保侧边栏可以接收所有鼠标事件 */
  touch-action: manipulation;
  transition: opacity 0.3s ease, transform 0.3s ease;
}

/* 固定显示模式：一直可见 */
.sidebar.always-visible {
  opacity: 1;
}

/* 鼠标悬停模式：默认半透明，鼠标经过时完全显示 */
.sidebar.hover-only {
  opacity: 0.2;
}

.sidebar.hover-only:hover {
  opacity: 1;
}

.sidebar-button {
  width: 40px;
  height: 40px;
  padding: 0;
  background: rgba(100, 150, 255, 0.8);
  color: #fff;
  border: none;
  border-radius: 8px;
  cursor: pointer;
  font-size: 18px;
  font-weight: 500;
  transition: all 0.2s ease;
  display: flex;
  align-items: center;
  justify-content: center;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.2);
  -webkit-app-region: no-drag !important;
  pointer-events: auto !important;
  position: relative;
  z-index: 10001;
  flex-shrink: 0;
  touch-action: manipulation;
  user-select: none;
  /* 确保按钮可以接收点击事件，即使与立绘重叠 */
  isolation: isolate;
}

.sidebar-button:hover:not(:disabled) {
  background: rgba(100, 150, 255, 1);
  transform: scale(1.1);
  box-shadow: 0 4px 12px rgba(100, 150, 255, 0.4);
}

.sidebar-button:active:not(:disabled) {
  transform: scale(0.95);
}

.sidebar-button:disabled {
  opacity: 0.5;
  cursor: not-allowed;
  transform: none;
}

.button-icon {
  display: block;
  line-height: 1;
}

/* 固定显示按钮的特殊样式 */
.pin-button {
  background: rgba(150, 100, 255, 0.8) !important;
}

.pin-button:hover:not(:disabled) {
  background: rgba(150, 100, 255, 1) !important;
}

/* 打卡按钮的特殊样式 */
.clock-in-button {
  background: rgba(100, 150, 255, 0.8) !important;
}

.clock-in-button:hover:not(:disabled) {
  background: rgba(100, 150, 255, 1) !important;
}

.clock-in-button.clocked-in {
  background: rgba(76, 175, 80, 0.8) !important;
}

.clock-in-button.clocked-in:hover:not(:disabled) {
  background: rgba(76, 175, 80, 1) !important;
}
</style>
