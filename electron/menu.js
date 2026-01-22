const { Menu, app } = require('electron')

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

module.exports = {
  createContextMenu
}
