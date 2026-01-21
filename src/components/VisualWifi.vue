<template>
  <div 
    class="visual-wifi" 
    :style="{ transform: transformStyle }"
    @mouseenter="handleMouseEnter"
    @mouseleave="handleMouseLeave"
    @contextmenu="handleContextMenu"
  >
    <!-- 如果有立绘图片，显示图片；否则显示默认的 CSS 动画宠物 -->
    <img 
      v-if="imagePath && !imageError" 
      ref="imageRef"
      :src="imagePath" 
      alt="桌宠立绘" 
      class="visual-wifi-image"
      @error="handleImageError"
      @load="handleImageLoad"
    />
    <div v-else ref="imageRef" class="visual-wifi-body">
      <div class="visual-wifi-face">
        <div class="visual-wifi-eye left-eye"></div>
        <div class="visual-wifi-eye right-eye"></div>
        <div class="visual-wifi-mouth"></div>
      </div>
    </div>
    <!-- 圆形拖动区域（使用自定义拖拽逻辑） -->
    <div 
      ref="dragCircleRef"
      class="drag-circle"
      :style="circleStyle"
      @pointerdown="handleDragCirclePointerDown"
    ></div>
    <div class="visual-wifi-shadow"></div>
    <!-- 对话框 -->
    <DialogBubble
      :visible="isDialogVisible"
      :text="dialogText"
      :position="{ x: imageCenterX, y: imageCenterY }"
      :offset-x="100"
      :offset-y="50"
      :should-ignore-click-outside="() => isProcessingDragClick"
      @close="closeDialog"
      ref="dialogBubbleRef"
    />
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onUnmounted, watch } from 'vue'
import DialogBubble from './DialogBubble.vue'

const props = defineProps({
  imagePath: {
    type: String,
    default: ''
  },
  dialogText: {
    type: String,
    default: ''
  }
})

const imageError = ref(false)
const imageRef = ref(null)
const imageHeight = ref(0)
const imageCenterX = ref(0)
const imageCenterY = ref(0)

const animationFrame = ref(null)
const bounceOffset = ref(0)
const isHovered = ref(false)

// 对话框相关状态
const isDialogVisible = ref(false)
const dialogBubbleRef = ref(null)
const dragCircleRef = ref(null) // 拖动区域元素引用
const isProcessingDragClick = ref(false) // 标记是否正在处理拖动区域的点击
const hasMoved = ref(false) // 标记是否移动了
const isDragging = ref(false) // 标记是否正在拖拽

// 窗口位置信息（初始化时获取）
const windowPosition = ref({ x: 0, y: 0 })
// 拖拽开始时记录的偏移量（指针相对于窗口左上角的偏移）
const dragOffset = ref({ x: 0, y: 0 })

// 计算 transform 样式
const transformStyle = computed(() => {
  let transform = `translateY(${bounceOffset.value}px)`
  if (isHovered.value) {
    transform += ' translateY(-5px) scale(1.05)'
  }
  return transform
})

// 动画循环
const animate = () => {
  bounceOffset.value = Math.sin(Date.now() / 500) * 5
  animationFrame.value = requestAnimationFrame(animate)
}

// 处理鼠标事件
const handleMouseEnter = () => {
  isHovered.value = true
}

const handleMouseLeave = () => {
  isHovered.value = false
}

// 处理右键菜单
const handleContextMenu = (event) => {
  event.preventDefault()
  if (window.electronAPI && window.electronAPI.showContextMenu) {
    window.electronAPI.showContextMenu()
  }
}

// 处理图片加载错误
const handleImageError = (event) => {
  imageError.value = true
  console.error('图片加载失败:', props.imagePath, event)
}

