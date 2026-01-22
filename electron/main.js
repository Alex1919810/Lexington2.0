const { app, BrowserWindow } = require('electron')
const { createPetWindow } = require('./window')
const { registerIpcHandlers } = require('./ipcHandlers')

// 注册所有 IPC 处理器
registerIpcHandlers()

// 应用准备就绪
app.whenReady().then(() => {
  createPetWindow()

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      createPetWindow()
    }
  })
})

// 所有窗口关闭时（macOS除外）
app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') {
    app.quit()
  }
})

// 安全设置：防止新窗口打开
app.on('web-contents-created', (event, contents) => {
  contents.on('new-window', (event, navigationUrl) => {
    event.preventDefault()
  })
})
