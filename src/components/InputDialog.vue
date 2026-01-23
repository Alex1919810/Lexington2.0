<template>
  <div
    v-if="visible"
    ref="inputDialogRef"
    class="input-dialog"
    :style="{ ...dialogStyle, opacity: dialogOpacity }"
    @mouseenter="isHovered = true"
    @mouseleave="isHovered = false"
  >
    <div class="input-dialog-content" :style="mergedDialogContentStyle">
      <div v-if="isLoading" class="loading-state">
        <div class="loading-spinner"></div>
        <span>正在思考...</span>
      </div>
      <div v-else-if="error" class="error-state">
        <span class="error-text">{{ error }}</span>
      </div>
      <div v-else class="input-wrapper">
        <input
          ref="inputRef"
          v-model="inputText"
          type="text"
          class="input-field"
          placeholder="输入消息..."
          @keydown.enter="handleSubmit"
          @keydown.esc="handleClose"
          @focus="isFocused = true"
          @blur="isFocused = false"
        />
      </div>
    </div>
    <div class="input-dialog-tail"></div>
  </div>
</template>

<script setup>
import { ref, computed, watch, onMounted, onUnmounted, nextTick } from 'vue'

const props = defineProps({
  visible: {
    type: Boolean,
    default: false
  },
  position: {
    type: Object,
    default: () => ({ x: 0, y: 0 })
  },
  offsetX: {
    type: Number,
    default: undefined
  },
  offsetY: {
    type: Number,
    default: undefined
  }
})

const emit = defineEmits(['submit', 'close'])

const inputText = ref('')
const isLoading = ref(false)
const error = ref('')
const inputDialogRef = ref(null)
const inputRef = ref(null)
const backgroundImage = ref('')
const isFocused = ref(false)
const isHovered = ref(false)

// 窗口尺寸（用于边界校验）
const windowWidth = ref(typeof window !== 'undefined' ? window.innerWidth : 450)
const windowHeight = ref(typeof window !== 'undefined' ? window.innerHeight : 450)

// 配置相关
const dialogWidth = ref(250)
const dialogMaxHeight = ref(80)
const dialogOffsetX = ref(0) // 水平居中，不需要偏移
const dialogOffsetY = ref(20) // 显示在立绘底部下方，默认偏移20px
const calculatedHeight = ref(null)

// 计算对话框样式（包含边界校验）
const dialogStyle = computed(() => {
  const centerX = props.position.x > 0 ? props.position.x : 225
  const centerY = props.position.y > 0 ? props.position.y : 225
  const imageHeight = props.position.imageHeight || 0
  const offsetX = props.offsetX !== undefined ? props.offsetX : dialogOffsetX.value
  const offsetY = props.offsetY !== undefined ? props.offsetY : dialogOffsetY.value
  
  // 使用响应式窗口尺寸
  const winWidth = windowWidth.value
  const winHeight = windowHeight.value
  
  // 输入框的尺寸
  const inputWidth = dialogWidth.value
  const inputHeight = calculatedHeight.value || dialogMaxHeight.value || 70 // 默认高度约70px（包含padding）
  
  // 计算初始位置
  // 水平居中：使用立绘中心X坐标，transform会居中
  let left = centerX + offsetX
  // 垂直位置：在立绘底部下方（立绘中心Y + 立绘高度的一半 + 偏移量）
  const imageBottomY = centerY + (imageHeight / 2)
  let top = imageBottomY + offsetY
  
  // 边界校验和调整
  // 由于使用了 translate(-50%, 0)，输入框的中心在 left 位置
  const halfWidth = inputWidth / 2
  const padding = 10 // 留一些边距，避免紧贴边界
  
  // 左边界检查：确保输入框左边缘不超出窗口
  if (left - halfWidth < padding) {
    left = padding + halfWidth
  }
  
  // 右边界检查：确保输入框右边缘不超出窗口
  if (left + halfWidth > winWidth - padding) {
    left = winWidth - padding - halfWidth
  }
  
  // 上边界检查：确保输入框顶部不超出窗口
  if (top < padding) {
    top = padding
  }
  
  // 下边界检查：确保输入框底部不超出窗口
  if (top + inputHeight > winHeight - padding) {
    top = winHeight - padding - inputHeight
  }
  
  return {
    left: `${left}px`,
    top: `${top}px`,
    transform: 'translate(-50%, 0)', // 水平居中
    width: `${dialogWidth.value}px`
  }
})

// 计算对话框内容样式（包含背景图片）
const dialogContentStyle = computed(() => {
  const style = {}
  if (backgroundImage.value) {
    style.backgroundImage = `url(${backgroundImage.value})`
    style.backgroundSize = 'cover'
    style.backgroundPosition = 'center'
    style.backgroundRepeat = 'no-repeat'
    style.backgroundColor = 'transparent'
  }
  return style
})

// 合并对话框内容样式（包含计算的高度）
const mergedDialogContentStyle = computed(() => {
  const style = { ...dialogContentStyle.value }
  if (calculatedHeight.value !== null) {
    style.height = `${calculatedHeight.value}px`
  }
  return style
})

// 加载输入框背景图片（从 resource/background/input 加载）
const loadInputBackground = async () => {
  if (window.electronAPI && window.electronAPI.scanInputBackgrounds) {
    try {
      const result = await window.electronAPI.scanInputBackgrounds()
      if (result.success && result.backgrounds && result.backgrounds.length > 0) {
        backgroundImage.value = result.backgrounds[0].path
      } else {
        backgroundImage.value = ''
      }
    } catch (error) {
      console.error('加载输入框背景失败:', error)
      backgroundImage.value = ''
    }
  }
}

