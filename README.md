<div align="center">

<img src="src/assets/logo.png" alt="TraeJumper" style="width: 120px; height: 120px; margin-bottom: 16px;">

# TraeJumper

TraeCode CN / TraeWork CN / 国际版 Trae 多账号管理小工具

[![Version](https://img.shields.io/badge/version-1.0.0-blue?style=flat-square)](https://github.com/marscey/trae-jumper/releases)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS-lightgrey?style=flat-square)](#-系统要求)
[![Build](https://img.shields.io/github/actions/workflow/status/marscey/trae-jumper/build.yml?style=flat-square)](https://github.com/marscey/trae-jumper/actions)
[![License](https://img.shields.io/badge/license-MIT-orange?style=flat-square)](#-免责声明)

[功能特性](#-功能特性) • [系统要求](#-系统要求) • [安装](#-安装) • [使用指南](#-使用指南) • [常见问题](#-常见问题) • [技术栈](#-技术栈) • [更新日志](#-更新日志)

</div>

TraeJumper 是一款面向 Trae 系列 IDE 用户的多账号管理桌面工具。基于 [Tauri 2](https://tauri.app/) 构建，支持在多个 Trae 账号之间一键切换，实时查看各账号的 Token 使用量与积分配额，内置完整的签到、日志、自动刷新与定时续签能力。所有数据仅保存在本地。

## ✨ 功能特性

**多应用支持**

- 同时支持 TraeCode CN（国内版）、TraeWork CN、Trae（国际版）三种应用变体
- 在设置页随时切换目标应用，机器码、安装路径、登录站点、API 端点自动跟随变更
- **多客户端活跃检测**：扫描所有已安装变体，账号在哪个客户端登录一目了然

**账号管理**

- **内嵌 WebView 登录**：登录窗口内嵌浏览器（无痕隔离会话），完整捕获 JWT Token + HttpOnly Cookies（用于后续自动续签）
- 通过 Token 或 WebView 登录两种方式添加账号，自动获取账号信息并绑定机器码
- 一键切换账号：自动关闭 Trae → 清除旧登录态 → 写入新账号 → 重新打开
- **跨客户端冲突检测**：切换前检查目标账号是否已在其他客户端活跃，防止互踢顶号（可 force 强制切换）
- 支持更新 Token、删除账号、查看详情、复制账号信息
- **点击用户名**可直接打开账号详情弹窗（无需右键）
- **批量操作**：选中多个账号后可批量刷新、批量删除、切换账号（单选）
- **登录来源打标**：账号卡片显示来源标签（客户端导入 / WebView 登录 / Cookie 登录 / 手动 Token / 切号注入 / 原生 OAuth）

**Token 生命周期管理**

- **后端定时续签**：后台 tokio 任务每分钟检查，Token 快过期时自动 GetUserToken 续签 + 写入客户端 storage.json
- **手动续签写回**：主动触发续签并写给客户端（无需等待 Token 过期，用于测试和应急）
- **多客户端让位逻辑**：新版自管理客户端（OAuth refresh token 架构）活跃账号完全让位，TraeJumper 不调用 GetUserToken，防止误作废旧 Token 导致客户端登出
- **切换账号 Token 有效性守卫**：写入前先用 entitlement 接口校验 Token 存活，死 Token 时先尝试 cookies 续签；绝不把失效 Token 写入客户端
- **Token 过期状态细化**：客户端登录中（让位）/ 正常（>3h）/ 即将过期（<3h）+ 相对时长 / 已过期（红色加粗）

**签到系统**

- **虚拟设备档案**：每个账号独立分配一组虚拟设备指纹（session-id、market-user-id、device-id、device-brand），模拟"一台独立设备"每日签到
- **设备 ID 双策略**：策略一 = 真实设备前缀 + 随机后缀；策略二 = FNV-1a 哈希映射到安全区间 [1e15, 4.5e15)
- **签到风控自愈**：自动检测旧版非法 device-id（≥4.5e15）并重生成
- **单账号签到 / 批量签到 / 签到状态查询**，带账号专属随机延迟防风控
- **自动签到**：设置触发时间点（默认 22:00），防跨零点策略 + 内存去重，同一天只执行一次
- **签到请求头预览**：查看每个账号实际发送的全部 headers 配置

**使用量监控**

- 实时展示每个账号的今日/总使用量与剩余额度
- 查看详细使用事件，支持快捷筛选（今天/7天/30天）+ 自定义时间范围，展示 Token 数量与模型信息
- **积分体系支持**：自动识别 Trae CN / TRAE WORK 的积分计费模式，展示通用积分、Work 专属积分、奖励积分（每月登录赠送/老用户福利等）的总额、已用、剩余与到期时间
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
- **运行中 watchdog**：后台线程每 N 秒检查日志文件 (dev, ino)，发现被外部删除/替换则自动重建目录 + 文件并重新 dup2 重定向
- 日志仅保留关键信息：添加账号时的 cookie 名称列表、续签候选及触发原因、活跃账号判定、安全保护触发

**机器码管理**

- 查看、复制、刷新、重置 Trae 机器码
- 每个账号独立绑定机器码，切换账号时自动更新

**数据管理**

- 将全部账号数据导出为 JSON（完整包含 cookies、jwt_token、user_id、tenant_id 等所有核心凭据），或从 JSON 导入（可完整恢复）
- 一键清空所有数据（危险操作，带二次确认弹窗）

**系统集成**

- 系统托盘：关闭窗口后隐藏到托盘，左键恢复、右键菜单退出
- 单实例运行，避免多开冲突
- 登录窗口独立，不受托盘行为影响
- 支持在线自动更新（关于页点击「检查更新」即可下载安装新版本，签名校验）

**安全存储**

- 数据仅保存在本地，不上传云端
- 兼容并读取 Trae 的 AES-128-CBC + SHA-512 加密存储，同时兼容旧版明文格式
- 新版客户端（storage.json 含 `icube-dc:*` 设备密钥键）登录态自动适配读取

## 💻 系统要求

| 平台 | 版本 |
|------|------|
| Windows | 10 / 11 |
| macOS | 10.15+ |

> [!NOTE]
> 需已安装任意一种 Trae 客户端（TraeCode CN / TraeWork CN / 国际版）。

## 📦 安装

### 下载安装包

前往 [Releases](https://github.com/marscey/trae-jumper/releases) 页面下载对应平台的安装包：

- **macOS**：`TraeJumper-1.0.0-mac-arm64.dmg` / `TraeJumper-1.0.0-mac-x64.dmg`
- **Windows**：`TraeJumper-1.0.0-win-x64-setup.exe`（另有 `.msi` 安装包）

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

> [!TIP]
> 构建产物位于 `src-tauri/target/release/bundle/`。CI 工作流见 `.github/workflows/build.yml`，自动签名 + 创建 GitHub Release。

## 📖 使用指南

### 1. 选择目标应用

首次使用请进入 **设置** 页，在 **目标应用** 区块选择要管理的 Trae 客户端，系统会自动检测已安装的应用。

### 2. 配置 Trae 路径

在设置页 **客户端路径** 部分，点击 **自动扫描** 自动查找应用，或点击 **手动设置** 选择应用文件。

### 3. 添加账号

**方式 A — WebView 登录（推荐）**

1. 点击右上角 **添加账号**
2. 在弹出的登录窗口中，使用内嵌浏览器完成 Trae 登录
3. 登录完成后，点击底部 **确认登录** 按钮
4. 系统自动读取 Token + Cookies + 用户信息，保存并绑定机器码

> 此方式通过无痕隔离会话实现，每次登录互不顶号；捕获的 Cookies 包含 HttpOnly sessionid，用于后续 Token 自动续签。

**方式 B — 手动 Token**

1. 点击右上角 **添加账号**
2. 切换到手动模式，输入 Trae Token
3. 点击 **添加**，系统自动获取账号信息并保存

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

> [!WARNING]
> 切换账号前请保存 Trae 中的工作内容。

### 5. 更新账号 Token

**方式 A — WebView 登录更新**

1. 在账号卡片右键菜单选择 **更新 Token**（或 UpdateTokenModal）
2. 内嵌 WebView 自动登录 → 确认登录 → 校验同一用户 → 更新 Token + Cookies

**方式 B — 从客户端读取更新**

1. 确保 Trae 客户端已登录目标账号
2. 点击 **从客户端读取更新**，自动解密读取 storage.json 中的登录态

**方式 C — 手动输入新 Token**

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

在设置页 **数据管理** 区域，点击 **导出** 将全部账号数据保存为 JSON（完整包含 cookies、jwt_token、user_id、tenant_id 等核心凭据）；点击 **导入** 从 JSON 恢复账号数据；点击 **清空** 删除全部数据（需二次确认）。

## ⚠️ 免责声明

> [!WARNING]
> 本工具仅供学习和技术研究使用。使用过程中可能涉及绕过软件账号切换限制，使用者需自行评估并承担全部风险；请勿用于商业用途，不得用于绕过软件正当授权机制。

## 🛠️ 技术栈

- **前端**：React 19 / TypeScript / Vite 7 / Recharts 3 / lucide-react
- **后端**：Tauri 2 / Rust / Tokio / Reqwest / Serde
- **加密**：AES-128-CBC + SHA-512
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
│   │   │   ├── account_manager.rs  # AccountManager（2952 行）：切换、续签、签到、存储、让位逻辑
│   │   │   ├── types.rs            # 类型定义：Account / AccountBrief / CheckinDeviceProfile / CheckinConfig / AccountLoginSource / AccountLoginType / DeviceIdStrategy
│   │   │   └── mod.rs
│   │   ├── api/             # Trae API 客户端
│   │   │   ├── trae_api.rs         # API 客户端：entitlement / GetUserToken / credit / checkin / usage（多端点容灾）
│   │   │   ├── types.rs            # API 响应类型
│   │   │   └── mod.rs
│   │   ├── crypto.rs        # 加密解密（AES-128-CBC + SHA-512，兼容 Trae storage.json）
│   │   ├── trae_app.rs      # 应用变体管理（CN / WORK / 国际版，端点、bundle path、key 格式）
│   │   ├── machine.rs       # 机器码 + 进程检测（ps / osascript / registry）
│   │   ├── login.rs         # 内嵌 WebView 登录（无痕隔离会话 + WKHTTPCookieStore 读取 HttpOnly）
│   │   ├── lib.rs           # Tauri 命令注册 + 托盘菜单 + 后台定时任务（自动刷新、定时续签、日志 watchdog）
│   │   └── main.rs          # 入口
│   ├── Cargo.toml           # Rust 依赖
│   └── tauri.conf.json     # Tauri 配置（updater 公钥、插件）
├── .github/workflows/build.yml  # CI：signer 签名 + 跨平台构建 + GitHub Release
├── scripts/                 # 版本同步脚本（version:sync）
└── package.json             # Node.js 依赖
```

## 📋 更新日志

### v1.0.0（2026-10-10）— 正式发布 1.0.0 🎊

fork 原项目以来的首个稳定大版本，将 v0.9.10（在线自动更新、签到系统、侧边栏可收缩）到 v0.9.11（客户端登录态安全重构、内嵌 WebView 登录、后端定时任务化）的完整功能集合并发。

**关键问题修复**
- 🔧 **客户端"登录已失效"根因**：修复单活跃 Token 策略下 TraeJumper 与客户端互相顶号、新版自管理客户端被误签写作废的问题
- 🔧 **多客户端活跃检测**：遍历所有已安装变体，不再只检查当前目标客户端
- 🔧 **切换账号 Token 有效性守卫**：写入前校验 Token 存活，死 Token 先尝试 cookies 续签；绝不把失效 Token 写入客户端
- 🔧 **新版客户端登录态读取**：适配 `iCubeAuthInfo://icube-dc:<device_id>` 新 key 格式
- 🔧 **进程检测修复**：macOS `ps -axo pid=,command=` 替代 `pgrep -f`，解决 TraeWork 主进程命令行过长漏判

**新功能**
- ✨ **一键切换客户端账号**（跨客户端冲突检测 + Token 有效性守卫）
- ✨ **内嵌 WebView 无痕登录**（替代 warp HTTP 回调，捕获 HttpOnly cookies）
- ✨ **Token 自动续签 + 多客户端让位防互踢**（后端 tokio interval 驱动）
- ✨ **每日自动签到领积分** + 虚拟设备档案自愈
- ✨ **Dashboard 仪表盘**（积分/配额双模式 + 饼图）
- ✨ **在线自动更新**（Ed25519 签名校验）
- ✨ **完整日志系统**（落盘 + 5MB 轮转 + Watchdog）
- ✨ **登录来源打标** + **多应用变体** + **侧边栏可收缩** + **数据导入导出**

[完整 Release Notes →](notes/RELEASE_NOTES_v1.0.0.md)

### v0.9.10 及更早版本

详见 [Releases 页面](https://github.com/marscey/trae-jumper/releases)。

## 📄 致谢

本项目 fork 自 [Yang-505/Trae-Account-Manager](https://github.com/Yang-505/Trae-Account-Manager)，并基于 [Tauri](https://tauri.app/) 与 [React](https://react.dev/) 构建。
