# 桌面宠物 - Electron + Vue3

一个基于 Electron 和 Vue 3 的桌面宠物应用，支持立绘展示、对话交互、打卡提醒等功能。

## 功能特性

- **立绘展示** - 支持 GIF 和 PNG 格式的立绘图片，按角色分类管理
- **对话系统** - 点击宠物显示对话框，支持打字机效果，可配置自动关闭
- **LLM 对话** - 集成大模型 API，支持与宠物进行对话交互
- **打卡功能** - 上班打卡计时，定时提醒下班
- **角色切换** - 支持多个角色之间的切换，每个角色有独立的立绘和对话文本
- **格式切换** - 同一角色支持 GIF/PNG 两种立绘格式切换
- **自定义背景** - 支持对话框和输入框的自定义背景图片
- **侧边栏控制** - 刷新立绘、切换立绘、切换格式、切换角色、打卡等功能按钮
- **可拖拽窗口** - 通过立绘区域拖拽移动窗口位置
- **始终置顶** - 透明无边框窗口，始终保持在桌面最前

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
├── electron/              # Electron 主进程文件
│   ├── main.js           # 主进程入口
│   ├── preload.js        # 预加载脚本
│   ├── window.js         # 窗口管理
│   ├── config.js         # 配置管理
│   ├── fileScanner.js    # 文件扫描
│   ├── llmApi.js         # LLM API 调用
│   ├── ipcHandlers.js    # IPC 处理器
│   └── menu.js           # 右键菜单
├── src/                  # Vue 3 应用源码
│   ├── components/       # Vue 组件
│   │   ├── VisualWifi.vue      # 主立绘组件
│   │   ├── DialogBubble.vue    # 对话框组件
│   │   ├── InputDialog.vue     # 输入框组件
│   │   └── Sidebar.vue         # 侧边栏组件
│   ├── composables/      # Vue 组合式函数
│   │   └── useHotReload.ts     # 热加载管理
│   ├── App.vue           # 根组件
│   ├── main.js           # 应用入口
│   └── style.css         # 全局样式
├── resource/             # 资源文件夹
│   ├── illustration/     # 立绘图片 (按角色/格式分类)
│   │   └── 角色名/
│   │       ├── gif/      # GIF 格式立绘
│   │       └── png/      # PNG 格式立绘
│   ├── background/       # 背景图片
│   │   ├── dialog/       # 对话框背景
│   │   └── input/        # 输入框背景
│   ├── texts/            # 角色文本
│   │   └── 角色名/
│   │       └── text.json # 角色对话文本 (JSON 数组)
│   └── icon/             # 应用图标
├── config/               # 配置文件夹
│   └── config.json       # 应用配置
├── index.html            # HTML 模板
├── vite.config.js        # Vite 配置
└── package.json          # 项目配置
```

## 配置说明

在 `config/config.json` 中配置应用：

```json
{
  "dialog": {
    "width": 200,
    "maxHeight": 120,
    "offsetX": 100,
    "offsetY": 50,
    "autoCloseDelay": 10
  },
  "llm": {
    "apiUrl": "https://api.openai.com/v1/chat/completions",
    "apiKey": "your-api-key",
    "model": "gpt-3.5-turbo"
  },
  "clockIn": {
    "workReminderHours": 8,
    "clockInMessage": "打卡成功！",
    "clockOutMessage": "下班时间到了！",
    "clockInCancelMessage": "打卡已终止"
  }
}
```

### 配置项说明

| 配置项 | 说明 |
|--------|------|
| `dialog.width` | 对话框宽度 (像素) |
| `dialog.maxHeight` | 对话框最大高度 (像素) |
| `dialog.offsetX` | 对话框水平偏移 |
| `dialog.offsetY` | 对话框垂直偏移 |
| `dialog.autoCloseDelay` | 对话框自动关闭延迟 (秒) |
| `llm.apiUrl` | 大模型 API 地址 |
| `llm.apiKey` | API 密钥 |
| `llm.model` | 模型名称 |
| `clockIn.workReminderHours` | 工作时长 (小时) |
| `clockIn.clockInMessage` | 打卡成功消息 |
| `clockIn.clockOutMessage` | 下班提醒消息 |
| `clockIn.clockInCancelMessage` | 取消打卡消息 |

## 添加角色

1. 在 `resource/illustration/` 下创建以角色名命名的文件夹
2. 在角色文件夹中创建 `gif` 和/或 `png` 子文件夹
3. 将对应格式的立绘图片放入相应文件夹
4. (可选) 在 `resource/texts/角色名/` 下创建 `text.json` 文件，添加对话文本：

```json
[
  "你好！",
  "今天天气真好！",
  "有什么可以帮你的吗？"
]
```

## 侧边栏按钮说明

| 按钮 | 功能 |
|------|------|
| 📌/📍 | 切换侧边栏显示模式 (固定显示/鼠标悬停显示) |
| ↻ | 刷新立绘列表 |
| ▶ | 切换到下一个立绘 |
| G/P | 切换 GIF/PNG 格式 |
| 👤 | 切换角色 |
| ⏰ | 上班打卡/终止计时 |

## 许可证

MIT