// 更新图片尺寸和位置信息
const updateImageInfo = () => {
  if (!imageRef.value) {
    // 如果没有图片引用，使用默认值（visual-wifi-body 的尺寸）
    const visualWifiElement = document.querySelector('.visual-wifi')
    if (visualWifiElement) {
      const visualWifiRect = visualWifiElement.getBoundingClientRect()
      // visual-wifi-body 的尺寸是 120px x 120px
      imageHeight.value = 120
      imageCenterX.value = visualWifiRect.width / 2
      imageCenterY.value = visualWifiRect.height / 2
      console.log('使用默认宠物尺寸:', {
        height: imageHeight.value,
        centerX: imageCenterX.value,
        centerY: imageCenterY.value,
        radius: imageHeight.value
      })
    }
    return
  }
  
  const rect = imageRef.value.getBoundingClientRect()
  const visualWifiRect = imageRef.value.closest('.visual-wifi')?.getBoundingClientRect()
  
  if (visualWifiRect) {
    // 获取图片的实际显示尺寸
    imageHeight.value = rect.height
    
    // 计算图片中心相对于 .visual-wifi 容器的位置
    // rect.left 和 rect.top 是相对于视口的，需要转换为相对于 .visual-wifi 的位置
    imageCenterX.value = rect.left - visualWifiRect.left + rect.width / 2
    imageCenterY.value = rect.top - visualWifiRect.top + rect.height / 2
    
    console.log('图片信息更新:', {
      height: imageHeight.value,
      centerX: imageCenterX.value,
      centerY: imageCenterY.value,
      radius: imageHeight.value
    })
  }
}

// 处理图片加载成功
const handleImageLoad = (event) => {
  imageError.value = false
  console.log('图片加载成功:', props.imagePath, event.target.naturalWidth, 'x', event.target.naturalHeight)
  // 等待下一帧确保 DOM 已更新
  setTimeout(() => {
    updateImageInfo()
  }, 0)
}

// 计算圆形拖动区域的样式
const circleStyle = computed(() => {
  // 如果没有图片，使用默认值（visual-wifi-body 的尺寸 120px）
  const height = imageHeight.value > 0 ? imageHeight.value : 120
  const centerX = imageCenterX.value > 0 ? imageCenterX.value : 225 // 450px / 2
  const centerY = imageCenterY.value > 0 ? imageCenterY.value : 225 // 450px / 2
  
  // 半径乘以0.5，使圆形区域缩小为原来的一半
  const radius = height * 0.3
  const diameter = radius * 2
  
  return {
    width: `${diameter}px`,
    height: `${diameter}px`,
    left: `${centerX - radius}px`,
    top: `${centerY - radius}px`,
    borderRadius: '50%'
  }
})


