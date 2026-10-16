# TraeJumper v1.0.0 Release Notes

发布日期：2026-10-17（v1.0.0 最终版）

## 🎉 版本概述

**TraeJumper 正式发布 1.0.0** 🎊

本版本汇集了 TraeJumper 自 fork 原项目以来的完整功能集，是首个稳定大版本。核心解决了"单活跃 Token 服务端策略"下多账号切换的互踢根因，引入完整的内嵌 WebView 无痕登录 + Token 自动续签让位机制，并补齐 Dashboard 仪表盘、在线自动更新、完整日志系统等生产级能力。

**变更规模**：96 个文件，+17660/-1067 行（含 v1.0.0 RC → 最终版的登录态分类重构、Token 续签开关等打磨）。

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

### 5. OAuth RefreshToken 401 无限重试
**问题**：ExchangeToken 签发新 RefreshToken 时旧的立即失效（OAuth 标准一次性行为）。OAuth 账号首次登录后后台 refresh_all_tokens 每分钟用旧 RefreshToken 调用 ExchangeToken → 401 → 循环。
**修复**：账号重复 OAuth 登录自动走"更新凭证"路径（refresh_token / refresh_expired_at / 密钥对 / device_id / machine_id / host 全部覆盖），不报错。

### 6. OAuth callback 无标准 code
**问题**：Trae OAuth 不发标准 `code` query param，auth code 藏在 `authCodeInfo` JSON 参数的 `AuthCode` 字段。直接按标准拿 code 会报 CBError("missing code")。
**修复**：warp handler percent-decode callback URL 后 JSON parse `authCodeInfo.AuthCode`。

### 7. OAuth WebView Err 分支残留
**问题**：oauth_login.rs 多个 return Err 分支（账号重复、save_store 失败、ExchangeToken 失败、callback 超时、用户取消）遗漏 `close_login_webview_safe` → SUCCESS_HTML 永久留在屏幕上。
**修复**：所有 Err 分支统一三段式——先 `drop(manager)` + `close_login_webview_safe(&app)` + `return Err(msg)`。

### 8. Windows 平台日志 watchdog 空实现
**问题**：`start_log_watchdog` 的 `#[cfg(not(unix))]` 分支是空函数——Windows 平台日志文件被外部删除后不会自动重建，进程持有的 handle 指向已删除路径，println 静默成功但磁盘上不可见。
**修复**：补全 Windows watchdog 实现。Windows 无 inode，改用 `fs::metadata().is_ok()` 存在性检查；重建时用 `SetStdHandle(STD_OUTPUT/ERROR_HANDLE, handle)` 重新重定向（与 `init_logging` Windows 分支一致），`std::mem::forget(file)` 保持 handle 不关闭。

### 9. 设置页目标客户端切换无遮罩层
**问题**：Settings.tsx 的 `switchingApp` 仅用于 `disabled` 按钮防重复点击，切换过程中用户仍可操作其他区域，造成"点了没反应"的错觉或中途打断。
**修复**：Settings.tsx 用 `<>...</>` Fragment 包裹，当 `switchingApp=true` 时渲染 `global-busy-overlay`（z-index: 9999, position: fixed），与账号切换/批量签到复用同款 CSS。遮罩标题「正在切换目标客户端」。

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

### 3. Token 自动续签 + 多客户端让位（可配置开关）

- **后端 tokio interval** 每分钟检查（启动即执行 + 电脑睡眠唤醒后立即触发）
- **Token 自动续签开关**（`auto_renew_enabled`，默认开启）：设置页一键开关，关闭后 Token 过期不会自动续期
- **续签触发条件**：cookies 账号 1h 内过期 → GetUserToken 续签；OAuth 账号 2h 内过期 → RefreshToken 续签
- **自动写回客户端**：活跃账号续签后自动写 storage.json → kill + launch 客户端
- **多客户端让位**：新版自管理客户端活跃账号完全让位，TraeJumper 不调 GetUserToken / ExchangeToken
- **手动续签写回**：主动触发续签并写给客户端（不重启运行中的客户端）

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

