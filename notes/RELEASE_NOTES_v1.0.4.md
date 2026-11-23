# Release Notes — v1.0.4

> 发布日期：2026-11-23
> 最终 commit chain:
> - `d7f1240` — feat: 账号详情远端刷新 + OAuth 三路续签 + WebView cookies 自动采集（基底）
> - `774c895` — feat: 重构自动更新流程，新增侧边栏更新入口与详情弹窗
> - `99518a9` — feat: 豆包账号体系完整支持 + 账号管理平台隔离 + 设置页优化 + bump 1.0.4
> 上一版本：v1.0.3 (`7793625`)

---

## 🚀 新功能

### 豆包（Doubao）账号体系完整支持

新增独立的 `platform = "doubao"` 账号类型，与 TRAE 账号并列管理。核心模块：

| 模块 | 说明 |
|---|---|
| `src-tauri/src/doubao_client.rs` | **macOS 豆包客户端登录态读取** — 定位 `~/Library/Application Support/Doubao/` 下的 Chromium Profile cookies DB，解密链路：`security find-generic-password -s "Doubao Safe Storage"` → PBKDF2-HMAC-SHA1(saltysalt, 1003) → AES-128-CBC 解密 → 剥离 Chrome v24+ 的 SHA256(host_key) 前缀 → 过滤控制字符。缓存密钥到磁盘避免重复弹钥匙串授权框 |
| `src-tauri/src/api/doubao_api.rs` + `doubao_types.rs` | 豆包 API 客户端 — `/alice/passport/web/account/get_info` 获取账号信息、`/alice/ug/campaign/landing/checkin/` 签到（幂等，D10 端点 query 即 claim）、`/alice/passport/web/v3/benefit/overview` 额度查询 |
| `src-tauri/src/account/account_manager/doubao.rs` | DoubaoManager — `doubao_import_from_client` 从客户端一键导入/刷新；`checkin_with_fallback` 先用账号 cookies 签到返回 `710010702 系统错误` 时回退豆包客户端登录态（**强制校验 `login.user_id == account.user_id` 防串号**） |

**Profile 选择算法**：优先解析 `Local State` JSON 的 `profile.last_used`；兜底所有 Profile 按 `info_cache.<name>.active_time` **降序**（最近活跃的靠前）。一次 Local State 读，缓存 HashMap 全量复用，避免 N+1 IO。

**关键踩坑**：`multi_sids` cookie（`uid%3Asid%7Cuid%3Asid`）所有分隔符都被 percent-encode — **必须先对整个值 percent-decode，再按 `|` split，最后 `:` 拆 uid:sid**。之前用 `split(',')` 永远匹配不到，全靠 first_uid 兜底，容易串号。

### TRAE / 豆包账号平台隔离

账号管理页新增 **Tab 过滤**，两个 Tab 各自独立的操作域：

| 维度 | TRAE Tab | 豆包 Tab |
|---|---|---|
| 批量刷新 | 刷新 TRAE 账号 | 刷新豆包账号 |
| 批量签到 | 调 `checkinAll`（Trae API） | 逐个调 `doubaoCheckin` |
| 切换账号按钮 | ✅ 可用 | ❌ 不渲染 |
| 从客户端读取 | ✅ | ✅（豆包客户端） |
| 检测当前账号 | ✅ | ❌ 不渲染 |
| 登录来源标签 | `TRAE 客户端读取` | `豆包客户端读取` |

**Tab 切换时自动清空选中集**，避免跨 Tab 误操作。选中状态仅在当前 Tab 账号集内生效，全选 checkbox 的 checked 态也只看当前 Tab 账号。

### 设置页重构（方案 B：Section 归属标签）

现有 Section 按功能归属标注影响范围，**不搞 Tab 切换**（当前豆包专属设置只有 1-2 项，提前搞 Tab 会显得空）：

```
🔧 通用设置          [所有账号]     ← 置顶
🤖 TRAE 客户端       [TRAE 账号专属]
🔑 TRAE Token 续签   [TRAE 账号专属]
✍️ 签到配置          [所有账号]
  └ 设备 ID 生成策略   [TRAE 账号专属]  ← 内嵌小标签
  └ 设备型号池          [TRAE 账号专属]
  └ 重置所有签到设备    [TRAE 账号专属]
📦 数据管理          [所有账号]
```

