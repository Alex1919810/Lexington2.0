---
name: 打卡功能实现
overview: 在侧边栏添加打卡按钮，实现上班打卡和下班提醒功能，支持配置化的提醒时间和消息文本，以及计时终止和重新开始功能。
todos:
  - id: "1"
    content: 更新 config.json，添加打卡相关配置项（workReminderHours, clockInMessage, clockOutMessage）
    status: completed
  - id: "2"
    content: 在 Sidebar.vue 中添加打卡按钮，包括图标、状态显示和点击事件处理
    status: completed
  - id: "3"
    content: 在 App.vue 中添加打卡状态管理、定时器管理和配置加载逻辑
    status: completed
  - id: "4"
    content: 实现打卡处理函数：开始打卡、终止计时、重新开始的功能
    status: completed
  - id: "5"
    content: 实现下班提醒函数：显示提醒对话框并重置状态
    status: completed
---

# 打卡功能实现计划

## 功能需求

1. 侧边栏增加打卡按钮（使用表情符号图标）
2. 点击打卡按钮后显示打卡成功对话框提醒
3. 12小时后自动显示下班提醒对话框，按钮状态重置
4. 支持在配置文件中自定义提醒时间和消息文本
5. 支持终止计时：打卡中再次点击按钮可终止计时，再次点击重新开始

## 实现方案

### 1. 配置文件更新 (`config/config.json`)

在配置文件中添加打卡相关配置项：

- `workReminderHours`: 提醒时间（小时数，默认12）
- `clockInMessage`: 打卡成功提醒文本（默认"打卡成功！"）
- `clockOutMessage`: 下班提醒文本（默认"下班时间到了！"）

### 2. 侧边栏组件更新 (`src/components/Sidebar.vue`)

- 添加打卡按钮，使用表情符号图标（⏰）
- 添加按钮状态管理（是否在打卡中）
- 添加按钮点击事件处理函数
- 通过 emit 向父组件发送打卡事件
- 按钮样式根据打卡状态变化（例如：打卡中时显示不同颜色）

### 3. 主应用组件更新 (`src/App.vue`)

- 添加打卡状态管理（`isClockedIn`）
- 添加定时器管理（`clockOutTimer`）
- 添加配置加载逻辑（从 `config.json` 读取打卡配置）
- 实现打卡处理函数：
  - 如果未打卡：开始打卡，显示打卡成功对话框，启动定时器
  - 如果已打卡：终止计时，清除定时器，重置按钮状态
  - 终止后再次点击：重新开始打卡流程
- 实现下班提醒函数：显示下班提醒对话框，重置按钮状态
- 在组件卸载时清理定时器

### 4. 数据流

```
用户点击打卡按钮
  ↓
Sidebar 组件 emit('clock-in')
  ↓
App.vue 处理打卡逻辑
  ↓
设置 dialogText（打卡成功消息）
  ↓
VisualWifi 组件显示对话框
  ↓
启动定时器（12小时后）
  ↓
定时器触发 → 显示下班提醒 → 重置状态
```

## 技术细节

### 定时器管理

- 使用 `setTimeout` 实现12小时后的提醒
- 定时器ID存储在 `clockOutTimer` ref 中
- 支持清除定时器（终止计时功能）

### 状态管理

- `isClockedIn`: 是否正在打卡中
- `clockOutTimer`: 定时器ID（用于清除）

### 配置加载

- 在 `App.vue` 的 `onMounted` 中加载配置
- 使用 `window.electronAPI.getConfig()` 获取配置
- 如果配置不存在，使用默认值

### 按钮状态

- 未打卡：默认样式
- 打卡中：不同背景色（例如：绿色）表示正在计时
- 支持通过 props 传递打卡状态给 Sidebar 组件

## 文件修改清单

1. `config/config.json` - 添加打卡配置项
2. `src/components/Sidebar.vue` - 添加打卡按钮和相关逻辑
3. `src/App.vue` - 添加打卡状态管理和定时器逻辑