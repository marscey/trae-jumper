# TraeJumper v1.0.0 Release Notes

发布日期：2026-10-10

## 🎉 版本概述

**TraeJumper 正式发布 1.0.0** 🎊

本版本汇集了 TraeJumper 自 fork 原项目以来的完整功能集，是首个稳定大版本。核心解决了"单活跃 Token 服务端策略"下多账号切换的互踢根因，引入完整的内嵌 WebView 无痕登录 + Token 自动续签让位机制，并补齐 Dashboard 仪表盘、在线自动更新、完整日志系统等生产级能力。

**变更规模**：93 个文件，+5442/-881 行。

## 🚨 关键问题修复

### 1. 客户端"登录已失效"根因修复（彻底解决）

**问题链**：
- Trae 服务端为单活跃 Token 策略，`GetUserToken` 签发新 Token 时作废旧 Token
- 新版客户端（storage.json 含 `icube-dc:*` 设备密钥键）采用 OAuth refresh token 自行续签，TraeJumper 调用 `GetUserToken` 会作废其自管的新 Token
- 旧版活跃账号检测只检查当前目标客户端，账号在非目标客户端活跃时让位检测失败 → TraeJumper 误调 `GetUserToken` 踢掉另一个客户端
- 切换账号时写入**死 Token**到客户端 storage.json → 客户端启动校验失败触发 `clearUserInfo` 登出

**修复**：
- `refresh_all_tokens` step 2.5：账号活跃于任何新版自管理客户端时 **完全让位**——不调 `GetUserToken`、不写 `icube.cloudide`
- 多客户端活跃扫描：遍历所有已安装变体，收集 `active_in_clients` 列表
- 切换账号 Token 有效性守卫：写入前先用 entitlement 校验 Token 存活，死 Token 时先尝试 cookies 续签
- `current_account_id` 记录残留不再误拦截正向切换，仅客户端真实持有会话时才提示"已在使用"

### 2. 新版客户端登录态读取适配

**问题**：TraeCode CN 更新后认证 key 从固定值 `iCubeAuthInfo://icube.cloudide` 改为带设备 ID 的 `iCubeAuthInfo://icube-dc:<device_id>`

**修复**：新增设备密钥键遍历；顶层存在 `icube-dc:*` 键即标记 `ClientLogin.self_managed = true`。

### 3. macOS 进程检测漏判

**问题**：`pgrep -f` 无法读取 TraeWork 主进程过长的命令行，导致进程检测漏判

**修复**：改用 `ps -axo pid=,command=` 全量扫描。

### 4. Windows 平台编译失败

**问题**：macOS 专属函数未加 Windows 空实现；`libc` crate 在 Windows 不导出 Win32 API

**修复**：补全条件编译实现 + 改用 `windows-sys` 调用。

## ✨ 新功能

### 1. 一键切换客户端账号（核心）

- 自动杀进程 → 清除旧登录态 → 写入新账号 Token → 重新打开客户端
- **跨客户端冲突检测**：切换前遍历所有已安装变体，账号已在其他客户端活跃时提示二次确认（`force=true` 可强制切换）
- **Token 有效性守卫**：写入前 entitlement 接口校验 + 死 Token 自动 cookies 续签；**绝不把死 Token 写入客户端**

### 2. 内嵌 WebView 无痕登录

- 内嵌子 WebView（`trae-login-child`）+ 手动确认流程（已移除旧 warp HTTP 回调）
- 无痕隔离会话（每次登录独立，互不顶号）
- 从 WKHTTPCookieStore 直接读取 **完整 HttpOnly cookies**（绕过 JS document.cookie 限制），用于后续 Token 自动续签
- 支持新增账号 + 更新指定账号 Token 两种场景

### 3. Token 自动续签 + 多客户端让位

- **后端 tokio interval** 每分钟检查（启动即执行 + 电脑睡眠唤醒后立即触发）
- Token 快过期（2h 内）时 GetUserToken 续签 → 写 storage.json → kill + launch 客户端
- **多客户端让位**：新版自管理客户端活跃账号完全让位，TraeJumper 不调 GetUserToken
- **手动续签写回**：主动触发续签并写给客户端（不重启运行中的客户端，用于测试和应急）

### 4. 每日自动签到领积分

- 可配置触发时间点（默认 22:00），防跨零点策略 + 内存去重（同一天只执行一次）
- **签到虚拟设备档案**：每个账号独立分配 session-id / market-user-id / device-id / device-brand
- **设备 ID 双策略**：真实设备前缀 / FNV-1a 哈希安全区间 [1e15, 4.5e15)
- **风控自愈**：自动检测非法 device-id（≥4.5e15）并重生成
- 单账号签到 / 批量签到 / 签到状态查询，带账号专属随机延迟防风控

### 5. Dashboard 仪表盘

- 欢迎横幅 + 客户端徽章 + 四张统计卡（总积分/总配额、已使用、剩余可用、平均剩余）
- 使用量分布饼图 + 套餐分布饼图（Recharts 3）
- 账号概览预览卡片（前 4 个账号进度条 + 预警色）
- 自动检测 `is_credits_billing`，积分/配额双模式智能切换

### 6. 在线自动更新

- 集成 tauri-plugin-updater / tauri-plugin-process
- 「关于」页检查、下载、安装一键完成
- 更新包经 Ed25519 签名校验
- Windows 静默安装（passive），macOS 拖拽 DMG

