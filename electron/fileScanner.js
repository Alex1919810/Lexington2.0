const path = require('path')
const fs = require('fs')
const { app } = require('electron')

// 判断是否为开发环境
const isDev = process.env.NODE_ENV === 'development' || !app.isPackaged

// 获取应用路径
function getAppPath() {
  return isDev ? process.cwd() : path.join(__dirname, '..')
}

// 扫描立绘文件
async function scanIllustrations() {
  try {
    const illustrations = []
    const appPath = getAppPath()
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
}

// 扫描对话框背景文件
async function scanDialogBackgrounds() {
  try {
    const backgrounds = []
    const appPath = getAppPath()
    const dialogBackgroundPath = path.join(appPath, 'resource', 'background', 'dialog')
    
    // 检查dialog背景目录是否存在
    if (!fs.existsSync(dialogBackgroundPath)) {
      console.warn('dialog背景目录不存在:', dialogBackgroundPath)
      return { success: true, backgrounds: [] }
    }
    
    // 读取目录下的所有文件
    const files = fs.readdirSync(dialogBackgroundPath)
    
    // 支持的图片格式
    const imageExtensions = ['.png', '.jpg', '.jpeg', '.gif', '.webp']
    
    files.forEach(file => {
      const filePath = path.join(dialogBackgroundPath, file)
      const stat = fs.statSync(filePath)
      
      // 只处理文件，跳过目录
      if (stat.isFile()) {
        const ext = path.extname(file).toLowerCase()
        if (imageExtensions.includes(ext)) {
          const normalizedPath = filePath.replace(/\\/g, '/')
          const fileUrl = `file:///${normalizedPath}`
          backgrounds.push({
            name: file,
            path: fileUrl,
            fullPath: filePath
          })
        }
      }
    })
    
    // 按文件名排序，确保顺序一致
    backgrounds.sort((a, b) => a.name.localeCompare(b.name))
    
    console.log(`扫描完成，共找到 ${backgrounds.length} 个对话框背景文件`)
    return { success: true, backgrounds }
  } catch (error) {
    console.error('扫描对话框背景文件失败:', error)
    return { success: false, error: error.message, backgrounds: [] }
  }
}

module.exports = {
  scanIllustrations,
  scanDialogBackgrounds
}