// 处理拖动区域的指针按下事件（使用 Pointer Events 和 setPointerCapture）
const handleDragCirclePointerDown = (event) => {
  // 阻止默认行为
  event.preventDefault()
  
  // 获取拖动区域元素
  const element = dragCircleRef.value
  if (!element) return
  
  // 捕获指针事件，确保在拖拽期间持续接收事件，即使光标已离开窗口区域
  element.setPointerCapture(event.pointerId)
  
  // 记录点击开始时间
  const startTime = Date.now()
  hasMoved.value = false
  isDragging.value = false
  
  // 计算指针在屏幕上的绝对坐标
  // screenX/screenY 是相对于整个屏幕的坐标
  const screenX = event.screenX
  const screenY = event.screenY
  
  // 计算指针相对于窗口左上角的偏移量
  // clientX/clientY 是相对于窗口的坐标
  dragOffset.value = {
    x: event.clientX,
    y: event.clientY
  }
  
  // 处理指针移动事件
  const handlePointerMove = (e) => {
    // 使用屏幕坐标计算移动距离（因为指针可能已移出窗口）
    const startScreenX = screenX
    const startScreenY = screenY
    const currentScreenX = e.screenX
    const currentScreenY = e.screenY
    const distance = Math.sqrt(
      Math.pow(currentScreenX - startScreenX, 2) + 
      Math.pow(currentScreenY - startScreenY, 2)
    )
    
    // 如果移动距离大于5px，认为是拖拽
    if (distance > 5) {
      if (!isDragging.value) {
        // 第一次检测到拖拽，开始拖拽
        hasMoved.value = true
        isDragging.value = true
        console.log('检测到拖拽，开始窗口拖拽')
      }
      
      // 基于屏幕绝对坐标，直接计算出"此刻窗口应该在的位置"
      // 窗口位置 = 屏幕绝对坐标 - 相对偏移量
      const newWindowX = e.screenX - dragOffset.value.x
      const newWindowY = e.screenY - dragOffset.value.y
      
      // 更新窗口位置信息
      windowPosition.value = { x: newWindowX, y: newWindowY }
      
      // 一次性发给主进程，直接设置窗口位置
      if (window.electronAPI && window.electronAPI.setWindowPosition) {
        window.electronAPI.setWindowPosition({ x: newWindowX, y: newWindowY })
      }
    }
  }
  
  // 处理指针抬起事件
  const handlePointerUp = (e) => {
    // 释放指针捕获
    if (element) {
      element.releasePointerCapture(e.pointerId)
    }
    
    // 计算移动距离和时间差（使用屏幕坐标）
    const startScreenX = screenX
    const startScreenY = screenY
    const distance = Math.sqrt(
      Math.pow(e.screenX - startScreenX, 2) + 
      Math.pow(e.screenY - startScreenY, 2)
    )
    const timeDiff = Date.now() - startTime
    
    console.log('pointerup:', { distance, timeDiff, hasMoved: hasMoved.value, isDragging: isDragging.value })
    
    // 如果移动距离小于5px且时间小于300ms，认为是点击而不是拖拽
    if (distance < 5 && timeDiff < 300 && !hasMoved.value) {
      console.log('检测到点击，显示对话框')
      setTimeout(() => {
        handleDialogClick()
      }, 50)
    }
    
    // 重置状态
    hasMoved.value = false
    isDragging.value = false
    dragOffset.value = { x: 0, y: 0 }
    
    // 移除事件监听器
    element.removeEventListener('pointermove', handlePointerMove)
    element.removeEventListener('pointerup', handlePointerUp)
  }
  
  // 添加事件监听器
  element.addEventListener('pointermove', handlePointerMove)
  element.addEventListener('pointerup', handlePointerUp, { once: true })
  
  // 处理指针取消事件（例如指针被系统取消）
  const handlePointerCancel = (e) => {
    if (element) {
      element.releasePointerCapture(e.pointerId)
    }
    element.removeEventListener('pointermove', handlePointerMove)
    element.removeEventListener('pointerup', handlePointerUp)
    element.removeEventListener('pointercancel', handlePointerCancel)
    
    // 重置状态
    hasMoved.value = false
    isDragging.value = false
    dragOffset.value = { x: 0, y: 0 }
  }
  
  element.addEventListener('pointercancel', handlePointerCancel, { once: true })
}

// 处理对话框点击逻辑
const handleDialogClick = () => {
  isProcessingDragClick.value = true
  
  // 如果有对话框文本
  if (props.dialogText) {
    if (dialogBubbleRef.value && isDialogVisible.value) {
      // 如果正在打字，跳过动画
      dialogBubbleRef.value.skipTyping()
    }
    
    if (!isDialogVisible.value) {
      // 如果对话框未显示，显示对话框
      isDialogVisible.value = true
    } else {
      // 如果对话框已显示，关闭对话框
      closeDialog()
    }
  } else {
    console.log('对话框文本为空，无法显示对话框')
    // 即使没有对话框文本，如果对话框已显示，也应该关闭
    if (isDialogVisible.value) {
      closeDialog()
    }
  }
  
  // 延迟重置标志，确保 handleClickOutside 能正确判断
  setTimeout(() => {
    isProcessingDragClick.value = false
  }, 100)
}

// 关闭对话框
const closeDialog = () => {
  isDialogVisible.value = false
}

// 监听对话框文本变化，重置对话框状态
watch(() => props.dialogText, (newText) => {
  if (!newText) {
    closeDialog()
  }
})

// 监听图片路径变化，重置错误状态
watch(() => props.imagePath, (newPath) => {
  if (newPath) {
    imageError.value = false
    console.log('切换立绘:', newPath)
    // 重置图片信息，等待新图片加载
    imageHeight.value = 0
  }
})

// 监听图片尺寸变化
watch(imageHeight, () => {
  if (imageHeight.value > 0) {
    updateImageInfo()
  }
})

