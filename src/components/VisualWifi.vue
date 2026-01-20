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
    <!-- 圆形拖动区域遮罩层 -->
    <div 
      class="drag-circle"
      :style="circleStyle"
    ></div>
    <div class="visual-wifi-shadow"></div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onUnmounted, watch } from 'vue'

const props = defineProps({
  x: {
    type: Number,
    default: 0
  },
  y: {
    type: Number,
    default: 0
  },
  imagePath: {
    type: String,
    default: ''
  }
})

const imageError = ref(false)
const imageRef = ref(null)
const imageHeight = ref(0)
const imageWidth = ref(0)
const imageCenterX = ref(0)
const imageCenterY = ref(0)

const animationFrame = ref(null)
const bounceOffset = ref(0)
const isHovered = ref(false)

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
      imageWidth.value = 120
      imageHeight.value = 120
      imageCenterX.value = visualWifiRect.width / 2
      imageCenterY.value = visualWifiRect.height / 2
      console.log('使用默认宠物尺寸:', {
        width: imageWidth.value,
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
    imageWidth.value = rect.width
    imageHeight.value = rect.height
    
    // 计算图片中心相对于 .visual-wifi 容器的位置
    // rect.left 和 rect.top 是相对于视口的，需要转换为相对于 .visual-wifi 的位置
    imageCenterX.value = rect.left - visualWifiRect.left + rect.width / 2
    imageCenterY.value = rect.top - visualWifiRect.top + rect.height / 2
    
    console.log('图片信息更新:', {
      width: imageWidth.value,
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
watch([imageHeight, imageWidth], () => {
  if (imageHeight.value > 0) {
    updateImageInfo()
  }
})

onMounted(() => {
  animate()
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

/* 圆形拖动区域 */
.drag-circle {
  position: absolute;
  -webkit-app-region: drag;
  pointer-events: auto;
  z-index: 2;
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