### 9. 登录态双维度分类体系（v1.0.0 最终重构）

**来源类型**（`AccountLoginSource`，持久化，4 种 + 1 兜底）：

| 枚举 | 展示名 | 打标入口 |
|------|--------|---------|
| `WebviewOauth` | WebView · OAuth自管 | OAuth 授权登录入口 |
| `WebviewCookie` | WebView · Cookie自管 | 旧版 WebView cookies 登录窗口 |
| `ClientImport` | Trae客户端读取 | `read_trae_ide_account()` storage.json 导入 |
| `ManualToken` | 手动 Token/Cookie | 手动 Token 输入 + JSON 导入 |
| `Unknown` | 未分类 | 存量旧数据（serde alias 兼容旧 `"webview"` / `"cookie"`） |

**客户端激活类型**（`AccountLoginType`，运行时推导，5 种 injected + 3 种 standalone）：

| 枚举 | 展示名 | 推导条件 |
|------|--------|---------|
| `NativeOauth` | 客户端 OAuth | 客户端活跃 && TraeJumper 从未注入（客户端原生登录，TraeJumper 让位） |
| `InjectedOauth` | TJ OAuth | 客户端活跃 && 来源=WebviewOauth（TraeJumper OAuth 注入） |
| `InjectedCookie` | TJ Cookie | 客户端活跃 && 来源=WebviewCookie（cookies 模式注入） |
| `InjectedClient` | TJ 客户端 | 客户端活跃 && 来源=ClientImport |
| `InjectedManual` | TJ 手动 Token | 客户端活跃 && 来源=ManualToken |
| `StandaloneOauth` | （不显示） | 客户端不活跃 && oauth_enabled=true |
| `StandaloneCookie` | （不显示） | 客户端不活跃 && 有 cookies |
| `StandaloneManual` | （不显示） | 客户端不活跃 && 纯手动 Token |

**关键设计**：
- 两个维度**独立显示**：来源标签（`WebView · OAuth自管`）始终显示在账号名旁；激活类型（`TJ OAuth`）仅在客户端活跃时拼入"当前"标签副标题
- runtime 脏数据修正：OAuth 授权更新路径强制覆盖 `login_source`；存量账号按 `oauth_enabled` / `has_cookies` 推断 Unknown
- serde 兼容：旧 `"webview"` / `"cookie"` 通过 alias 映射到 `Unknown`，runtime 再推断还原

**示例**：
```
✓ TRAE CN & TraeWork 当前 · TJ OAuth    ← 来源=WebviewOauth，活跃于 2 个客户端
✓ TRAE CN 当前 · 客户端 OAuth           ← 客户端原生 OAuth 登录，TraeJumper 让位
  （无当前）  WebView · Cookie自管       ← 仅库内，客户端未登录
```

### 10. 数据导入导出

- 导出 JSON 完整包含 cookies、jwt_token、user_id、tenant_id、签到设备档案、OAuth 密钥对 + refresh_token 等核心凭据
- 导入可完整恢复，支持从 v0.9.x 数据平滑升级

### 11. OAuth Native 完整实现

**两个新 Rust 模块（1500+ 行）**
- `oauth.rs`：EC P-256 密钥对、PKCE S256、DeviceProof 签名（x-device-signature + x-device-proxy-credential）、ExchangeToken HTTP 客户端、decode_jwt_payload
- `oauth_login.rs`：warp callback server（`/oauth/trae/callback` + `/oauth/cancel`）、`create_oauth_login_webview`（非 incognito，持久 cookies + resize 兼容）、SUCCESS_HTML（白底 spinner + ExchangeToken 串行等待）、阶段事件 `oauth-phase=opening/waiting/exchanging`