// 加载配置
const loadConfig = async () => {
  if (window.electronAPI && window.electronAPI.getConfig) {
    try {
      const result = await window.electronAPI.getConfig()
      if (result.success && result.config && result.config.dialog) {
        dialogWidth.value = result.config.dialog.width || 250
        dialogMaxHeight.value = result.config.dialog.maxHeight || 80
        // 输入框始终水平居中，不需要offsetX
        dialogOffsetX.value = 0
        // 输入框显示在立绘底部下方，使用配置的offsetY或默认20px
        dialogOffsetY.value = result.config.dialog.offsetY ? result.config.dialog.offsetY : 20
      }
    } catch (error) {
      console.error('加载配置失败:', error)
    }
  }
}

// 处理提交
const handleSubmit = async () => {
  const text = inputText.value.trim()
  if (!text) {
    return
  }

  if (isLoading.value) {
    return
  }

  isLoading.value = true
  error.value = ''

  try {
    if (window.electronAPI && window.electronAPI.callLLMAPI) {
      const result = await window.electronAPI.callLLMAPI(text)
      if (result.success && result.text) {
        // 清空输入框
        inputText.value = ''
        // 发送提交事件，传递API返回的文本
        emit('submit', result.text)
      } else {
        error.value = result.error || 'API调用失败'
      }
    } else {
      error.value = 'API不可用'
    }
  } catch (err) {
    error.value = `调用API时发生错误: ${err.message}`
  } finally {
    isLoading.value = false
  }
}

// 处理关闭（输入框始终显示，关闭只是重置状态）
const handleClose = () => {
  if (isLoading.value) {
    return // 加载中不允许关闭
  }
  inputText.value = ''
  error.value = ''
  // 输入框始终显示，不需要emit close事件
  // emit('close')
}

// 处理点击外部事件（输入框始终显示，不需要点击外部关闭）
// const handleClickOutside = (event) => {
//   if (inputDialogRef.value && !inputDialogRef.value.contains(event.target)) {
//     // 检查点击是否在拖动区域内
//     const dragCircle = document.querySelector('.drag-circle')
//     if (dragCircle && !dragCircle.contains(event.target)) {
//       handleClose()
//     }
//   }
// }

// 监听 visible 变化（输入框始终显示，但需要处理状态重置）
watch(() => props.visible, async (newVal) => {
  if (newVal) {
    calculatedHeight.value = dialogMaxHeight.value
    // 输入框始终显示，不需要自动聚焦，避免干扰用户
    // await nextTick()
    // if (inputRef.value) {
    //   inputRef.value.focus()
    // }
  } else {
    // 如果隐藏，重置状态
    inputText.value = ''
    error.value = ''
    isLoading.value = false
    calculatedHeight.value = null
  }
})

// 监听位置变化，确保输入框始终在正确位置
watch(() => props.position, () => {
  // 位置变化时不需要特殊处理，computed会自动更新
}, { deep: true })

// 计算输入框的透明度（鼠标悬停、获得焦点或加载中时显示）
const dialogOpacity = computed(() => {
  return isFocused.value || isHovered.value || isLoading.value ? 1 : 0
})

// 监听窗口大小变化，更新窗口尺寸以触发位置重新计算
const handleWindowResize = () => {
  if (typeof window !== 'undefined') {
    windowWidth.value = window.innerWidth
    windowHeight.value = window.innerHeight
  }
}

onMounted(() => {
  loadConfig()
  loadInputBackground()
  // 监听窗口大小变化
  if (typeof window !== 'undefined') {
    window.addEventListener('resize', handleWindowResize)
  }
})

onUnmounted(() => {
  // 移除窗口大小变化监听
  if (typeof window !== 'undefined') {
    window.removeEventListener('resize', handleWindowResize)
  }
})
</script>

<style scoped>
/* 输入对话框气泡样式 */
.input-dialog {
  position: absolute;
  z-index: 101;
  pointer-events: auto;
  -webkit-app-region: no-drag;
  transition: opacity 0.3s ease;
}

.input-dialog-content {
  background-color: rgba(255, 255, 255, 0.95);
  border-radius: 12px;
  padding: 10px 14px;
  font-size: 14px;
  line-height: 1.5;
  color: #333;
  box-shadow: 0 2px 10px rgba(0, 0, 0, 0.15);
  box-sizing: border-box;
  min-height: 50px;
  display: flex;
  align-items: center;
}

.input-wrapper {
  width: 100%;
}

.input-field {
  width: 100%;
  border: none;
  outline: none;
  background: transparent;
  font-size: 14px;
  color: #333;
  padding: 0;
  font-family: inherit;
}

.input-field::placeholder {
  color: #999;
}

.loading-state {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  color: #666;
  font-size: 13px;
}

.loading-spinner {
  width: 16px;
  height: 16px;
  border: 2px solid rgba(0, 0, 0, 0.1);
  border-top-color: #333;
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.error-state {
  width: 100%;
}

.error-text {
  color: #e74c3c;
  font-size: 13px;
  word-wrap: break-word;
  word-break: break-word;
}

.input-dialog-tail {
  position: absolute;
  top: -8px;
  left: 50%;
  transform: translateX(-50%);
  width: 0;
  height: 0;
  border-left: 8px solid transparent;
  border-right: 8px solid transparent;
  border-bottom: 8px solid rgba(255, 255, 255, 0.95);
  filter: drop-shadow(0 -2px 2px rgba(0, 0, 0, 0.1));
  /* 与输入框保持一致的透明行为 */
  opacity: inherit;
}
</style>
