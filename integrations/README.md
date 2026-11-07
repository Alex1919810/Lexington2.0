# 看看Lexington怎么说

点击菜单即发送并开始处理，回复显示在桌宠气泡；不会先填输入框，不修改剪贴板。Lexington 未启动时自动启动，正在处理时排队（最多 16 条），复用当前会话及审批设置。选中文字、来源、行号与文件路径以 JSON 传递，单次最多 1 MiB、32 个文件。外部请求处理完保留输入框草稿。

## 安装

项目运行 `bash scripts/install-linux.sh` 时安装客户端和本机通信桥；不自动修改浏览器的安全策略。菜单必须由各软件的扩展提供，不能注入所有软件的原生菜单。

### Windows 文件资源管理器

运行 `powershell -ExecutionPolicy Bypass -File scripts/install-windows.ps1`，在 HKCU 注册文件/文件夹的“看看Lexington怎么说”。Windows 11 经典菜单通常位于“显示更多选项”，不代表新菜单的原生扩展。命令直接启动带引号的 exe 和本地路径，不通过 shell。VS Code 默认程序为 `%LOCALAPPDATA%\lexington\lexington.exe`。

浏览器宿主注册在 HKCU 的 Chrome/Chromium/Edge/Brave/Firefox NativeMessagingHosts。`lexington-native-host.exe` 是同一 Rust 二进制的别名，stdout 仅协议帧；启动桌宠时转交同目录 lexington.exe。Chromium key/宿主白名单与产物保持一致；扩展仍需浏览器加载，Firefox 持久安装仍需签名。宿主注册和边界测试不代表所有浏览器已端到端验证。

### GNOME 文件管理器（Nautilus）

选择本地文件或文件夹，右键 → **脚本 → 看看Lexington怎么说**。安装器已创建此脚本；发送绝对路径，让 Lexington 自行查看内容。支持多个文件，路径中的空格、引号和换行不会被当成命令执行。

顶层菜单提供器位于 `nautilus/lexington_menu.py`，安装时复制到 `~/.local/share/nautilus-python/extensions/`（遵循 XDG_DATA_HOME）。安装器会在 Ubuntu/Debian 自动检查并通过系统授权安装 `python3-nautilus` 与 Nautilus 4.0 typelib；使用 `--skip-optional` 跳过时需自行安装。其他发行版需要系统安装 `nautilus-python`。Ubuntu 可由管理员执行 `sudo apt install python3-nautilus`，之后退出并重新打开文件管理器以加载顶层菜单；缺少依赖时仍可使用“脚本”子菜单。安装器不强制关闭用户的文件管理器。

### VS Code

安装器检测到 `code` 时，自动安装 `artifacts/lexington-context.vsix`。如果当前窗口没有出现菜单，运行 VS Code 命令 **Developer: Reload Window**；也可在扩展页选择 **Install from VSIX** 手动安装。

- 编辑器选中文字 → 右键 **看看Lexington怎么说**：发送选区、本地绝对路径与起始行号。
- 文件资源管理器右键文件 → 同名菜单：发送文件路径。
- 未保存文档发送选区并标明无磁盘路径；远程/虚拟文档发送 URI，不能伪造本机路径。远程文件资源管理器不提供本地文件菜单。
- 非默认安装位置可设置 `lexington.executable`。扩展作为本地 UI 扩展运行，远程选区来源不代表本机 Codex 能访问远端文件。

### Chromium / Chrome / Edge / Brave

打开浏览器扩展管理页，启用开发者模式，选择“加载已解压的扩展”，选择安装目录中的 `integrations/artifacts/chromium/`。例如：

```text
~/.local/share/lexington/integrations/artifacts/chromium/
```

选中文字后右键 **看看Lexington怎么说**。网页附带页面/框架 URL；本地 `file://` 页可取得路径，浏览器可能需要开启“允许访问文件网址”。扩展只申请 contextMenus/nativeMessaging，不读取整个页面，也不监听日常选区。公开扩展 key 仅固定扩展 ID，Native Messaging 仅允许该 ID。

### Firefox

已生成 `integrations/artifacts/lexington-firefox.xpi`。正式 Firefox 持久安装需要 Mozilla 签名，当前包未签名；开发验证可打开 `about:debugging#/runtime/this-firefox`，点击“临时载入附加组件”，选择 `integrations/artifacts/firefox/manifest.json`，浏览器重启后需要重新加载。正式分发时应签名此包，保留 ID `lexington-context@lexington.local`。

安装器注册 `com.lexington.context` 本机桥。浏览器的 Snap/Flatpak 沙箱可能限制启动本机程序，写入宿主清单不保证沙箱浏览器可访问；若提示找不到本机应用，可用普通系统安装的浏览器验证，或按其沙箱/门户支持配置 Native Messaging。当前 Ubuntu Snap Firefox 的端到端调用尚未验证。

发送失败时扩展工具栏显示 `!`，悬停查看原因。网页没有本地磁盘路径，使用真实 URL 作为来源。

## 程序接口与验证

```bash
lexington --ask-files /absolute/path/file.rs
printf '%s' '{"text":"选中文字","files":["/absolute/path/file.rs"],"source":"编辑器","line":12}' | lexington --ask-json
python3 scripts/context-smoke.py target/release/lexington
node scripts/test-integrations.js
```

`--ask-json` 从 stdin 接收单个 Request；`--native-messaging` 使用浏览器规定的 4 字节小端长度加 JSON 帧。成功 ACK 表示已接收入队，不表示模型已完成。不会自动切换 Codex 的工作目录或放宽沙箱，文件访问仍受当前会话权限约束。通信桥没有监听 HTTP/TCP，也不暴露可由任意网页直接调用的 URL scheme。

Rust 主程序不依赖 Node。`extension.js` 使用 VS Code 自带的扩展宿主；浏览器脚本运行在浏览器里；`scripts/test-integrations.js` 仅开发验证使用 Node。安装/打包脚本使用 Python 3 标准库创建 VSIX/XPI，无 npm 下载。
