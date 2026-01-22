const { contextBridge, ipcRenderer } = require('electron')

// 暴露安全的 API 给渲染进程
contextBridge.exposeInMainWorld('electronAPI', {
  // 窗口控制
  minimize: () => ipcRenderer.invoke('window-minimize'),
  close: () => ipcRenderer.invoke('window-close'),
  
  // 获取屏幕信息
  getScreenSize: () => ipcRenderer.invoke('get-screen-size'),
  
  // 获取窗口位置和大小（用于初始化）
  getWindowBounds: () => ipcRenderer.invoke('get-window-bounds'),
  
  // 窗口拖拽（基于屏幕绝对坐标，直接设置窗口位置）
  setWindowPosition: (absolutePosition) => ipcRenderer.invoke('set-window-position', absolutePosition),
  
  // 右键菜单
  showContextMenu: () => ipcRenderer.invoke('show-context-menu'),
  
  // 扫描立绘文件
  scanIllustrations: () => ipcRenderer.invoke('scan-illustrations'),
  
  // 扫描对话框背景文件
  scanDialogBackgrounds: () => ipcRenderer.invoke('scan-dialog-backgrounds'),

  // 扫描输入框背景文件
  scanInputBackgrounds: () => ipcRenderer.invoke('scan-input-backgrounds'),
  
  // 读取配置文件
  getConfig: () => ipcRenderer.invoke('get-config'),
  
  // 加载角色文本文件
  loadCharacterText: (characterName) => ipcRenderer.invoke('load-character-text', characterName),
  
  // 调用大模型API
  callLLMAPI: (userInput) => ipcRenderer.invoke('call-llm-api', userInput),
})