// 初始化窗口位置信息
const initializeWindowBounds = async () => {
  if (window.electronAPI && window.electronAPI.getWindowBounds) {
    try {
      const result = await window.electronAPI.getWindowBounds()
      if (result.success) {
        windowPosition.value = { x: result.x, y: result.y }
        console.log('窗口位置信息已初始化:', { position: windowPosition.value })
      }
    } catch (error) {
      console.error('获取窗口位置信息失败:', error)
    }
  }
}

onMounted(() => {
  animate()
  // 初始化窗口位置信息
  initializeWindowBounds()
  // 初始化时更新图片信息
  setTimeout(() => {
    updateImageInfo()
    // 监听窗口大小变化
    window.addEventListener('resize', updateImageInfo)
  }, 100)
})

onUnmounted(() => {
  if (animationFrame.value) {
    cancelAnimationFrame(animationFrame.value)
  }
  window.removeEventListener('resize', updateImageInfo)
})

</script>

<style scoped>
.visual-wifi {
  width: 450px;
  height: 450px;
  position: relative;
  transition: transform 0.3s ease;
  cursor: pointer;
  /* 移除整个容器的拖动，改为只在圆形区域内拖动 */
  -webkit-app-region: no-drag;
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1;
  /* 确保立绘区域不会阻止其他元素的点击事件 */
  pointer-events: auto;
}

.visual-wifi-image {
  max-width: 100%;
  max-height: 100%;
  width: auto;
  height: auto;
  object-fit: contain;
  pointer-events: none;
  user-select: none;
  display: block;
  image-rendering: auto;
  -webkit-user-drag: none;
  position: relative;
  z-index: 1;
}

.visual-wifi-body {
  width: 120px;
  height: 120px;
  background: linear-gradient(135deg, #ff6b9d, #ff8fab);
  border-radius: 50% 50% 50% 50% / 60% 60% 40% 40%;
  position: relative;
  margin: 0 auto;
  box-shadow: 0 10px 20px rgba(0, 0, 0, 0.2);
  animation: float 2s ease-in-out infinite;
}

@keyframes float {
  0%, 100% {
    transform: translateY(0px);
  }
  50% {
    transform: translateY(-10px);
  }
}

.visual-wifi-face {
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  width: 100%;
  height: 100%;
}

.visual-wifi-eye {
  width: 12px;
  height: 12px;
  background: #333;
  border-radius: 50%;
  position: absolute;
  top: 35%;
  animation: blink 3s infinite;
}

.visual-wifi-eye.left-eye {
  left: 35%;
}

.visual-wifi-eye.right-eye {
  right: 35%;
}

@keyframes blink {
  0%, 90%, 100% {
    height: 12px;
  }
  95% {
    height: 2px;
  }
}

.visual-wifi-mouth {
  width: 30px;
  height: 15px;
  border: 3px solid #333;
  border-top: none;
  border-radius: 0 0 30px 30px;
  position: absolute;
  bottom: 30%;
  left: 50%;
  transform: translateX(-50%);
}

.visual-wifi-shadow {
  width: 80px;
  height: 20px;
  background: rgba(0, 0, 0, 0.1);
  border-radius: 50%;
  position: absolute;
  bottom: -10px;
  left: 50%;
  transform: translateX(-50%);
  animation: shadow 2s ease-in-out infinite;
}

/* 圆形拖动区域（使用自定义拖拽逻辑） */
.drag-circle {
  position: absolute;
  -webkit-app-region: no-drag;
  pointer-events: auto;
  z-index: 2;
  cursor: move;
  touch-action: none; /* 禁用触摸默认行为，支持 Pointer Events */
  /* 可选：用于调试，显示圆形区域 */
  /* background: rgba(255, 0, 0, 0.1); */
  /* border: 1px dashed rgba(255, 0, 0, 0.3); */
}

@keyframes shadow {
  0%, 100% {
    width: 80px;
    opacity: 0.3;
  }
  50% {
    width: 100px;
    opacity: 0.2;
  }
}

</style>