标签采用 pill 形状 + 1px 边框 + 更深色文字（蓝 `#1d4ed8` / 橙 `#c2410c` / 灰 `#475569`），字号 12px + 700 bold，不抢标题风头但一眼可辨。**所有用户可见字符串 `Trae` 统一大写为 `TRAE`**（代码变量名/类型名保持驼峰不动）。

### 重构自动更新流程

- 侧边栏新增版本号入口（点击弹出更新弹窗）
- 重构 UpdateModal：独立于 UpdateBanner，显示版本差异 + Release Notes + 下载进度
- 移除旧 UpdateBanner（Toast 形式的更新提示）

### TRAE 账号详情刷新 + OAuth 三路续签 + WebView cookies 自动采集

（d7f1240 基底提交的功能，详见原始 commit message）

---

## 🐛 Bug 修复（含 Code Review 修复）

### Code Review 10 项修复

| # | 问题 | 修复 |
|---|---|---|
| 1 | `handleBatchRefresh` 的 useCallback deps 缺少 `platformFilter`，切 Tab 后闭包仍捕获旧值 | deps array 追加 `platformFilter` |
| 2 | Trae 有选中签到路径把签到前的 `checkinStatus`（`checked_in=false`）写回本地，签到成功后用户看到本地仍是 false | 签到成功后构造 `{code:0, checked_in:true, credits:200, enable:true}` 写入 |
| 3 | `doubao_checkin_status` 名字语义为"查状态"但实际调用 claim（D10 接口即查询即签到） | doc 注释明确标注副作用 + 幂等说明 |
| 4 | `is_doubao_system_error` 依赖中文字面量 `contains("系统错误")` 可能失效 | 已满足（code 710010702 优先匹配，中文兜底） |
| 5 | 豆包批量签到后不刷新额度（`doubao_quota` 停留在旧值） | 批量后加 `Promise.allSettled(ids.map(refreshAccount))` |
| 6 | `build_headers` 的 `host` 参数是死代码（内部 `let _ = host;`），`accounts.doubao.com` 端点带错误 Origin/Referer | 按 `host.contains("accounts.")` 条件生成 Origin/Referer |
| 7 | `resolve_user_id` 匹配不到 sessionid 时静默回退 `first_uid`，可能返回错误账号 | 严格匹配 + **先 decode 整个 multi_sids 再 split**（分隔符也是 percent-encoded 的）+ 无匹配直接 None |
| 8 | prod `println!` 打印 Cookie 片段 + 完整 API body（release 仍执行） | 所有 debug println! 加 `#[cfg(debug_assertions)]`，release 自动消失 |
| 9 | `collect_cookie_dbs` 在 fallback 循环里对每个 Profile 重复读 Local State | 开头读一次并缓存 HashMap，全量复用 |
| 10 | Cookie 清洗后为空串时 `DoubaoApiClient::new` 不校验，后续请求报 401 | 清洗后 `cleaned.is_empty()` 返回明确错误 + 原始输入长度提示 |

### multi_sids 解析链根因

```
真实解密后值: 4333589105537683%3AsidA%7C1831141604067483%3AsidB
                                          ↑
                                    分隔符 | 也是 percent-encoded 的!

之前错误顺序（永远匹配不到，全靠 first_uid 兜底，易串号）:
  raw.contains('|')? → false（raw 里是 %7C）
  raw.split(',')     → 整条大串
  percent_decode → split(':') → 第一条 uid + 整条剩余
  first_uid 兜底返回第一个 uid（不保证正确）

修复后正确顺序:
  percent_decode(整个 raw) → 得到 "uid:sid|uid:sid"
  split('|') → 两条
  每条 split(':') → 精确匹配 sessionid ✅
```

### Cookie header 控制字符