**OAuth 持久化字段（Account 8 个新字段，全 `#[serde(default)]`）**
- `oauth_enabled` / `oauth_refresh_token` / `oauth_refresh_expired_at` / `oauth_public_key_pem` / `oauth_private_key_pem` / `oauth_device_id` / `oauth_machine_id` / `oauth_host`
- AccountBrief 新增 `oauth_enabled` + `oauth_refresh_expired_at`（两个 From 实现都投影）

**后台 RefreshToken 续签集成**
- `refresh_all_tokens` step4 新增 OAuth 分支：检测 `account.oauth_enabled` → 调 ExchangeToken(Refresh) 自动续签 → 更新 refresh_token / refresh_expired_at / jwt_token
- ExchangeToken host 优先用 `account.oauth_host`（来自 callback），fallback `variant.api_host`

**前端 OAuth 状态机（AddAccountModal + UpdateTokenModal 共用）**
```
idle → opening（spinner + oauth-loading.html 中转）
     → waiting（warp emit，footer 状态行）
     → exchanging（SUCCESS_HTML 白底 spinner + footer 同步）
     → resolve 成功 / reject 失败
```
- UpdateTokenModal 新增 OAuth tab，顺序：OAuth 授权 → 从 Trae 客户端读取 → 浏览器登录（旧）→ 手动输入 Token

**oauth-loading.html 本地中转页**
- `WebviewUrl::App("oauth-loading.html".into())` → initialization_script 注入 `window.__OAUTH_TARGET__` → JS 检测到后 `location.replace(真实 authorize_url)`
- 为什么需要：WKWebView 在远程授权页首字节到达前持续渲染当前页 → 用本地 loading 页覆盖 10-22s TTFB 白屏
- "切换账号"按钮：销毁当前 webview → 无痕 incognito 重建 → 换号重新授权

## 🎨 UI/UX 优化

| 区域 | 优化 |
|------|------|
| 账号卡片 | 多客户端"当前"标签（如 TraeCode & TraeWork 当前）、Token 过期状态细化 + 按小时判断（因 cookies 续签 Token 仅 8h）、时间格式 yyyy-MM-dd HH:mm:ss 窄宽度换行 |
| 登录弹窗 | 内嵌 webview + 底部操作栏、状态行（错误红色提示）、账号已存在自动 toast 关闭 |
| 设置页 | 通用设置区（日志 watchdog + 自动刷新 + 自动签到）、客户端卡片合并（目标应用 / 路径 / 机器码） |
| 右键菜单 | 四边钳制 + 最大高度 calc(100vh - 16px) + 垂直滚动；移除「复制 Token」保留详情页复制 |
| 详情弹窗 | Token / Cookies 旁显示过期时间 + 状态颜色预警（绿/橙/红）；用量明细日期筛选（快捷 + 自定义）；**RefreshToken 凭证行**（OAuth 账号专属） |
| About 页 | 应用介绍、功能特性、技术栈全量更新 |
| 窗口标题 | 动态同步当前客户端名称（TraeJumper · Trae CN） |
| **批量签到全屏 loading 遮罩** | 深色半透明 rgba(20,24,33,0.55) + backdrop-filter blur(4px) + 居中白色卡片 + 蓝色 spinner；z-index 9999 覆盖所有交互；提示"正在批量签到 N 个账号，请勿切换或关闭" |
| **OAuth WebView 白屏修复** | oauth-loading.html 本地中转页先加载（自带 spinner），initialization_script 注入目标授权 URL 后 location.replace 跳转；覆盖 WKWebView TTFB 10-22s 白屏 |
| **OAuth 流程阶段化状态** | 后端 emit `oauth-phase` 事件（opening/waiting/exchanging）；前端同步显示 spinner 文案；ExchangeToken 期间 SUCCESS_HTML（白底 spinner）持续覆盖 webview，后端 exchange 成功后才关 |
| **Tab 顺序统一** | AddAccountModal / UpdateTokenModal 四个 tab 统一顺序：OAuth 授权 → 从 Trae 客户端读取 → 浏览器登录（旧）→ 手动输入 Token；所有 tab 统一 fullscreen 布局 |
| **UpdateTokenModal 新增 OAuth tab** | 完整复用 AddAccountModal 的 OAuth 状态机；"取消登录" / "切换账号"（无痕重建 webview 换号）底部双按钮 |
| **OAuth 登录态标签** | 双维度体系：来源标签（WebView · OAuth自管 / WebView · Cookie自管 / Trae客户端读取 / 手动 Token/Cookie）始终显示；激活类型仅在客户端活跃时拼入"当前"标签，用 **TJ** 简称（TJ OAuth / TJ Cookie / TJ 客户端 / TJ 手动 Token / 客户端 OAuth） |
| **所有 Err 分支统一关 webview** | oauth_login.rs 6 处 return Err 全部先 close_login_webview_safe + drop manager，不漏网 |
| **Windows 进程检测** | `machine.rs` 适配 Windows：多版本进程枚举（tasklist）、变体安装路径扫描（注册表 HKLM\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall + 目录 fallback） |
| **设置页目标客户端切换遮罩** | 切换目标客户端时显示全屏 loading 遮罩（与批量签到/账号切换复用 `global-busy-overlay`），防止中途打断 |

