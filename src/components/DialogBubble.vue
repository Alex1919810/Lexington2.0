<template>
  <div 
    v-if="visible && text"
    ref="dialogRef"
    class="dialog-bubble"
    :style="dialogStyle"
  >
    <div class="dialog-content" :style="mergedDialogContentStyle">
      {{ displayedText }}
    </div>
    <div class="dialog-tail"></div>
  </div>
</template>

<script setup>
import { ref, computed, watch, onUnmounted, onMounted } from 'vue'

const props = defineProps({
  visible: {
    type: Boolean,
    default: false
  },
  text: {
    type: String,
    default: ''
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
  },
  shouldIgnoreClickOutside: {
    type: Function,
    default: () => false
  }
})

const emit = defineEmits(['close'])

const displayedText = ref('')
const isTyping = ref(false)
const typingTimer = ref(null)
const dialogRef = ref(null)
const backgroundImage = ref('')

// 配置相关
const dialogWidth = ref(200)
const dialogMaxHeight = ref(120)
const dialogOffsetX = ref(100)
const dialogOffsetY = ref(50)
const calculatedHeight = ref(null)

// 计算对话框样式
const dialogStyle = computed(() => {
  const centerX = props.position.x > 0 ? props.position.x : 225
  const centerY = props.position.y > 0 ? props.position.y : 225
  // 优先使用 props 传入的偏移值，如果没有则使用配置值
  const offsetX = props.offsetX !== undefined ? props.offsetX : dialogOffsetX.value
  const offsetY = props.offsetY !== undefined ? props.offsetY : dialogOffsetY.value
  
  return {
    left: `${centerX + offsetX}px`,
    top: `${centerY + offsetY}px`,
    transform: 'translate(-100%, 0)',
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
    // 有背景图片时，移除默认背景色，让图片显示
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

// 加载对话框背景图片
const loadDialogBackground = async () => {
  if (window.electronAPI && window.electronAPI.scanDialogBackgrounds) {
    try {
      const result = await window.electronAPI.scanDialogBackgrounds()
      if (result.success && result.backgrounds && result.backgrounds.length > 0) {
        // 使用第一张图片作为背景
        backgroundImage.value = result.backgrounds[0].path
        console.log('对话框背景已加载:', backgroundImage.value)
      } else {
        // 没有背景图片，不设置背景
        backgroundImage.value = ''
        console.log('未找到对话框背景图片')
      }
    } catch (error) {
      console.error('加载对话框背景失败:', error)
      backgroundImage.value = ''
    }
  } else {
    console.warn('electronAPI.scanDialogBackgrounds 不可用')
    backgroundImage.value = ''
  }
}

// 加载配置
const loadConfig = async () => {
  if (window.electronAPI && window.electronAPI.getConfig) {
    try {
      const result = await window.electronAPI.getConfig()
      if (result.success && result.config && result.config.dialog) {
        dialogWidth.value = result.config.dialog.width || 200
        dialogMaxHeight.value = result.config.dialog.maxHeight || 120
        dialogOffsetX.value = result.config.dialog.offsetX || 100
        dialogOffsetY.value = result.config.dialog.offsetY || 50
        console.log('对话框配置已加载:', { 
          width: dialogWidth.value, 
          maxHeight: dialogMaxHeight.value,
          offsetX: dialogOffsetX.value,
          offsetY: dialogOffsetY.value
        })
      }
    } catch (error) {
      console.error('加载配置失败:', error)
    }
  } else {
    console.warn('electronAPI.getConfig 不可用')
  }
}

// 计算文本所需高度
const calculateTextHeight = (text) => {
  if (!text) return null
  
  // 创建临时测量元素
  const measureElement = document.createElement('div')
  measureElement.style.position = 'absolute'
  measureElement.style.visibility = 'hidden'
  measureElement.style.width = `${dialogWidth.value}px`
  measureElement.style.padding = '10px 14px'
  measureElement.style.fontSize = '14px'
  measureElement.style.lineHeight = '1.5'
  measureElement.style.wordWrap = 'break-word'
  measureElement.style.wordBreak = 'break-word'
  measureElement.style.whiteSpace = 'pre-wrap'
  measureElement.style.boxSizing = 'border-box'
  measureElement.textContent = text
  
  // 添加到 DOM 进行测量
  document.body.appendChild(measureElement)
  const height = measureElement.scrollHeight
  document.body.removeChild(measureElement)
  
  return height
}

// 开始打字机效果
const startTyping = () => {
  if (!props.text) return
  
  isTyping.value = true
  displayedText.value = ''
  let currentIndex = 0
  
  const typeChar = () => {
    if (currentIndex < props.text.length && isTyping.value) {
      displayedText.value = props.text.substring(0, currentIndex + 1)
      currentIndex++
      typingTimer.value = setTimeout(typeChar, 50) // 50ms 一个字符
    } else {
      isTyping.value = false
    }
  }
  
  typeChar()
}

// 跳过打字机效果
const skipTyping = () => {
  if (typingTimer.value) {
    clearTimeout(typingTimer.value)
    typingTimer.value = null
  }
  isTyping.value = false
  displayedText.value = props.text
}

// 处理点击外部事件
const handleClickOutside = (event) => {
  // 如果父组件要求忽略点击外部关闭，则不处理
  if (props.shouldIgnoreClickOutside && props.shouldIgnoreClickOutside()) {
    return
  }
  
  if (dialogRef.value && !dialogRef.value.contains(event.target)) {
    // 检查点击是否在拖动区域内
    const dragCircle = document.querySelector('.drag-circle')
    if (dragCircle && !dragCircle.contains(event.target)) {
      emit('close')
    }
  }
}

// 监听 visible 变化
watch(() => props.visible, (newVal) => {
  if (newVal) {
    // 在显示前计算所需高度
    const textHeight = calculateTextHeight(props.text)
    if (textHeight !== null) {
      // 如果计算高度 <= 最大高度，使用计算高度；否则使用最大高度
      calculatedHeight.value = textHeight <= dialogMaxHeight.value ? textHeight : dialogMaxHeight.value
    } else {
      calculatedHeight.value = dialogMaxHeight.value
    }
    
    displayedText.value = ''
    startTyping()
    // 添加点击外部关闭的事件监听
    setTimeout(() => {
      document.addEventListener('click', handleClickOutside)
    }, 0)
  } else {
    displayedText.value = ''
    calculatedHeight.value = null
    isTyping.value = false
    if (typingTimer.value) {
      clearTimeout(typingTimer.value)
      typingTimer.value = null
    }
    // 移除点击外部关闭的事件监听
    document.removeEventListener('click', handleClickOutside)
  }
})

// 监听文本变化
watch(() => props.text, (newText) => {
  if (!newText && props.visible) {
    emit('close')
  } else if (props.visible && newText) {
    // 如果对话框已显示且文本变化，重新计算高度并开始打字机效果
    const textHeight = calculateTextHeight(newText)
    if (textHeight !== null) {
      calculatedHeight.value = textHeight <= dialogMaxHeight.value ? textHeight : dialogMaxHeight.value
    } else {
      calculatedHeight.value = dialogMaxHeight.value
    }
    skipTyping()
    startTyping()
  }
})

// 暴露方法供父组件调用
defineExpose({
  skipTyping
})

onMounted(() => {
  // 组件挂载时加载配置和对话框背景
  loadConfig()
  loadDialogBackground()
})

onUnmounted(() => {
  // 清理打字机定时器
  if (typingTimer.value) {
    clearTimeout(typingTimer.value)
  }
  // 移除点击外部关闭的事件监听
  document.removeEventListener('click', handleClickOutside)
})
</script>

<style scoped>
/* 对话框气泡样式 */
.dialog-bubble {
  position: absolute;
  z-index: 100;
  pointer-events: auto;
  -webkit-app-region: no-drag;
}

.dialog-content {
  background-color: rgba(255, 255, 255, 0.95);
  border-radius: 12px;
  padding: 10px 14px;
  font-size: 14px;
  line-height: 1.5;
  color: #333;
  box-shadow: 0 2px 10px rgba(0, 0, 0, 0.15);
  word-wrap: break-word;
  word-break: break-word;
  white-space: pre-wrap;
  overflow-y: auto;
  box-sizing: border-box;
}

.dialog-tail {
  position: absolute;
  top: -8px;
  left: 20px;
  width: 0;
  height: 0;
  border-left: 8px solid transparent;
  border-right: 8px solid transparent;
  border-bottom: 8px solid rgba(255, 255, 255, 0.95);
  filter: drop-shadow(0 -2px 2px rgba(0, 0, 0, 0.1));
}

/* 对话框滚动条样式 */
.dialog-content::-webkit-scrollbar {
  width: 4px;
}

.dialog-content::-webkit-scrollbar-track {
  background: transparent;
}

.dialog-content::-webkit-scrollbar-thumb {
  background: rgba(0, 0, 0, 0.2);
  border-radius: 2px;
}

.dialog-content::-webkit-scrollbar-thumb:hover {
  background: rgba(0, 0, 0, 0.3);
}
</style>
