<template>
  <div class="pet-container">
    <Pet :x="petX" :y="petY" />
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue'
import Pet from './components/Pet.vue'

const petX = ref(0)
const petY = ref(0)

onMounted(() => {
  // 初始化宠物位置
  petX.value = 0
  petY.value = 0
})
</script>

<style scoped>
.pet-container {
  width: 100%;
  height: 100%;
  overflow: hidden;
  user-select: none;
  /* 允许拖拽窗口，但在宠物区域外 */
  -webkit-app-region: drag;
}

/* 宠物本身不参与拖拽，可以添加点击交互 */
.pet-container :deep(.pet) {
  -webkit-app-region: no-drag;
}
</style>
