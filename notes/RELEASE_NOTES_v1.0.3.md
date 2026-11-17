# TraeJumper v1.0.3 Release Notes

发布日期：2026-11-17

## 🎉 版本概述

**v1.0.3 — OAuth 设备身份修复 + 大规模架构重构**

本次迭代修复了一个影响新版 TRAE CN 客户端的关键问题：OAuth 授权签发的凭据绑定的是 TraeJumper 自建设备密钥，与客户端自身设备身份不一致，服务端返回 20403（Token device not match），导致客户端启动后清空登录态、"切换后未登录"。v1.0.3 改为复用客户端 storage.json 中的设备密钥对与 machineId，使 TraeJumper 与客户端共享同一设备身份。

同时完成了三项大规模重构：后端 `account_manager.rs`、前端 `App.tsx`、`Settings.tsx` 三个巨型文件按职责拆分，行为保持不变，为后续迭代降低维护成本。

**变更规模**：29 个源码文件，+6839 / -6025 行（以代码搬移为主，净逻辑变更集中在 OAuth 设备身份修复）。

## 🐛 关键修复

### 1. OAuth 授权复用 Trae 客户端设备身份（修复 20403 登录态被清空）

**问题现象**：OAuth 账号切换注入后，新版 TRAE CN 客户端启动时清空登录态，表现为"切号后未登录"。

**根因**：新版客户端内核（3.4.20+）启动时会用自身 `icube-dc` 设备密钥对 refreshToken 做 ExchangeToken 自刷。TraeJumper 此前用自建 EC 密钥对签发凭据，设备身份不匹配，服务端返回 401 "Token device not match"（Code 20403），客户端随即 `clearUserInfo` 登出。

**修复方案**：

- 新增 `read_client_device_identity`：解密读取客户端 `storage.json` 中的 `iCubeAuthInfo://icube-dc:<device_id>` 设备密钥对（PKCS8 PEM）与 `telemetry.machineId`
- OAuth 授权流程优先复用该设备身份（device_id、machine_id、EC 密钥对），注入的凭据可被客户端正常续签
- 客户端 storage 不可读或版本过旧时，自动回退到原有自建密钥对逻辑，不影响旧环境

> 注意：修复前已存在的 OAuth 账号，其 refresh_token 仍绑定旧的自建设备身份，建议在账号操作菜单中重新走一次 OAuth 授权（更新凭证）以刷新设备绑定。

## 🧱 架构重构（行为不变）

### 1. 后端 `account_manager.rs` 拆分（3648 行 → 6 个内聚模块）

- `account_manager.rs`（约 375 行）：结构体定义 + 账号 CRUD 与存储读写
- `switch.rs`：账号切换、客户端登录态读取与同步
- `token.rs`：Token / refresh_token 生命周期与批量续签
- `usage.rs`：用量积分查询、401 归属仲裁
- `checkin.rs`：签到、虚拟设备指纹、自动签到
- `import_export.rs`：账号导入导出
- `tests.rs`：原有导入测试

方法原样搬移，跨模块私有方法提升为 `pub(super)`，警告集与原版一致，单元测试全部通过。

### 2. 前端 `App.tsx` 拆分（1897 行 → 约 500 行组合根）

- `hooks/useAccountsData`：账号数据中枢（列表、积分懒加载、局部刷新、启动初始化、token-refreshed 监听、自动刷新定时器）
- `hooks/useAutoUpdate`：自动更新检查与静默安装
- `hooks/useAppModals`：全局弹窗状态集中管理
- `features/accounts/useTokenActions`、`useCheckinActions`、`useAccountActions`、`useImportExport`：Token 续签、签到、账号 CRUD/切换、导入导出
- `pages/AccountsPage`：账号管理页 JSX 与本地状态

逻辑原样搬移，handlers 改为 `useCallback` 并补全依赖清单，交互行为与原版一致。

### 3. 前端 `Settings.tsx` 拆分（1140 行 → 约 70 行组装壳）

- `settings/ClientSection`：客户端选择、路径、机器码、设备 ID、账号切换
- `settings/GeneralSection`：自动刷新、日志重建
- `settings/RenewSection`：续签配置、重启客户端
- `settings/CheckinSection`：设备策略、延迟、自动签到、型号池
- `settings/DataSection`：导出 / 导入 / 清空
- `settings/useSettingsConfig`：共享配置状态与乐观保存（6 个 handler 归一为工厂方法，保留字段级回滚语义）

## 📦 安装包

| 平台 | 文件 |
|------|------|
| macOS (Apple Silicon) | `TraeJumper-1.0.3-mac-arm64.dmg` |
| macOS (Intel) | `TraeJumper-1.0.3-mac-x64.dmg` |
| Windows x64 (NSIS) | `TraeJumper-1.0.3-win-x64-setup.exe` |

## 📋 升级建议

- **1.0.2 用户**：应用启动 10 秒后会自动检查并静默下载安装 1.0.3，完成后按横幅提示重启即可
- **OAuth 账号用户**：升级后如仍遇到客户端登录态被清空，请对该账号执行一次重新 OAuth 授权，使 refresh_token 绑定客户端设备身份
- **全新安装**：下载对应平台安装包覆盖安装即可，数据格式向后兼容
- 本次重构不改变存储格式与业务逻辑，升级无需迁移数据

## 🛠️ 技术栈

React 19 / Vite 7 / Tauri 2 / Rust / Serde / Tokio / tauri-plugin-updater

## 🙏 致谢

感谢反馈"OAuth 账号切换后 TRAE CN 未登录"的用户，本次设备身份复用修复直接定位并解决了该问题的根因。
