const { ipcMain, BrowserWindow } = require('electron')
const { getPetWindow, getWindowInitialSize } = require('./window')
const { createContextMenu } = require('./menu')
const { scanIllustrations, scanDialogBackgrounds, scanInputBackgrounds, loadCharacterText } = require('./fileScanner')
const { getConfig } = require('./config')
const { callLLMAPI } = require('./llmApi')

// 注册所有 IPC 处理器
function registerIpcHandlers() {
  // 处理右键菜单 IPC
  ipcMain.handle('show-context-menu', (event) => {
    const menu = createContextMenu()
    const window = BrowserWindow.fromWebContents(event.sender)
    if (window) {
      menu.popup({ window })
    }
  })

  // 获取窗口位置和大小（用于渲染进程初始化）
  ipcMain.handle('get-window-bounds', (event) => {
    const window = BrowserWindow.fromWebContents(event.sender)
    if (window && !window.isDestroyed()) {
      const [x, y] = window.getPosition()
      const windowInitialSize = getWindowInitialSize()
      return { 
        success: true, 
        x, 
        y, 
        width: windowInitialSize.width, 
        height: windowInitialSize.height 
      }
    }
    return { success: false }
  })

  // 自定义拖拽逻辑（基于屏幕绝对坐标）
  // 设置窗口位置（直接使用绝对坐标，不依赖增量移动）
  ipcMain.handle('set-window-position', (event, absolutePosition) => {
    const window = BrowserWindow.fromWebContents(event.sender)
    if (window && !window.isDestroyed()) {
      const windowInitialSize = getWindowInitialSize()
      // 直接设置窗口位置，使用缓存的初始化尺寸，避免DPI回环引入的尺寸误差
      const newBounds = {
        x: Math.round(absolutePosition.x),
        y: Math.round(absolutePosition.y),
        width: windowInitialSize.width,
        height: windowInitialSize.height
      }
      window.setBounds(newBounds, false) // false 参数表示不触发动画
      return { success: true }
    }
    return { success: false }
  })

  // 扫描立绘文件
  ipcMain.handle('scan-illustrations', async () => {
    return await scanIllustrations()
  })

  // 扫描对话框背景文件
  ipcMain.handle('scan-dialog-backgrounds', async () => {
    return await scanDialogBackgrounds()
  })

  // 扫描输入框背景文件
  ipcMain.handle('scan-input-backgrounds', async () => {
    return await scanInputBackgrounds()
  })

  // 读取配置文件
  ipcMain.handle('get-config', async () => {
    return await getConfig()
  })

  // 加载角色文本文件
  ipcMain.handle('load-character-text', async (event, characterName) => {
    return await loadCharacterText(characterName)
  })

  // 调用大模型API
  ipcMain.handle('call-llm-api', async (event, userInput) => {
    try {
      // 获取配置
      const configResult = await getConfig()
      if (!configResult.success || !configResult.config || !configResult.config.llm) {
        return {
          success: false,
          error: '无法读取LLM配置，请检查config.json'
        }
      }

      const llmConfig = configResult.config.llm
      
      // 调用API
      const result = await callLLMAPI(userInput, llmConfig)
      return result
    } catch (error) {
      return {
        success: false,
        error: `调用LLM API时发生错误: ${error.message}`
      }
    }
  })
}

module.exports = {
  registerIpcHandlers
}