### 7. 完整日志系统

- stdout/stderr 重定向落盘（macOS: Console.app 默认可见；Windows: %LOCALAPPDATA%）
- 5MB 自动轮转
- **运行中 Watchdog**：后台线程每 N 秒检查 (dev, ino)，日志文件被外部删除/替换时自动重建并重新 dup2
- 日志仅保留关键信息：添加账号 cookies、续签候选原因、GetUserToken 调用详情、活跃账号判定、安全保护触发

### 8. 多应用变体 + 侧边栏

- 同时支持 TraeCode CN / TraeWork CN / 国际版 Trae
- 设置页切换目标应用，机器码、安装路径、登录站点、API 端点自动跟随
- 侧边栏可收缩（手动 / 窄窗口自动），切换应用按钮移至底部 footer

### 9. 登录来源打标

- `AccountLoginSource`：ClientImport / Webview / Cookie / ManualToken
- `AccountLoginType`：NativeOAuth / Injected / Standalone
- 账号卡片显示来源标签（切号注入·来源 / 原生OAuth / 来源）

### 10. 数据导入导出

- 导出 JSON 完整包含 cookies、jwt_token、user_id、tenant_id、签到设备档案等核心凭据
- 导入可完整恢复，支持从 v0.9.x 数据平滑升级

## 🎨 UI/UX 优化

| 区域 | 优化 |
|------|------|
| 账号卡片 | 多客户端"当前"标签（如 TraeCode & TraeWork 当前）、Token 过期状态细化 + 按小时判断（因 cookies 续签 Token 仅 8h）、时间格式 yyyy-MM-dd HH:mm:ss 窄宽度换行 |
| 登录弹窗 | 内嵌 webview + 底部操作栏、状态行（错误红色提示）、账号已存在自动 toast 关闭 |
| 设置页 | 通用设置区（日志 watchdog + 自动刷新 + 自动签到）、客户端卡片合并（目标应用 / 路径 / 机器码） |
| 右键菜单 | 四边钳制 + 最大高度 calc(100vh - 16px) + 垂直滚动；移除「复制 Token」保留详情页复制 |
| 详情弹窗 | Token / Cookies 旁显示过期时间 + 状态颜色预警（绿/橙/红）；用量明细日期筛选（快捷 + 自定义） |
| About 页 | 应用介绍、功能特性、技术栈全量更新 |
| 窗口标题 | 动态同步当前客户端名称（TraeJumper · Trae CN） |

## 🔧 技术改进

### CI 签名构建
- 上传 `.app.tar.gz` + `.sig` 作为 updater artifacts
- `merge-updater-json.py` 从 `.sig` 提取签名，自动生成跨平台 `latest.json`
- `.gitignore` 新增密钥防泄露规则（`*.key` / `*.priv`）

### CheckinConfig 扩展
- 新增可配置项：`auto_refresh_enabled`（默认 true, 10 分钟）、`auto_checkin_enabled`（默认 false, 22:00）、`log_watchdog_enabled`（默认 true, 5 秒）

### 版本号同步三端
- `npm run version:set 1.0.0` 自动同步 package.json / tauri.conf.json / Cargo.toml
- Vite 自动注入 `VITE_APP_VERSION` 到前端

### macOS Tauri v2 子窗口兼容
- 添加子 WebView 后显式调用 `set_resizable(true)` + `set_maximizable(true)` 恢复主窗口可调整大小

## 📦 安装包

| 平台 | 文件 |
|------|------|
| macOS (Apple Silicon) | `TraeJumper-1.0.0-mac-arm64.dmg` |
| macOS (Intel) | `TraeJumper-1.0.0-mac-x64.dmg` |
| Windows x64 (NSIS) | `TraeJumper-1.0.0-win-x64-setup.exe` |
| Windows x64 (MSI) | `TraeJumper-1.0.0-win-x64.msi` |

## 📋 升级建议

- **历史版本**可直接覆盖安装
- **重要**：若之前遇到过客户端被踢登出，本版本已彻底修复根因——升级后客户端登录态不会再被 TraeJumper 误操作
- 自动刷新、自动签到、日志 watchdog 的开启状态已在设置页标明，可按需调整
- macOS 日志查看：Console.app 搜索 `traejumper`，或直接打开 `~/Library/Logs/traejumper/trae-jumper.log`

## 🛠️ 技术栈

- **前端**：React 19 / TypeScript / Vite 7 / Recharts 3
- **后端**：Tauri 2 / Rust / Tokio / Reqwest / Serde
- **加密**：AES-128-CBC + SHA-512（兼容 Trae storage.json）
- **自动更新**：Tauri Updater 2 + Ed25519 签名校验
- **平台**：Windows（NSIS/MSI）、macOS（DMG）

## 🙏 致谢

感谢 fork 原项目 [Yang-505/Trae-Account-Manager](https://github.com/Yang-505/Trae-Account-Manager) 的基础工作，感谢所有反馈客户端登录失效问题的用户。这个 Bug 链路较长（单活跃 Token 策略 → 多客户端扫描缺失 → 写入死 Token），最终通过完整的根因分析 + 多客户端全局检测 + 写入前 Token 守卫三道防线彻底解决。

**v1.0.0 里程碑** — TraeJumper 已从 fork 原型演进为生产级多账号管理工具。
