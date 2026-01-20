const { app, BrowserWindow, screen } = require('electron')
const path = require('path')

let petWindow = null

// 判断是否为开发环境
const isDev = process.env.NODE_ENV === 'development' || !app.isPackaged

function createPetWindow() {
  const { width, height } = screen.getPrimaryDisplay().workAreaSize
  
  // 创建无边框、透明、置顶的窗口
  petWindow = new BrowserWindow({
    width: 200,
    height: 200,
    x: width - 220,
    y: 50,
    frame: false, // 无边框
    transparent: true, // 透明背景
    alwaysOnTop: true, // 始终置顶
    skipTaskbar: true, // 不在任务栏显示
    resizable: false, // 不可调整大小
    movable: true, // 可移动
    webPreferences: {
      nodeIntegration: false,
      contextIsolation: true,
      enableRemoteModule: false,
      preload: path.join(__dirname, 'preload.js')
    }
  })

  // 加载页面
  if (isDev) {
    petWindow.loadURL('http://localhost:5173')
    // 开发环境下打开开发者工具（可选）
    // petWindow.webContents.openDevTools()
  } else {
    petWindow.loadFile(path.join(__dirname, '../dist/index.html'))
  }

  // 窗口关闭事件
  petWindow.on('closed', () => {
    petWindow = null
  })

  // 点击穿透（可选，让宠物窗口可以点击穿透到下层）
  // petWindow.setIgnoreMouseEvents(true, { forward: true })
}

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
