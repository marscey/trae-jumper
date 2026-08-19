<div align="center">

![TraeJumper](src/assets/logo.png)

# TraeJumper

Trae CN / TRAE WORK / 国际版 Trae 多账号管理小工具

[![Version](https://img.shields.io/badge/version-0.9.8-blue?style=flat-square)](https://github.com/marscey/trae-jumper/releases)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS-lightgrey?style=flat-square)](#-系统要求)
[![Build](https://img.shields.io/github/actions/workflow/status/marscey/trae-jumper/build.yml?style=flat-square)](https://github.com/marscey/trae-jumper/actions)
[![License](https://img.shields.io/badge/license-MIT-orange?style=flat-square)](#-免责声明)

[功能特性](#-功能特性) • [系统要求](#-系统要求) • [安装](#-安装) • [使用指南](#-使用指南) • [常见问题](#-常见问题) • [技术栈](#-技术栈)

</div>

TraeJumper 是一款面向 Trae 系列 IDE 用户的多账号管理桌面工具。基于 [Tauri 2](https://tauri.app/) 构建，支持在多个 Trae 账号之间一键切换，实时查看各账号的 Token 使用量，并内置机器码管理、数据导入导出等功能。所有数据仅保存在本地。

## ✨ 功能特性

**多应用支持**

- 同时支持 Trae CN（国内版）、TRAE WORK、Trae（国际版）三种应用变体
- 在设置页随时切换目标应用，机器码、安装路径、登录站点、API 端点自动跟随变更

**账号管理**

- 通过 Token 添加账号，自动获取账号信息并绑定机器码
- 一键切换账号：自动关闭 Trae → 清除旧登录态 → 写入新账号 → 重新打开
- 支持更新 Token、删除账号、查看详情、复制账号信息

**使用量监控**

- 实时展示每个账号的今日/总使用量与剩余额度
- 查看详细使用事件，按时间范围筛选，展示 Token 数量与模型信息

**机器码管理**

- 查看、复制、刷新、重置 Trae 机器码
- 每个账号独立绑定机器码，切换账号时自动更新

**数据管理**

- 将全部账号数据导出为 JSON，或从 JSON 导入
- 一键清空所有数据（危险操作，带二次确认弹窗）

**系统集成**

- 系统托盘：关闭窗口后隐藏到托盘，左键恢复、右键菜单退出
- 单实例运行，避免多开冲突
- 登录窗口独立，不受托盘行为影响

**安全存储**

- 数据仅保存在本地，不上传云端
- 兼容并读取 Trae 的 AES-128-CBC + SHA-512 加密存储，同时兼容旧版明文格式

## 💻 系统要求

| 平台 | 版本 |
|------|------|
| Windows | 10 / 11 |
| macOS | 10.15+ |

> [!NOTE]
> 需已安装任意一种 Trae 客户端（Trae CN / TRAE WORK / 国际版）。

## 📦 安装

### 下载安装包

前往 [Releases](https://github.com/marscey/trae-jumper/releases) 页面下载对应平台的安装包：

- **macOS**：`TraeJumper-<version>-mac-arm64.dmg` / `TraeJumper-<version>-mac-x64.dmg`
- **Windows**：`TraeJumper-<version>-win-x64-setup.exe`（另有 `.msi` 安装包）

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

> [!TIP]
> 构建产物位于 `src-tauri/target/release/bundle/`。修改应用版本号时，编辑 `src-tauri/tauri.conf.json` 中的 `version` 字段。

## 📖 使用指南

### 1. 选择目标应用

首次使用请进入 **设置** 页，在 **目标应用** 区块选择要管理的 Trae 客户端，系统会自动检测已安装的应用。

### 2. 配置 Trae 路径

在设置页 **客户端路径** 部分，点击 **自动扫描** 自动查找应用，或点击 **手动设置** 选择应用文件。

### 3. 添加账号

1. 点击右上角 **添加账号**
2. 输入 Trae Token
3. 点击 **添加**，系统自动获取账号信息并保存

**获取 Token 的方法：**

1. 打开 Trae，按 `F12` 打开开发者工具
2. 切换到 `Application` 标签
3. 在 `Local Storage` → `vscode-webview://xxx` 中找到包含 `iCubeAuthInfo` 的键
4. 复制其中的 `token` 值

### 4. 切换账号

点击账号卡片上的 **切换** 按钮并确认。系统会自动：关闭当前 Trae → 清除旧登录态 → 写入新账号 → 重新打开 Trae。

> [!WARNING]
> 切换账号前请保存 Trae 中的工作内容。

### 5. 查看使用量

- **仪表板**：查看所有账号的使用量概览（今日/总量/进度条）
- **详情页**：切换至 **使用记录** 标签，查看每次使用的时间、Token 数量、模型与请求类型

### 6. 管理机器码

进入 **设置** 页，在 **机器码** 区域可复制、刷新或重置机器码；**清除登录状态** 会重置机器码并删除本地缓存数据。

### 7. 数据导入导出

在设置页 **数据管理** 区域，点击 **导出** 将全部账号数据保存为 JSON；点击 **导入** 从 JSON 恢复账号数据；点击 **清空** 删除全部数据（需二次确认）。

## ⚠️ 免责声明

> [!WARNING]
> 本工具仅供学习和技术研究使用。使用过程中可能涉及绕过软件账号切换限制，使用者需自行评估并承担全部风险；请勿用于商业用途，不得用于绕过软件正当授权机制。

## 🛠️ 技术栈

- **前端**：React 18 / TypeScript / Vite / CSS3
- **后端**：Tauri 2 / Rust / Tokio / Reqwest / Serde
- **加密**：AES-128-CBC + SHA-512
- **平台**：Windows（NSIS/MSI）、macOS（DMG）

## 📁 项目结构

```
trae-jumper/
├── src/                     # 前端源码
│   ├── components/          # React 组件（账号卡片、弹窗、右键菜单等）
│   ├── pages/               # 页面（仪表板、设置、关于）
│   ├── hooks/               # 自定义 Hooks
│   ├── api.ts               # 前端 API 封装
│   └── App.tsx              # 主应用组件
├── src-tauri/               # Tauri 后端
│   ├── src/
│   │   ├── account/         # 账号管理（存储、切换、加密解密）
│   │   ├── api/             # Trae API 客户端（多端点容灾）
│   │   ├── crypto.rs        # 加密解密模块
│   │   ├── trae_app.rs      # 应用变体管理（CN / WORK / 国际版）
│   │   ├── machine.rs       # 机器码管理（跨平台）
│   │   ├── login.rs         # 浏览器登录
│   │   └── lib.rs           # Tauri 命令注册
│   ├── Cargo.toml           # Rust 依赖
│   └── tauri.conf.json      # Tauri 配置
├── .github/workflows/       # CI 构建工作流
└── package.json             # Node.js 依赖
```

## 📄 致谢

本项目 fork 自 [Yang-505/Trae-Account-Manager](https://github.com/Yang-505/Trae-Account-Manager)，并基于 [Tauri](https://tauri.app/) 与 [React](https://react.dev/) 构建。
