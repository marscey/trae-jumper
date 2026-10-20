# TraeJumper v1.0.1 Release Notes

发布日期：2026-10-20

## 🎉 版本概述

**v1.0.1 — 续签重启粒度精细化 + UI 打磨**

本次迭代聚焦"Token 自动续签后是否重启 Trae 客户端"这一高频需求，将原来的全局开关拆分为"总开关 + 单账号覆盖"两层配置，并新增一键扫描工具修正客户端退出后遗留的活跃态标签。

**变更规模**：12 个文件，+785/-133 行（前端 UI + 后端 Tauri command + 数据模型扩展）。

## ✨ 新功能

### 1. 设置页"续签配置"独立 section

原来"Token 自动续签"和"自动续签后重启 Trae 客户端"嵌在通用设置里，与签到/数据管理等平级区块混排。v1.0.1 将它们抽出来作为 top-level section **续签配置**：

- **自动续签总开关**（`auto_refresh_enabled`，默认 true）：决定后台每分钟是否执行 `refresh_all_tokens`
- **续签后重启总开关**（`auto_restart_after_refresh`，默认 true，v1.0.1 新增）：续签成功后是否 kill + launch 客户端让新 Token 生效
- 风险提示文案：明确说明"关闭重启后 Token 只写入 storage.json 但客户端不会自动重新加载"

### 2. Per-account 续签重启三态开关

**问题**：有多个账号活跃在不同客户端时，有的账号续签后需要重启让新 Token 立即生效（比如在跑关键任务），有的账号不想打断正在编辑的 Trae 窗口。原来全局开关无法满足这种 per-account 粒度。

**v1.0.1 新增三态切换**（账号详情 Modal 内）：

| 状态 | 含义 |
|------|------|
| 跟随总开关 | `restart_after_refresh_override = None` |
| 强制开启 | `restart_after_refresh_override = Some(true)` |
| 强制关闭 | `restart_after_refresh_override = Some(false)` |

- 乐观更新：点击按钮后 UI 瞬间变蓝 + spinner 旋转，API 失败自动回滚
- 卡片/列表激活标签**末尾显示有效重启状态**：`· 重启` / `· 不重启`（仅当账号活跃于客户端时显示，节省非活跃账号的视觉噪音）

**计算逻辑**：
```
effective_restart = account.restart_after_refresh_override
                  ?? checkin_config.auto_restart_after_refresh
                  ?? true  (fallback)
```

### 3. Toolbar "检测当前账号"按钮

**问题**：手动关闭 Trae 客户端后，TraeJumper 的 `is_client_active` 标签会一直保留"当前"状态（直到下一次 `refreshAccountsData` 刷新，默认 10 分钟间隔），造成"客户端明明关了但 TJ 还显示活跃"的错觉。

**v1.0.1 新增按钮**：Toolbar 区域紧接"刷新全部"按钮旁加 **🔄 检测当前账号**，点击后：
- 重新扫描所有已安装变体的 `storage.json`
- 修正 `is_client_active` / `active_in_clients` 标签
- 显示 toast "已重新检测 N 个客户端"

### 4. 后端数据模型扩展

**Account struct 新增字段**：
```rust
pub restart_after_refresh_override: Option<bool>,  // None = 跟随 CheckinConfig.auto_restart_after_refresh
```

**CheckinConfig 新增字段**：
```rust
pub auto_restart_after_refresh: bool,  // 默认 true
```

**新 Tauri Command**：
- `set_account_restart_override(account_id, enabled: Option<bool>)` → ()：设置单账号覆盖值
- `check_current_login()` → ()：重新扫描所有客户端 storage.json（前端复用 `refreshAccountsData` + `toast` 包装）

## 🎨 UI/UX 打磨

| 区域 | 优化 |
|------|------|
| 详情弹窗三态开关 | 垂直布局（`detail-row-vertical`）避免水平 flex 挤压截断；保存中按钮组整体降透明度（0.35）+ pointer-events: none，当前选中按钮显示旋转 spinner |
| 激活标签 | 末尾追加 `· 重启` / `· 不重启`，仅活跃时显示，不占非活跃账号视觉空间 |
| Toolbar | "检测当前账号"按钮用 `batch-btn` 样式 + spinner 图标，loading 态有独立 CSS 覆盖（避免全局 `.loading` page-level 遮罩样式污染） |

## 🔧 技术改进

### 前端状态管理优化

- **AccountBrief vs 完整 Account 同步**：`onAccountChanged` 原来用 `{ ...fresh }` 展开 AccountBrief 覆盖 `detailAccount`，导致 cookies/jwt_token 等完整详情被精简投影冲掉。改为只同步 `restart_after_refresh_override` 和 `checkin_status` 两个字段，保留 prev 的完整 Token 数据
- **乐观更新 + 失败回滚**：三态开关先 `setPendingOverride(val)` 再调 API，finally 里 spinner 消失；API 异常时 catch 里 `setPendingOverride(prevVal)` 回滚

### Rust Backend

- `lib.rs` 新增 `set_account_restart_override` command（`Option<bool>` 参数，`None` 清除覆盖回到跟随状态）
- `account_manager.rs` 新增 `set_account_restart_override` 方法 + 持久化到 accounts.json
- `account/types.rs` AccountBrief From 实现投影 `restart_after_refresh_override` 字段

## 📦 安装包

| 平台 | 文件 |
|------|------|
| macOS (Apple Silicon) | `TraeJumper-1.0.1-mac-arm64.dmg` |
| macOS (Intel) | `TraeJumper-1.0.1-mac-x64.dmg` |
| Windows x64 (NSIS) | `TraeJumper-1.0.1-win-x64-setup.exe` |
| Windows x64 (MSI) | `TraeJumper-1.0.1-win-x64.msi` |

## 📋 升级建议

- **直接覆盖安装**即可，数据格式向后兼容（新增字段全 `#[serde(default)]`，旧 accounts.json 自动补 None/false）
- 升级后到设置页 **续签配置** 确认总开关状态（默认开启 + 重启）
- 单账号覆盖值在账号详情 Modal 里设置，卡片/列表激活标签末尾会显示当前生效状态
- 手动关闭 Trae 后用 **检测当前账号** 按钮立即刷新活跃态标签

## 🛠️ 技术栈

与 v1.0.0 一致：React 19 / Vite 7 / Tauri 2 / Rust / Serde / Tokio

## 🙏 致谢

感谢 v1.0.0 用户反馈的"想对不同账号差异化控制续签重启"需求，直接驱动了本次 per-account 覆盖开关的设计。也感谢反馈"客户端关了但 TJ 还显示活跃"的用户，催生了"检测当前账号"按钮。
