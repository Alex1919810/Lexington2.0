import { inject, provide, type InjectionKey, type Ref } from 'vue'

/**
 * 热加载函数类型
 */
export type HotReloadFunction = () => Promise<void> | void

/**
 * 热加载管理器接口
 */
export interface HotReloadManager {
  /**
   * 注册热加载函数
   * @param name 热加载项的名称（用于调试）
   * @param reloadFn 热加载函数
   * @returns 取消注册的函数
   */
  register: (name: string, reloadFn: HotReloadFunction) => () => void
  
  /**
   * 触发所有已注册的热加载函数
   */
  reload: () => Promise<void>
  
  /**
   * 获取已注册的热加载项列表
   */
  getRegisteredItems: () => string[]
}

/**
 * 热加载管理器的注入键
 */
export const HotReloadManagerKey: InjectionKey<HotReloadManager> = Symbol('HotReloadManager')

/**
 * 创建热加载管理器
 */
export function createHotReloadManager(): HotReloadManager {
  const registeredItems = new Map<string, HotReloadFunction>()
  
  return {
    register(name: string, reloadFn: HotReloadFunction) {
      registeredItems.set(name, reloadFn)
      console.log(`[热加载] 注册: ${name}，当前共 ${registeredItems.size} 项`)
      
      // 返回取消注册的函数
      return () => {
        registeredItems.delete(name)
        console.log(`[热加载] 取消注册: ${name}，当前共 ${registeredItems.size} 项`)
      }
    },
    
    async reload() {
      const items = Array.from(registeredItems.entries())
      if (items.length === 0) {
        console.log('[热加载] 没有已注册的热加载项')
        return
      }
      
      console.log(`[热加载] 开始重新加载 ${items.length} 项...`)
      
      // 并行执行所有热加载函数
      const promises = items.map(async ([name, fn]) => {
        try {
          const startTime = Date.now()
          await Promise.resolve(fn())
          const duration = Date.now() - startTime
          console.log(`[热加载] ✓ ${name} (${duration}ms)`)
        } catch (error) {
          console.error(`[热加载] ✗ ${name} 失败:`, error)
        }
      })
      
      await Promise.all(promises)
      console.log('[热加载] 重新加载完成')
    },
    
    getRegisteredItems() {
      return Array.from(registeredItems.keys())
    }
  }
}

/**
 * 在组件中使用热加载功能
 * @param name 热加载项的名称
 * @param reloadFn 热加载函数
 */
export function useHotReload(name: string, reloadFn: HotReloadFunction) {
  const manager = inject(HotReloadManagerKey)
  
  if (!manager) {
    console.warn(`[热加载] ${name}: 未找到 HotReloadManager，请确保在 App.vue 中提供了管理器`)
    return {
      unregister: () => {}
    }
  }
  
  // 注册热加载函数
  const unregister = manager.register(name, reloadFn)
  
  // 组件卸载时自动取消注册
  return {
    unregister
  }
}

/**
 * 在 App.vue 中提供热加载管理器
 */
export function provideHotReload(manager: HotReloadManager) {
  provide(HotReloadManagerKey, manager)
}

/**
 * 在 App.vue 中注入热加载管理器（用于触发刷新）
 */
export function useHotReloadManager(): HotReloadManager | undefined {
  return inject(HotReloadManagerKey)
}
