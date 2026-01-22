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

// 扫描指定目录下的背景图片（通用逻辑）
async function scanBackgrounds(relativeDir) {
  try {
    const backgrounds = []
    const appPath = getAppPath()
    const backgroundPath = path.join(appPath, 'resource', 'background', relativeDir)

    if (!fs.existsSync(backgroundPath)) {
      console.warn(`背景目录不存在:`, backgroundPath)
      return { success: true, backgrounds: [] }
    }

    const files = fs.readdirSync(backgroundPath)
    const imageExtensions = ['.png', '.jpg', '.jpeg', '.gif', '.webp']

    files.forEach(file => {
      const filePath = path.join(backgroundPath, file)
      const stat = fs.statSync(filePath)
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

    backgrounds.sort((a, b) => a.name.localeCompare(b.name))
    return { success: true, backgrounds }
  } catch (error) {
    console.error(`扫描背景文件失败 (${relativeDir}):`, error)
    return { success: false, error: error.message, backgrounds: [] }
  }
}

// 扫描对话框背景文件
async function scanDialogBackgrounds() {
  const result = await scanBackgrounds('dialog')
  if (result.success && result.backgrounds) {
    console.log(`扫描完成，共找到 ${result.backgrounds.length} 个对话框背景文件`)
  }
  return result
}

// 扫描输入框背景文件
async function scanInputBackgrounds() {
  const result = await scanBackgrounds('input')
  if (result.success && result.backgrounds) {
    console.log(`扫描完成，共找到 ${result.backgrounds.length} 个输入框背景文件`)
  }
  return result
}

// 加载指定角色的文本文件
async function loadCharacterText(characterName) {
  try {
    if (!characterName) {
      console.warn('角色名为空，无法加载文本')
      return { success: false, error: '角色名为空', texts: [] }
    }
    
    const appPath = getAppPath()
    const textFilePath = path.join(appPath, 'resource', 'texts', characterName, 'text.json')
    
    // 检查文件是否存在
    if (!fs.existsSync(textFilePath)) {
      console.warn(`角色 ${characterName} 的文本文件不存在:`, textFilePath)
      return { success: true, texts: [] }
    }
    
    // 读取文件内容
    const fileContent = fs.readFileSync(textFilePath, 'utf-8')
    
    // 解析 JSON
    let texts = []
    try {
      texts = JSON.parse(fileContent)
      
      // 验证是否为数组
      if (!Array.isArray(texts)) {
        console.error(`角色 ${characterName} 的文本文件格式错误：不是数组`)
        return { success: false, error: '文本文件格式错误：不是数组', texts: [] }
      }
      
      // 验证数组元素是否为字符串
      const invalidItems = texts.filter(item => typeof item !== 'string')
      if (invalidItems.length > 0) {
        console.error(`角色 ${characterName} 的文本文件包含非字符串元素`)
        return { success: false, error: '文本文件格式错误：包含非字符串元素', texts: [] }
      }
      
      // 过滤空字符串
      texts = texts.filter(text => text && text.trim().length > 0)
      
      console.log(`成功加载角色 ${characterName} 的文本，共 ${texts.length} 条`)
      return { success: true, texts }
    } catch (parseError) {
      console.error(`解析角色 ${characterName} 的文本文件失败:`, parseError)
      return { success: false, error: `JSON 解析失败: ${parseError.message}`, texts: [] }
    }
  } catch (error) {
    console.error(`加载角色 ${characterName} 的文本文件失败:`, error)
    return { success: false, error: error.message, texts: [] }
  }
}

module.exports = {
  scanIllustrations,
  scanDialogBackgrounds,
  scanInputBackgrounds,
  loadCharacterText
}
