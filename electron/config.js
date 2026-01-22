const path = require('path')
const fs = require('fs')
const { app } = require('electron')

// 判断是否为开发环境
const isDev = process.env.NODE_ENV === 'development' || !app.isPackaged

// 获取应用路径
function getAppPath() {
  return isDev ? process.cwd() : path.join(__dirname, '..')
}

// 默认配置
const defaultConfig = {
  dialog: {
    width: 200,
    maxHeight: 120,
    offsetX: 100,
    offsetY: 50
  }
}

// 读取配置文件
async function getConfig() {
  try {
    const appPath = getAppPath()
    const configPath = path.join(appPath, 'config', 'config.json')
    
    // 如果配置文件不存在，返回默认配置
    if (!fs.existsSync(configPath)) {
      console.warn('配置文件不存在，使用默认配置:', configPath)
      return {
        success: true,
        config: defaultConfig
      }
    }
    
    // 读取配置文件
    const configContent = fs.readFileSync(configPath, 'utf-8')
    const config = JSON.parse(configContent)
    
    // 确保配置结构完整，如果缺少字段则使用默认值
    const mergedConfig = {
      dialog: {
        width: config.dialog?.width ?? defaultConfig.dialog.width,
        maxHeight: config.dialog?.maxHeight ?? defaultConfig.dialog.maxHeight,
        offsetX: config.dialog?.offsetX ?? defaultConfig.dialog.offsetX,
        offsetY: config.dialog?.offsetY ?? defaultConfig.dialog.offsetY
      }
    }
    
    console.log('配置文件已加载:', mergedConfig)
    return { success: true, config: mergedConfig }
  } catch (error) {
    console.error('读取配置文件失败:', error)
    // 返回默认配置
    return {
      success: true,
      config: defaultConfig
    }
  }
}

module.exports = {
  getConfig
}
