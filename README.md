<div align="center">

<img src="src/assets/logo.png" alt="TraeJumper" style="width: 120px; height: 120px; margin-bottom: 16px;">

# TraeJumper

TRAE CN / TraeWork CN / 国际版 Trae / 豆包 多账号管理小工具

[![Version](https://img.shields.io/badge/version-1.0.4-blue?style=flat-square)](https://github.com/marscey/trae-jumper/releases)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS-lightgrey?style=flat-square)](#-系统要求)
[![Build](https://img.shields.io/github/actions/workflow/status/marscey/trae-jumper/build.yml?style=flat-square)](https://github.com/marscey/trae-jumper/actions)
[![License](https://img.shields.io/badge/license-MIT-orange?style=flat-square)](#-免责声明)

[功能特性](#-功能特性) • [系统要求](#-系统要求) • [安装](#-安装) • [使用指南](#-使用指南) • [常见问题](#-常见问题) • [技术栈](#-技术栈) • [更新日志](#-更新日志)

</div>

TraeJumper 是一款面向 TRAE 系列 IDE 用户的多账号管理桌面工具，现已扩展支持**豆包（Doubao）桌面客户端账号**。基于 [Tauri 2](https://tauri.app/) 构建，支持在多个账号之间一键切换，实时查看各账号的 Token 使用量与积分配额，内置完整的签到、日志、自动刷新与定时续签能力。所有数据仅保存在本地。

## ✨ 功能特性

**多应用支持**

- 同时支持 TRAE CN（国内版）、TraeWork CN、Trae（国际版）三种应用变体
- 在设置页随时切换目标应用，机器码、安装路径、登录站点、API 端点自动跟随变更
- **多客户端活跃检测**：扫描所有已安装变体，账号在哪个客户端登录一目了然

**账号管理（双平台支持）**

- **双账号体系**：同时支持 **TRAE 系列**（CN / Work / 国际版）与 **豆包（Doubao）** 桌面客户端账号，Tab 切换独立管理、批量操作仅作用于当前平台
- **TRAE 账号添加方式**：内嵌 WebView 无痕登录（HttpOnly Cookies 自动捕获用于续签）/ OAuth Native 授权（EC P-256 + PKCE，ExchangeToken 签发 14 天 access\_token + 180 天 refresh\_token）/ 手动 Token
- **豆包账号添加方式**：**从豆包客户端读取**（推荐，自动解密 macOS 豆包 Chromium Profile Cookies DB，首次需授权系统钥匙串，后续缓存密钥）/ 手动粘贴 Cookie 串
- **一键切换 TRAE 账号**：自动关闭客户端 → 清除旧登录态 → 写入新账号 → 重新打开；切换前 entitlement 校验 Token 有效性，死 Token 自动 cookies 续签；跨客户端冲突检测（防止互踢顶号，可 force 强制切换）
- **登录来源打标**：账号卡片显示来源标签（TRAE 客户端读取 / 豆包客户端读取 / OAuth 授权 / WebView 登录 / 手动 Token | Cookie）
- **批量操作**：选中多个账号后批量刷新、批量签到、批量删除，选中优先（未选=全量，已选=交集）；Tab 切换自动清空跨 Tab 选中集

**Token 生命周期管理**

- **后端定时续签**：后台 tokio 任务每分钟检查，Token 快过期时自动 GetUserToken / ExchangeToken(Refresh) 续签 + 写入客户端 storage.json
- **OAuth RefreshToken 自管**：OAuth Native 授权账号使用 180 天 RefreshToken 自动续签，签发即覆盖（OAuth 标准一次性行为），旧 RefreshToken 立即作废
- **手动续签写回**：主动触发续签并写给客户端（无需等待 Token 过期，用于测试和应急）
- **多客户端让位逻辑**：新版自管理客户端（OAuth refresh token 架构）活跃账号完全让位，TraeJumper 不调用 GetUserToken，防止误作废旧 Token 导致客户端登出
- **切换账号 Token 有效性守卫**：写入前先用 entitlement 接口校验 Token 存活，死 Token 时先尝试 cookies 续签；绝不把失效 Token 写入客户端
- **Token 过期状态细化**：客户端登录中（让位）/ 正常（>3h）/ 即将过期（<3h）+ 相对时长 / 已过期（红色加粗）
- **OAuth 凭证面板**：账号详情、卡片、列表项显示 RefreshToken 有效期；`UpdateToken` 弹窗支持 OAuth 授权刷新

**签到系统（TRAE + 豆包）**

- **TRAE 签到**：虚拟设备档案（session-id / market-user-id / device-id / device-brand）模拟独立设备每日签到；设备 ID 双策略 + 风控自愈（自动检测并重生非法 device-id）
- **豆包签到**：D10 `/alice/ug/campaign/landing/checkin/` 端点（幂等）；Cookie 过期返回 710010702 系统错误时**自动回退豆包客户端登录态**（强制校验 user\_id 防串号）
- 批量签到带账号专属随机延迟防风控；自动签到（默认 22:00）防跨零点 + 内存去重；签到期间全屏 loading 遮罩

**使用量监控**

- 实时展示每个账号的今日/总使用量与剩余额度
- 查看详细使用事件，支持快捷筛选（今天/7天/30天）+ 自定义时间范围，展示 Token 数量与模型信息
- **积分体系支持**：自动识别 TRAE CN / TraeWork 的积分计费模式，展示签到积分、奖励积分（每月登录赠送/老用户福利等）的总额、已用、剩余与到期时间
- **到期预警**：积分到期日期按紧急程度分色显示（红色=0-3天/已过期，橙色=4-7天，绿色>7天），最近到期与最后到期分别展示

**自动刷新**

- 默认开启，每 10 分钟定时拉取账号使用量和积分数据
- 设置页可关闭或调整间隔（5/10/30/60 分钟）
- 后端 tokio interval 驱动，不依赖前端 setInterval，电脑睡眠唤醒后立即生效
- 乐观更新 + 失败回滚，不闪屏

**日志系统**

- **stdout/stderr 重定向落盘**：所有 println!/eprintln! 输出自动写入日志文件
  - macOS：`~/Library/Logs/traejumper/trae-jumper.log`（Console.app 默认可见）
  - Windows：`%LOCALAPPDATA%\traejumper\logs\trae-jumper.log`
- **5MB 自动轮转**：超过阈值时旧日志重命名为 `.old`
- **运行中 watchdog**：后台线程每 N 秒检查日志文件，发现被外部删除/替换则自动重建目录 + 文件并重新重定向
  - macOS/Linux：检查 `(dev, ino)` 元组，重建后 `libc::dup2` 重定向
  - Windows：无 inode，改用 `fs::metadata().is_ok()` 存在性检查；重建后 `SetStdHandle` 重定向
- 日志仅保留关键信息：添加账号时的 cookie 名称列表、续签候选及触发原因、活跃账号判定、安全保护触发

**机器码管理**

- 查看、复制、刷新、重置 Trae 机器码
- 每个账号独立绑定机器码，切换账号时自动更新

**数据管理**

- 将全部账号数据导出为 JSON（完整包含 cookies、jwt\_token、user\_id、tenant\_id 等所有核心凭据），或从 JSON 导入（可完整恢复）
- 一键清空所有数据（危险操作，带二次确认弹窗）

**系统集成**

- 系统托盘：关闭窗口后隐藏到托盘，左键恢复、右键菜单退出
- 单实例运行，避免多开冲突
- 登录窗口独立，不受托盘行为影响
- **自动更新**：侧边栏版本号入口 → 更新弹窗显示版本差异 + Release Notes + 下载进度（签名校验）

**设置页重构（按归属分组）**

```
🔧 通用设置          [所有账号]     ← 置顶：自动刷新、日志 watchdog
🤖 TRAE 客户端       [TRAE 账号专属]  ← 目标应用、机器码、清除登录态
🔑 TRAE Token 续签   [TRAE 账号专属]  ← OAuth RefreshToken 自动续签
✍️ 签到配置          [所有账号]     ← 签到时间 + 设备策略（后两项 TRAE 专属）
📦 数据管理          [所有账号]     ← 导出、导入、清空
```

**安全存储**

- 数据仅保存在本地，不上传云端
- 兼容并读取 Trae 的 AES-128-CBC + SHA-512 加密存储，同时兼容旧版明文格式
- 新版客户端（storage.json 含 `icube-dc:*` 设备密钥键）登录态自动适配读取

## 💻 系统要求

| 平台      | 版本      |
| ------- | ------- |
| Windows | 10 / 11 |
| macOS   | 10.15+  |

> \[!NOTE]
> TRAE 账号需已安装任意一种 TRAE 客户端（TRAE CN / TraeWork CN / 国际版）。豆包账号需已安装豆包桌面客户端（macOS 首次读取需授权系统钥匙串）。

## 📦 安装

### 下载安装包

前往 [Releases](https://github.com/marscey/trae-jumper/releases) 页面下载对应平台的安装包：

- **macOS**：`TraeJumper-1.0.4-mac-arm64.dmg` / `TraeJumper-1.0.4-mac-x64.dmg`
- **Windows**：`TraeJumper-1.0.4-win-x64-setup.exe`

### 从源码构建

```bash
# 克隆仓库
git clone https://github.com/marscey/trae-jumper.git
cd trae-jumper

# 安装依赖
npm install

# 开发模式运行
npm run tauri dev

# 构建生产版本
npm run tauri build
```

版本号同步（三端自动：package.json / tauri.conf.json / Cargo.toml）：

```bash
npm run version:set 1.1.0   # 指定版本
npm run version:sync          # 改 package.json 后同步
```

> \[!TIP]
> 构建产物位于 `src-tauri/target/release/bundle/`。CI 工作流见 `.github/workflows/build.yml`，自动签名 + 创建 GitHub Release。

## 📖 使用指南

### 1. 选择目标应用

首次使用请进入 **设置** 页，在 **目标应用** 区块选择要管理的 Trae 客户端，系统会自动检测已安装的应用。切换过程中会显示全屏 loading 遮罩，防止中途打断。

### 2. 配置 Trae 路径

在设置页 **客户端路径** 部分，点击 **自动扫描** 自动查找应用，或点击 **手动设置** 选择应用文件。

### 3. 添加账号

**TRAE 账号**

- **OAuth Native 授权（推荐）**：添加账号 → **TRAE Tab** → **OAuth 授权** tab → 内嵌 TRAE 官方授权页 → ExchangeToken 签发 14 天 access\_token + 180 天 refresh\_token（loading 中转页覆盖远程连接白屏）
- **TRAE 客户端读取**：TRAE 客户端已登录目标账号 → 从 storage.json 解密读取（含 AES-128-CBC + SHA-512 解密）
- **WebView 登录（旧）**：内嵌无痕隔离会话浏览器 → 完整捕获 JWT Token + HttpOnly Cookies
- **手动 Token**：F12 → Application → Local Storage → 复制 `iCubeAuthInfo` 的 token 值

**豆包账号**

- **从豆包客户端读取（推荐）**：添加账号 → **豆包 Tab** → **从豆包客户端读取**（自动解密 macOS 豆包 Chromium Profile Cookies DB：PBKDF2-HMAC-SHA1 → AES-128-CBC → 剥离 Chrome v24+ SHA256 前缀 → 匹配 sessionid → user\_id；首次需授权系统钥匙串「始终允许」，密钥缓存到本地后不再弹窗）
- **手动粘贴 Cookie**：登录豆包网页版 → F12 → Application → Cookies → 复制完整 cookie 串

**获取 Token 的方法：**

1. 打开 Trae，按 `F12` 打开开发者工具
2. 切换到 `Application` 标签
3. 在 `Local Storage` → `vscode-webview://xxx` 中找到包含 `iCubeAuthInfo` 的键
4. 复制其中的 `token` 值

### 4. 切换账号

点击账号卡片上的 **切换** 按钮并确认。系统会自动：

- 写入前用 entitlement 接口校验 Token 有效性（死 Token 自动 cookies 续签）
- 检测目标账号是否已在其他客户端活跃（多客户端冲突时提示二次确认）
- 关闭当前 Trae → 清除旧登录态 → 写入新账号 → 重新打开 Trae

> \[!WARNING]
> 切换账号前请保存 Trae 中的工作内容。

### 5. 更新账号 Token

**四种方式（与"添加账号"顺序/界面一致）**

1. 在账号卡片右键菜单选择 **更新 Token**（或 UpdateTokenModal）
2. tab 顺序：**OAuth 授权** → **从客户端读取** → **浏览器登录（旧）** → **手动输入 Token**

**方式 A — OAuth 授权刷新（推荐，新增）**

- 直接点 **OAuth 授权登录** → 官方授权页 → ExchangeToken 签发新 RefreshToken（覆盖旧的，OAuth 标准一次性行为）
- 自动更新 refresh\_token / refresh\_expired\_at / 昵称 / 头像，后台定时续签立即用新凭证

**方式 B — 从客户端读取更新**

1. 确保 Trae 客户端已登录目标账号
2. 点击 **从客户端读取更新**，自动解密读取 storage.json 中的登录态

**方式 C — 浏览器登录更新（旧）**

- 内嵌 WebView 自动登录 → 确认登录 → 校验同一用户 → 更新 Token + Cookies

**方式 D — 手动输入新 Token**

直接输入新 Token 即可。

### 6. 手动续签写回客户端

账号卡片/列表项上显示 **写回客户端** 按钮（仅对有 cookies 且非客户端活跃的账号可见），点击即可：

- 立即调用 GetUserToken 续签 Token
- 写入该账号活跃的所有客户端 storage.json（不重启运行中的客户端）

> 用于实测写回是否触发客户端登出，无需等待 8 小时 Token 过期。

### 7. 查看使用量与积分

- **仪表板**：欢迎横幅 + 客户端徽章 + 统计卡片（总积分/配额、已使用、剩余可用、平均剩余）+ 使用量分布饼图 + 套餐分布饼图 + 账号概览预览
- **账号列表**：每张卡片/列表项底部展示积分到期信息 + Token 过期时间 + Cookies 续签时间，按紧急程度分色预警
- **详情页**：切换至 **使用记录** 标签，查看每次使用的时间、Token 数量、模型与请求类型，支持快捷筛选（今天/7天/30天）+ 自定义时间范围

### 8. 签到

**手动签到**

- 单账号：账号卡片右键菜单 → 签到
- 批量签到：设置页签到区域 → 批量签到所有账号

**自动签到**

- 设置页 **自动签到** 区域，开启开关并设置触发时间点（默认 22:00）
- 防跨零点：预留 2 分钟缓冲，每个账号签到前延迟取 min(随机延迟, 剩余时间/剩余待签到数)
- 内存去重：同一天只执行一次

**签到设备管理**

- 设置页可查看/重置所有账号的签到虚拟设备档案
- 单个账号可独立重置（被风控时换指纹）
- 支持切换设备 ID 生成策略

### 9. 自动刷新配置

设置页 **自动刷新** 区域：

- 开关默认开启
- 间隔可选：5 / 10 / 30 / 60 分钟（默认 10）
- 修改即时生效，无需重启

### 10. 日志自动重建

设置页 **通用设置** 区域：

- **日志自动重建** 开关（默认开启）
- **检查间隔** 下拉（5 / 10 / 30 / 60 秒，默认 5）
- 后台线程持续守护，日志文件被外部删除/替换后自动重建并重新重定向
- macOS 下可在 Console.app 搜索 `traejumper` 查看日志

### 11. 管理机器码

进入 **设置** 页，在 **机器码** 区域可复制、刷新或重置机器码；**清除登录状态** 会重置机器码并删除本地缓存数据。

### 12. 数据导入导出

在设置页 **数据管理** 区域，点击 **导出** 将全部账号数据保存为 JSON（完整包含 cookies、jwt\_token、user\_id、tenant\_id 等核心凭据）；点击 **导入** 从 JSON 恢复账号数据；点击 **清空** 删除全部数据（需二次确认）。

## ⚠️ 免责声明

> \[!WARNING]
> 本工具仅供学习和技术研究使用。使用过程中可能涉及绕过软件账号切换限制，使用者需自行评估并承担全部风险；请勿用于商业用途，不得用于绕过软件正当授权机制。

## ❓ 常见问题

### macOS 提示"TraeJumper 已损坏，无法打开"

macOS 的 Gatekeeper 会对非 App Store 下载的 App 附加 `com.apple.quarantine` 扩展属性。首次打开前需要手动清除：

```bash
sudo xattr -rd com.apple.quarantine /Applications/TraeJumper.app
```

如果 App 名称不同（如 `TraeJumper-beta.app`），路径相应调整。执行完后直接双击 App 即可。

### macOS 首次读取豆包客户端 Cookie 弹钥匙串授权

第一次点击「从豆包客户端读取」时，系统会弹窗："TraeJumper 想要访问'Doubao Safe Storage'中存储的信息"。请点 **始终允许**，密钥会被缓存到本地，后续不再弹窗。

### 豆包签到返回 `710010702 系统错误`

这是豆包服务端风控——签到活动仅对**豆包桌面客户端登录态**开放，网页端 Cookie 恒定返回此错误。TraeJumper 已内置自动回退逻辑：账号 Cookie 过期返回 710010702 时，自动读取本机豆包客户端登录态重试（无需手动操作）。如果仍失败，确认豆包客户端已登录该账号。

### 切换 TRAE 账号后客户端自动登出

大概率是写入的 Token 已失效。TraeJumper 切换前会用 entitlement 接口校验 Token 有效性，死 Token 会先尝试 cookies 续签；但如果 cookies 也过期（约 14 天空闲），则续签失败、写入死 Token → 客户端重启校验失败 → 自动登出。

**解决办法**：重新通过 OAuth Native 授权或 TRAE 客户端读取方式更新该账号。

### OAuth 账号 RefreshToken 401

ExchangeToken 签发新 RefreshToken 时旧的**立即失效**（OAuth 标准一次性行为）。如果手动恢复了旧的 `accounts.json` 备份，里面存的 RefreshToken 可能已被轮换作废。重新走一遍 OAuth 授权登录即可。

### Windows 下进程检测失败

`machine.rs` 依赖 `tasklist` 枚举进程。如果 Trae 进程名变更或被混淆，检测可能失败。请确认已安装的是正版 TRAE CN / TraeWork CN 客户端。

## 🛠️ 技术栈

- **前端**：React 19 / TypeScript / Vite 7 / Recharts 3 / lucide-react
- **后端**：Tauri 2 / Rust / Tokio / Reqwest / Serde / PBKDF2
- **加密**：AES-128-CBC + SHA-512 + PBKDF2-HMAC-SHA1
- **macOS 集成**：Chromium Cookies 解密（安全钥匙串 → PBKDF2 → AES-128-CBC → v24+ SHA256 前缀剥离）
- **平台**：Windows（NSIS/MSI）、macOS（DMG）
- **自动更新**：Tauri Updater 2 + Ed25519 签名校验

## 📁 项目结构

```
trae-jumper/
├── src/                     # 前端源码
│   ├── components/          # React 组件（账号卡片、弹窗、右键菜单等）
│   ├── pages/               # 页面（仪表板、设置、关于）
│   ├── hooks/               # 自定义 Hooks
│   ├── types/               # TypeScript 类型定义
│   ├── api.ts               # 前端 Tauri API 封装
│   ├── App.tsx              # 主应用组件（状态管理、账号同步、定时任务调度）
│   └── App.css              # 全局样式
├── src-tauri/               # Tauri 后端（Rust）
│   ├── src/
│   │   ├── account/         # 账号管理核心
│   │   │   ├── account_manager.rs  # AccountManager：切换、续签、签到、存储、让位逻辑
│   │   │   ├── doubao.rs           # DoubaoManager：导入客户端登录态、签到 fallback、user_id 防串号
│   │   │   ├── types.rs            # Account / AccountBrief / CheckinDeviceProfile / Doubao 类型
│   │   │   └── mod.rs
│   │   ├── api/             # API 客户端
│   │   │   ├── trae_api.rs         # TRAE API：entitlement / GetUserToken / ExchangeToken / credit / checkin / usage
│   │   │   ├── doubao_api.rs       # 豆包 API：账号信息 / 签到 / 额度查询（D10 checkin）
│   │   │   ├── doubao_types.rs     # 豆包 API 响应类型
│   │   │   ├── types.rs            # TRAE API 响应类型
│   │   │   └── mod.rs
│   │   ├── doubao_client.rs # macOS 豆包客户端 Chromium Cookie 解密链路
│   │   ├── crypto.rs        # 加密解密（AES-128-CBC + SHA-512，兼容 TRAE storage.json）
│   │   ├── trae_app.rs      # 应用变体管理（CN / WORK / 国际版，端点、bundle path、key 格式）
│   │   ├── machine.rs       # 机器码 + 进程检测（ps / osascript / registry）
│   │   ├── login.rs         # 内嵌 WebView 登录（无痕 cookies + WKHTTPCookieStore 读取）
│   │   ├── oauth.rs         # OAuth Native 协议栈（EC P-256 + PKCE + DeviceProof + ExchangeToken）
│   │   ├── oauth_login.rs   # OAuth 完整流程（warp callback + 持久 cookies webview + 阶段事件）
│   │   ├── lib.rs           # Tauri 命令注册 + 托盘菜单 + 后台定时任务 + 日志 watchdog
│   │   └── main.rs          # 入口
│   ├── Cargo.toml           # Rust 依赖
│   └── tauri.conf.json     # Tauri 配置（updater 公钥、插件）
├── public/                  # Vite 静态文件（Tauri 打包时原样拷贝）
│   └── oauth-loading.html   # OAuth WebView 本地中转页（带 spinner，覆盖远程连接白屏）
├── .github/workflows/build.yml  # CI：signer 签名 + 跨平台构建 + GitHub Release
├── scripts/                 # 版本同步脚本（version:sync）
└── package.json             # Node.js 依赖
```

## 📋 更新日志

### v1.0.4（2026-11-23）— 豆包账号体系 + 设置页重构 + OAuth 详情刷新

[完整 Release Notes →](notes/RELEASE_NOTES_v1.0.4.md)

**✨ 新功能**

- **豆包（Doubao）账号完整支持**：独立 `platform="doubao"` 账号类型；macOS 客户端 Chromium Cookie DB 解密读取（PBKDF2-HMAC-SHA1 → AES-128-CBC → Chrome v24+ SHA256 前缀剥离）；豆包 API 客户端（账号信息 / D10 签到 / 额度查询）；签到 Cookie 过期自动回退客户端登录态（强制校验 user\_id 防串号）
- **账号管理平台隔离**：TRAE / 豆包 Tab 切换独立管理；批量操作（全选/刷新/签到）仅作用于当前平台；选中优先策略
- **设置页重构**：Section 按归属标注（TRAE 账号专属 / 所有账号）；通用置顶，TRAE 客户端 + 续签紧随
- **自动更新重构**：侧边栏版本号入口 + 更新弹窗（版本差异 + Release Notes + 下载进度）
- **TRAE 账号详情刷新**：OAuth 三路策略自动选择最优路径

**🔧 关键问题修复（含 Code Review 10 项）**

- `handleBatchRefresh` deps 缺 platformFilter / first\_uid 回退串号 / prod println! cfg guard / multi\_sids percent-encode 分隔符解析链 / Cookie header 控制字符过滤 / Doubao Safe Storage 缓存密钥

**技术细节**

- Profile 选择：Local State `last_used` 优先，fallback 按 `active_time` 降序
- multi\_sids 解析：**先 decode 整个值再 split**（所有分隔符都 percent-encoded）
- 6 新增文件 / 34 修改文件 / +4622 / -551 行

### v1.0.0（2026-10-15）— OAuth Native 授权 + UX 全面打磨 🎊

fork 原项目以来的首个稳定大版本，**本次追加 OAuth Native 完整链路** + 多项 UX 改造。

**🔧 关键问题修复**

- 🔧 **OAuth RefreshToken 401 无限重试**：ExchangeToken 签发新 RefreshToken 时旧的立即失效（OAuth 标准一次性行为）；账号重复 OAuth 登录自动更新凭证（不报错）
- 🔧 **OAuth callback 无标准 code**：auth code 在 `authCodeInfo` JSON 参数的 `AuthCode` 字段，需 percent-decode 后 JSON parse
- 🔧 **ExchangeToken 串行等待 webview 裸露**：SUCCESS\_HTML（白底 spinner）持续覆盖到 ExchangeToken **成功之后**才关 webview，用户看不到 modal 闪烁
- 🔧 **所有 Err 分支统一关 webview**：oauth\_login.rs 里 6 处 return Err 全部先 close\_login\_webview\_safe + drop manager

**✨ OAuth Native 完整实现（新增）**

- ✨ **OAuth Native 授权登录**：EC P-256 + PKCE + DeviceProof 签名 → Trae 官方授权页 → ExchangeToken 签发 14 天 access\_token + 180 天 refresh\_token
- ✨ **OAuth 全链路持久化**：每个账号独立保存密钥对、refresh\_token、device\_id、machine\_id、host（callback 带，优先于 variant.api\_host）
- ✨ **后台 OAuth RefreshToken 续签**：refresh\_all\_tokens 自动识别 oauth\_enabled 账号，ExchangeToken(Refresh) 无缝续签
- ✨ **账号已存在 → 自动更新 OAuth 凭证**：UpdateToken 弹窗和二次授权自动走更新路径，不报错
- ✨ **oauth-loading.html 本地中转页**：Warp 先加载带 spinner 的本地页，initialization\_script 注入目标 URL，location.replace 跳转覆盖 10-22s TTFB 白屏
- ✨ **OAuth WebView "切换账号"**：清 cookies → incognito 重建 → 换号重新授权

**✨ UX 打磨**

- ✨ **全屏 loading 遮罩**：批量签到期间深色半透明 + 毛玻璃遮罩，禁止其他交互（可扩展到其他长时间任务）
- ✨ **OAuth 流程阶段化状态**：opening spinner（打开中）→ waiting（等待授权）→ exchanging spinner（获取凭证中）
- ✨ **Tab 顺序统一**：AddAccountModal / UpdateTokenModal 四个 tab 顺序一致（OAuth 授权 → 从 Trae 客户端读取 → 浏览器登录（旧）→ 手动输入 Token）
- ✨ **UpdateTokenModal 新增 OAuth tab**：完整复用 AddAccountModal 的 OAuth 状态机
- ✨ **OAuth 登录态标签**：OAuth 自管·WebView / OAuth 注入·xx / 短效凭证·手动Token 等
- ✨ **设置页目标客户端切换遮罩**：切换过程中显示 `global-busy-overlay` 全屏遮罩，防止中途打断

**🔧 Windows 平台适配**

- 🔧 **Windows 进程检测**：`machine.rs` 适配 Windows 平台多版本进程枚举（tasklist）、变体安装路径扫描（注册表 + 目录 fallback）
- 🔧 **日志 watchdog Windows 补全**：Windows 平台补全 `start_log_watchdog` 实现（空函数补全），无 inode 用 `fs::metadata().is_ok()` 检查，重建后 `SetStdHandle` 重定向 stdout/stderr

**核心 v1.0.0 功能（初版）**

一键切换客户端账号 / 内嵌 WebView 无痕登录 / Token 自动续签 + 多客户端让位防互踢 / 每日自动签到领积分 / Dashboard 仪表盘 / 在线自动更新 / 完整日志系统 / 登录来源打标 / 多应用变体 / 侧边栏可收缩 / 数据导入导出

[完整 Release Notes →](notes/RELEASE_NOTES_v1.0.0.md)

### v0.9.10 及更早版本

详见 [Releases 页面](https://github.com/marscey/trae-jumper/releases)。

## 📄 致谢

本项目 fork 自 [Yang-505/Trae-Account-Manager](https://github.com/Yang-505/Trae-Account-Manager)，并基于 [Tauri](https://tauri.app/) 与 [React](https://react.dev/) 构建。