## 🔧 技术改进

### CI 签名构建
- 上传 `.app.tar.gz` + `.sig` 作为 updater artifacts
- `merge-updater-json.py` 从 `.sig` 提取签名，自动生成跨平台 `latest.json`
- `.gitignore` 新增密钥防泄露规则（`*.key` / `*.priv`）

### CheckinConfig 扩展
- 新增可配置项：`auto_refresh_enabled`（默认 true, 10 分钟）、`auto_refresh_interval`（5/10/30/60）、`auto_renew_enabled`（默认 true，Token 自动续签开关）、`auto_checkin_enabled`（默认 false, 22:00）、`log_watchdog_enabled`（默认 true, 5 秒）、`log_watchdog_interval`（2/5/10/30/60）

### 日志 watchdog Windows 跨平台实现
- **Unix 版**（macOS/Linux）：检查 `(dev, ino)` 元组，重建后 `libc::dup2(fd, 1/2)` 重定向
- **Windows 版**（补全空函数）：无 inode，改用 `fs::metadata().is_ok()` 存在性检查；重建后 `SetStdHandle(STD_OUTPUT/ERROR_HANDLE, handle)` 重定向；`std::mem::forget(file)` 保持 handle 不关闭
- 共享原子变量 `LOG_WATCHDOG_ENABLED` / `LOG_WATCHDOG_INTERVAL`，两平台行为一致

### 版本号同步三端
- `npm run version:set 1.0.0` 自动同步 package.json / tauri.conf.json / Cargo.toml
- Vite 自动注入 `VITE_APP_VERSION` 到前端

### macOS Tauri v2 子窗口兼容
- 添加子 WebView 后显式调用 `set_resizable(true)` + `set_maximizable(true)` 恢复主窗口可调整大小

### OAuth WebView 本地中转页（oauth-loading.html）
- Tauri 2 `WebviewUrl::App("oauth-loading.html".into())` 指向 `public/` 静态文件（Vite 构建时原样拷贝到 dist/，Tauri 打包时一起进 bundle）
- 为什么中转：WKWebView 在远程授权页首字节到达前持续渲染当前页 → 用本地 loading 页覆盖 10-22s TTFB 白屏 → `location.replace` 跳真实 URL

### 签到请求自动重试
- `post_checkin_with_retry`：checkin_status / checkin(claim) 连接错误或超时自动重试 3 次，退避 800ms × n
- 解决 api.trae.cn 连接抖动时签到偶发失败（无重试直接跳过）

### OAuth device_id 固定策略
- 首次 OAuth 授权用固定的 `get_trae_device_id()` 而非每次 `Uuid::new_v4()` 生成
- 避免 ExchangeToken 触发设备绑定限制（20401: device limit reached，每账号 5 台设备上限）
- machine_id 用 machine_guid SHA256 派生，稳定且跨重装一致

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
