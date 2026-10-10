# TraeJumper v0.9.10 Release Notes

发布日期：2026-08-24

## 🎉 版本概述

本版本引入 **在线自动更新**、**侧边栏可收缩** 与 **每日签到领取积分** 三大能力，全面优化账号管理交互与 UI，并完善 CI 发布流程（签名构建 + latest.json 自动生成）。

## ✨ 新功能

### 1. 在线自动更新
- 集成 tauri-plugin-updater / tauri-plugin-process
- 「关于」页新增「检查更新」：可检查、下载并安装新版本
- 更新包经 Ed25519 签名校验，安全可信
- Windows 静默安装（passive 模式），macOS 拖拽 DMG 安装

### 2. 每日签到领取积分
- 账号右键菜单「每日签到」、工具栏「批量签到」一键领取积分
- 每次签到 +200 积分，成功后自动刷新积分汇总
- 签到虚拟设备档案（x-device-id）自动生成与自愈，规避服务端 9074 拒绝
- 签到配置：设备 ID 生成策略（真实设备前缀 / FNV 安全区间）、批量签到延迟范围（模拟人工操作节奏）
- 支持一键重置所有账号的签到虚拟设备档案

### 3. 侧边栏可收缩
- 支持手动收起 / 展开，窗口较窄时自动收起
- 收起态仅显示图标，展开态图标 + 文字
- 切换应用 / 账号按钮移至底部 footer，侧边栏宽度优化

## 🎨 UI/UX 优化
- 账号卡片、详情弹窗、列表项视觉与交互优化
- 添加账号模态支持浏览器登录域名动态同步
- 右键菜单、确认弹窗、信息弹窗交互优化
- Settings 页扩展：新增签到配置、签到设备档案管理、客户端路径等区块

## 🔧 技术改进
- CI 签名构建：上传 `.app.tar.gz` + `.sig` 作为 updater artifacts，规范产物命名
- 重写 `merge-updater-json.py`：从 `.sig` 提取签名、结合规范 Release URL 生成 `latest.json`
- 合并多平台 `latest.json` 为单一跨平台清单
- `.gitignore` 新增密钥防泄露规则（`*.key` / `*.priv`）

## 🐛 Bug 修复
- 修复 CI 中 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` 空密码导致的 `Wrong password for that key` 签名失败
- 修复 Tauri v2 不自动生成 `latest.json`、导致自动更新端点无法获取清单的问题

## 📦 安装包

| 平台 | 文件 |
|------|------|
| macOS (Apple Silicon) | `TraeJumper-0.9.10-mac-arm64.dmg` |
| macOS (Intel) | `TraeJumper-0.9.10-mac-x64.dmg` |
| Windows x64 (NSIS) | `TraeJumper-0.9.10-win-x64-setup.exe` |

## 📋 升级建议

- v0.9.9 及更早版本可直接覆盖安装
- 安装后可在「关于」页使用「检查更新」，后续版本将支持应用内一键升级
