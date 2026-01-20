const { contextBridge, ipcRenderer } = require('electron')

// 暴露安全的 API 给渲染进程
contextBridge.exposeInMainWorld('electronAPI', {
  // 窗口控制
  minimize: () => ipcRenderer.invoke('window-minimize'),
  close: () => ipcRenderer.invoke('window-close'),
  
  // 获取屏幕信息
  getScreenSize: () => ipcRenderer.invoke('get-screen-size'),
  
  // 窗口拖拽
  startDrag: () => ipcRenderer.invoke('start-drag'),
  
  // 右键菜单
  showContextMenu: () => ipcRenderer.invoke('show-context-menu'),
  
  // 扫描立绘文件
  scanIllustrations: () => ipcRenderer.invoke('scan-illustrations'),
})
