const { BrowserWindow, screen, app } = require('electron')
const path = require('path')
const fs = require('fs')

// 判断是否为开发环境
const isDev = process.env.NODE_ENV === 'development' || !app.isPackaged

let petWindow = null
// 记录窗口初始化尺寸（启动时记录一次，后续拖拽始终使用此值，避免DPI回环引入的尺寸误差）
let windowInitialSize = { width: 0, height: 0 }

function createPetWindow() {
  const { width: screenWidth, height: screenHeight } = screen.getPrimaryDisplay().workAreaSize
  
  // 开发模式下使用更大的窗口，方便调试
  const windowWidth = isDev ? 800 : 500
  const windowHeight = isDev ? 600 : 500
  
  // 图标路径
  const appPath = isDev ? process.cwd() : path.join(__dirname, '..')
  const iconPath = path.join(appPath, 'resource', 'icon', 'Normal.ico')
  
  // 创建无边框、透明、置顶的窗口
  petWindow = new BrowserWindow({
    width: windowWidth,
    height: windowHeight,
    x: isDev ? 100 : screenWidth - 520,
    y: isDev ? 100 : 50,
    frame: false, // 无边框
    transparent: true, // 透明背景
    alwaysOnTop: true, // 始终置顶
    skipTaskbar: true, // 不在任务栏显示
    resizable: isDev, // 开发模式下可调整大小
    movable: false, // 禁用默认移动，使用自定义拖拽逻辑
    icon: fs.existsSync(iconPath) ? iconPath : undefined, // 设置窗口图标
    webPreferences: {
      nodeIntegration: false,
      contextIsolation: true,
      enableRemoteModule: false,
      preload: path.join(__dirname, 'preload.js'),
      webSecurity: false, // 允许加载本地文件
      allowRunningInsecureContent: false
    }
  })

  // 加载页面
  if (isDev) {
    petWindow.loadURL('http://localhost:5173')
    // 开发环境下打开开发者工具
    petWindow.webContents.openDevTools()
  } else {
    petWindow.loadFile(path.join(__dirname, '../dist/index.html'))
  }

  // 窗口关闭事件
  petWindow.on('closed', () => {
    petWindow = null
  })
  
  // 记录窗口初始化尺寸（启动时记录一次）
  const [initWidth, initHeight] = petWindow.getSize()
  windowInitialSize = { width: initWidth, height: initHeight }
  console.log('窗口初始化尺寸已记录:', windowInitialSize)

  // 点击穿透（可选，让宠物窗口可以点击穿透到下层）
  // petWindow.setIgnoreMouseEvents(true, { forward: true })
  
  return petWindow
}

function getPetWindow() {
  return petWindow
}

function getWindowInitialSize() {
  return windowInitialSize
}

module.exports = {
  createPetWindow,
  getPetWindow,
  getWindowInitialSize
}
