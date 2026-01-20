# 桌面宠物 - Electron + Vue3

一个基于 Electron 和 Vue 3 的桌面宠物应用框架。

## 功能特性

- 🎨 精美的宠物动画效果
- 🖱️ 可拖拽的桌面窗口
- 📌 始终置顶显示
- 🎭 透明无边框窗口
- ⚡ 基于 Vite 的快速开发体验

## 技术栈

- **Electron** - 跨平台桌面应用框架
- **Vue 3** - 渐进式 JavaScript 框架
- **Vite** - 下一代前端构建工具

## 快速开始

### 安装依赖

```bash
npm install
```

### 开发模式

```bash
npm run dev
```

这将同时启动 Vite 开发服务器和 Electron 应用。

### 构建应用

```bash
npm run build
```

### 打包应用

```bash
npm run electron:build
```

## 项目结构

```
├── electron/          # Electron 主进程文件
│   ├── main.js       # 主进程入口
│   └── preload.js    # 预加载脚本
├── src/              # Vue 3 应用源码
│   ├── components/   # Vue 组件
│   ├── App.vue       # 根组件
│   ├── main.js       # 应用入口
│   └── style.css     # 全局样式
├── index.html        # HTML 模板
├── vite.config.js    # Vite 配置
└── package.json      # 项目配置
```

## 自定义宠物

你可以在 `src/components/Pet.vue` 中自定义宠物的外观和动画效果。

## 许可证

MIT
