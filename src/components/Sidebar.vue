<template>
  <div 
    class="sidebar"
  >
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
  </div>
</template>

<script setup>
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
  }
})

const emit = defineEmits(['refresh', 'switch'])

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
  // 切换到下一个立绘，循环切换
  let nextIndex = props.currentIndex + 1
  // 如果当前索引为-1（未选择）或超出范围，切换到第一个
  if (props.currentIndex < 0 || nextIndex >= props.illustrations.length) {
    nextIndex = 0
  }
  console.log('侧边栏：切换到下一个立绘，当前索引:', props.currentIndex, '下一个索引:', nextIndex, '立绘名称:', props.illustrations[nextIndex]?.name)
  emit('switch', nextIndex)
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
   */
  left: calc(50% - 225px);
  top: 50%;
  transform: translateY(-50%);
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

.refresh-button {
  background: rgba(100, 200, 100, 0.8);
}

.refresh-button:hover:not(:disabled) {
  background: rgba(100, 200, 100, 1);
  box-shadow: 0 4px 12px rgba(100, 200, 100, 0.4);
}

.switch-button {
  background: rgba(255, 150, 100, 0.8);
}

.switch-button:hover:not(:disabled) {
  background: rgba(255, 150, 100, 1);
  box-shadow: 0 4px 12px rgba(255, 150, 100, 0.4);
}

.button-icon {
  display: block;
  line-height: 1;
}
</style>
