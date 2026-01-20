<template>
  <div class="pet-container">
    <Pet :x="petX" :y="petY" :imagePath="petImagePath" />
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue'
import Pet from './components/Pet.vue'

const petX = ref(0)
const petY = ref(0)
const petImagePath = ref('') // 立绘图片路径，默认为空则显示默认 CSS 动画

onMounted(() => {
  // 初始化宠物位置
  petX.value = 0
  petY.value = 0
  
  // 可以在这里设置立绘图片路径，例如：
  // petImagePath.value = './pets/pet1.png'
  // 或者从配置中读取
})
</script>

<style scoped>
.pet-container {
  width: 100%;
  height: 100%;
  overflow: hidden;
  user-select: none;
  /* 整个容器都可以拖动窗口 */
  -webkit-app-region: drag;
}

/* 宠物组件内部已经设置了 drag，确保可以拖动窗口 */
.pet-container :deep(.pet) {
  -webkit-app-region: drag;
}
</style>