Chrome v10 AES-128-CBC 解密后可能残留 `\x00` 等控制字符，reqwest 的 `HeaderValue::from_bytes` 严格拒绝。修复：`doubao_client.rs` 拼接时**每个 value 都过滤非可见 ASCII**（保留 `0x20..=0x7e` + `\t`）；`doubao_api.rs::DoubaoApiClient::new` 再做一次兜底过滤 + 空串校验。

---

## 📦 构建产物

| 平台 | 架构 | 下载链接 |
|---|---|---|
| macOS (Apple Silicon) | arm64 | [.dmg](https://github.com/marscey/trae-jumper/releases/download/v1.0.4/TraeJumper-1.0.4-mac-arm64.dmg) · [.app.tar.gz](https://github.com/marscey/trae-jumper/releases/download/v1.0.4/TraeJumper-1.0.4-mac-arm64.app.tar.gz) |
| macOS (Intel) | x64 | [.dmg](https://github.com/marscey/trae-jumper/releases/download/v1.0.4/TraeJumper-1.0.4-mac-x64.dmg) · [.app.tar.gz](https://github.com/marscey/trae-jumper/releases/download/v1.0.4/TraeJumper-1.0.4-mac-x64.app.tar.gz) |
| Windows | x64 | [.exe](https://github.com/marscey/trae-jumper/releases/download/v1.0.4/TraeJumper-1.0.4-win-x64-setup.exe) |
| latest.json | — | [自动更新](https://github.com/marscey/trae-jumper/releases/download/v1.0.4/latest.json) |

---

## 🔧 技术变更

- **新增文件**（6 个，+3200 行）：
  - `src-tauri/src/doubao_client.rs` — 豆包客户端 Chromium Cookie 解密链路
  - `src-tauri/src/api/doubao_api.rs` — 豆包 API 客户端（3 端点）
  - `src-tauri/src/api/doubao_types.rs` — 豆包账号/签到/额度类型
  - `src-tauri/src/account/account_manager/doubao.rs` — DoubaoManager
  - `src/components/UpdateModal.tsx` — 自动更新详情弹窗
  - `notes/RELEASE_NOTES_v1.0.4.md` — 本文件

- **修改文件**（34 个，+4593 / -551 行）：
  - 后端：`account_manager.rs`（doubao_import_from_client）、`types.rs`（platform/login_source 扩展）、`import_export.rs`（doubao 导入导出）、`checkin.rs`（签到 fallback 改豆包回退）、`lib.rs`（注册新 Tauri command）、`api/mod.rs`、`token.rs`
  - 前端：`accounts` 全链路（Tab 过滤 + 批量操作隔离）、`App.tsx`（platformFilter 状态托管）、`Settings.tsx` + 所有 Section（归属标签 + 文案统一）、`AccountCard/ListItem`（登录来源平台感知）、`AddAccountModal`（豆包大卡片登录方式）、`App.css`（section-badge 样式）、`About.tsx`、`types/index.ts`

---

## 🎯 升级影响

- **用户侧**：已有 TRAE 账号不受影响；可在「添加账号 → 豆包 Tab → 从豆包客户端读取」一键导入豆包账号。豆包签到如果 Cookie 过期（710010702 风控）会自动回退到客户端登录态。
- **开发者侧**：`platform` 字段是新账号类型判定的唯一依据（`platform !== "doubao"` 视为 TRAE 账号）。OAuth Native 链路仅对 TRAE 账号生效（豆包账号无 JWT）。签到 fallback 强制校验 `login.user_id == account.user_id`，未来扩展其他平台回退链路务必保留该校验。

---

## ⚠️ 已知限制

1. **豆包 Cookie 无长期续签** — 豆包 API 无类似 OAuth 的刷新 token，Cookies 过期（约 30 天空闲）需重新从客户端读取
2. **豆包首次读取需授权钥匙串** — macOS 系统钥匙串首次访问豆包浏览器加密密钥会弹授权框，点「始终允许」后自动缓存密钥到本地，后续不再弹窗
3. **设置页暂未按平台分 Tab** — 目前通过 Section 标签（TRAE 账号专属 / 所有账号）区分归属；豆包专属设置项（当前仅 1-2 项）积累到 3+ 后将升级为 Tab 切换结构
