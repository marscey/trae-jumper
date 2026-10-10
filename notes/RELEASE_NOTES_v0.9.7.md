# TraeJumper v0.9.7 Release Notes

发布日期：2026-08-17

## 🎉 版本概述

TraeJumper 是一款专为 Trae 系列 IDE 用户打造的多账号管理桌面工具。本版本基于 fork 原项目（Yang-505/Trae-Account-Manager）后进行深度重构，新增多应用变体支持、跨平台适配、安全存储加密等核心功能，是 fork 后的首个正式版本（致敬拳皇 97，打磨完成后将发布 1.0.0）。

## ✨ 新功能

### 1. 多应用变体支持
- 🎯 **同时支持 Trae CN（国内版）、TRAE WORK、Trae（国际版）**
- 在设置页随时切换目标应用，机器码、安装路径、登录站点、API 端点自动跟随变更

### 2. 系统集成
- 💻 **系统托盘**：关闭窗口后隐藏到托盘，左键恢复、右键菜单退出
- 🔒 **单实例运行**：避免多开冲突，登录窗口独立不受影响

### 3. 账号管理
- ➕ **清空数据功能**：一键删除所有账号数据（带危险操作确认弹窗）
- 🔄 **账号切换优化**：自动关闭 Trae → 清除旧登录态 → 写入新账号 → 重新打开

### 4. 数据导入导出
- 📦 **导出**：将全部账号数据保存为 JSON 文件
- 📥 **导入**：从 JSON 恢复账号数据，保留原账号名称

## 🔧 技术改进

### 跨平台适配
- 🍎 **macOS 平台支持**：进程检测、数据目录、安装路径扫描全链路适配
- 🪟 **Windows 平台支持**：完整的 NSIS/MSI 安装包支持

### 安全存储
- 🔐 **AES-128-CBC + SHA-512 加密**：与 Trae 客户端保持一致的存储加密方案
- 兼容旧版明文格式，自动检测并迁移

### 工程化
- 📋 **CI/CD 工作流**：支持 macOS arm64/x64、Windows x64 三平台自动构建
- 🏷️ **版本管理**：三端（package.json / tauri.conf.json / Cargo.toml）统一

## 📦 安装包

| 平台 | 文件 |
|------|------|
| macOS (Apple Silicon) | `TraeJumper-0.9.7-mac-arm64.dmg` |
| macOS (Intel) | `TraeJumper-0.9.7-mac-x64.dmg` |
| Windows x64 (NSIS) | `TraeJumper-0.9.7-win-x64-setup.exe` |
| Windows x64 (MSI) | `TraeJumper-0.9.7-win-x64.msi` |

### macOS 首次运行提示
macOS 可能会提示「应用已损坏」，可在终端执行：
```bash
sudo xattr -rd com.apple.quarantine /Applications/TraeJumper.app
```

## 📋 升级建议

- 本版本为 fork 后首次发布，从原项目升级请先备份账号数据
- 首次使用请在设置页选择目标 Trae 应用变体

## 🛠️ 技术栈

- **前端**：React 18 / TypeScript / Vite / CSS3
- **后端**：Tauri 2 / Rust / Tokio / Reqwest / Serde
- **加密**：AES-128-CBC + SHA-512

## ⚠️ 免责声明

本工具仅供学习和技术研究使用。使用过程中可能涉及绕过软件账号切换限制，使用者需自行评估并承担全部风险；请勿用于商业用途，不得用于绕过软件正当授权机制。

## 🔗 相关链接

- 源代码：[GitHub](https://github.com/marscey/trae-jumper)
- 问题反馈：[Issues](https://github.com/marscey/trae-jumper/issues)
- 上一版本：原项目 v1.0.0（[Yang-505/Trae-Account-Manager](https://github.com/Yang-505/Trae-Account-Manager)）
