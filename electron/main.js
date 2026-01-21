const { app, BrowserWindow, screen, Menu, ipcMain } = require('electron')
const path = require('path')
const fs = require('fs')

let petWindow = null

// 判断是否为开发环境
const isDev = process.env.NODE_ENV === 'development' || !app.isPackaged

function createPetWindow() {
  const { width, height } = screen.getPrimaryDisplay().workAreaSize
  
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
    x: isDev ? 100 : width - 520,
    y: isDev ? 100 : 50,
    frame: false, // 无边框
    transparent: true, // 透明背景
    alwaysOnTop: true, // 始终置顶
    skipTaskbar: true, // 不在任务栏显示
    resizable: isDev, // 开发模式下可调整大小
    movable: true, // 可移动
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

  // 点击穿透（可选，让宠物窗口可以点击穿透到下层）
  // petWindow.setIgnoreMouseEvents(true, { forward: true })
}

// 创建右键菜单
function createContextMenu() {
  return Menu.buildFromTemplate([
    {
      label: '设置',
      click: () => {
        // 预留设置功能
        console.log('设置功能待实现')
      }
    },
    {
      type: 'separator'
    },
    {
      label: '关于',
      click: () => {
        // 预留关于功能
        console.log('关于功能待实现')
      }
    },
    {
      type: 'separator'
    },
    {
      label: '关闭',
      click: () => {
        app.quit()
      }
    }
  ])
}

// 处理右键菜单 IPC
ipcMain.handle('show-context-menu', (event) => {
  const menu = createContextMenu()
  const window = BrowserWindow.fromWebContents(event.sender)
  if (window) {
    menu.popup({ window })
  }
})

// 扫描立绘文件
ipcMain.handle('scan-illustrations', async () => {
  try {
    const illustrations = []
    const appPath = isDev ? process.cwd() : path.join(__dirname, '..')
    const illustrationBasePath = path.join(appPath, 'resource', 'illustration')
    
    // 检查illustration目录是否存在
    if (!fs.existsSync(illustrationBasePath)) {
      console.warn('illustration目录不存在:', illustrationBasePath)
      return { success: true, illustrations: [] }
    }
    
    // 遍历illustration下的所有子文件夹
    const characterFolders = fs.readdirSync(illustrationBasePath, { withFileTypes: true })
    
    characterFolders.forEach(characterDir => {
      // 只处理文件夹，跳过文件
      if (!characterDir.isDirectory()) {
        return
      }
      
      const characterName = characterDir.name
      const characterPath = path.join(illustrationBasePath, characterName)
      
      // 扫描该角色文件夹下的gif子文件夹
      const gifPath = path.join(characterPath, 'gif')
      if (fs.existsSync(gifPath) && fs.statSync(gifPath).isDirectory()) {
        const gifFiles = fs.readdirSync(gifPath)
        gifFiles.forEach(file => {
          if (file.toLowerCase().endsWith('.gif')) {
            const fullPath = path.join(gifPath, file)
            const normalizedPath = fullPath.replace(/\\/g, '/')
            const fileUrl = `file:///${normalizedPath}`
            illustrations.push({
              name: `${characterName}/${file}`,
              path: fileUrl,
              type: 'gif',
              fullPath: fullPath,
              character: characterName
            })
          }
        })
      }
      
      // 扫描该角色文件夹下的png子文件夹
      const pngPath = path.join(characterPath, 'png')
      if (fs.existsSync(pngPath) && fs.statSync(pngPath).isDirectory()) {
        const pngFiles = fs.readdirSync(pngPath)
        pngFiles.forEach(file => {
          if (file.toLowerCase().endsWith('.png')) {
            const fullPath = path.join(pngPath, file)
            const normalizedPath = fullPath.replace(/\\/g, '/')
            const fileUrl = `file:///${normalizedPath}`
            illustrations.push({
              name: `${characterName}/${file}`,
              path: fileUrl,
              type: 'png',
              fullPath: fullPath,
              character: characterName
            })
          }
        })
      }
    })
    
    console.log(`扫描完成，共找到 ${illustrations.length} 个立绘文件`)
    return { success: true, illustrations }
  } catch (error) {
    console.error('扫描立绘文件失败:', error)
    return { success: false, error: error.message, illustrations: [] }
  }
})

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
