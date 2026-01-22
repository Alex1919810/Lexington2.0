const { ipcMain, BrowserWindow } = require('electron')
const { getPetWindow, getWindowInitialSize } = require('./window')
const { createContextMenu } = require('./menu')
const { scanIllustrations, scanDialogBackgrounds } = require('./fileScanner')
const { getConfig } = require('./config')

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

  // 读取配置文件
  ipcMain.handle('get-config', async () => {
    return await getConfig()
  })
}

module.exports = {
  registerIpcHandlers
}
