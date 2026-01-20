<template>
  <div 
    class="pet" 
    :style="{ transform: transformStyle }"
    @mouseenter="handleMouseEnter"
    @mouseleave="handleMouseLeave"
    @contextmenu="handleContextMenu"
  >
    <!-- 如果有立绘图片，显示图片；否则显示默认的 CSS 动画宠物 -->
    <img 
      v-if="imagePath" 
      :src="imagePath" 
      alt="桌宠立绘" 
      class="pet-image"
      @error="handleImageError"
    />
    <div v-else class="pet-body">
      <div class="pet-face">
        <div class="pet-eye left-eye"></div>
        <div class="pet-eye right-eye"></div>
        <div class="pet-mouth"></div>
      </div>
    </div>
    <div class="pet-shadow"></div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onUnmounted } from 'vue'

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
const handleImageError = () => {
  imageError.value = true
}

onMounted(() => {
  animate()
})

onUnmounted(() => {
  if (animationFrame.value) {
    cancelAnimationFrame(animationFrame.value)
  }
})
</script>

<style scoped>
.pet {
  width: 150px;
  height: 150px;
  position: relative;
  transition: transform 0.3s ease;
  cursor: pointer;
  -webkit-app-region: drag;
}

.pet-image {
  width: 100%;
  height: 100%;
  object-fit: contain;
  pointer-events: none;
  user-select: none;
}

.pet-body {
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

.pet-face {
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  width: 100%;
  height: 100%;
}

.pet-eye {
  width: 12px;
  height: 12px;
  background: #333;
  border-radius: 50%;
  position: absolute;
  top: 35%;
  animation: blink 3s infinite;
}

.pet-eye.left-eye {
  left: 35%;
}

.pet-eye.right-eye {
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

.pet-mouth {
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

.pet-shadow {
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
