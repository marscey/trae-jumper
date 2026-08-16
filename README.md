# 🚀 TraeJumper

<div align="center">

![TraeJumper](https://img.shields.io/badge/TraeJumper-blue?style=for-the-badge)
![Version](https://img.shields.io/badge/version-1.0.0-green?style=for-the-badge)
![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS-lightgrey?style=for-the-badge)
![License](https://img.shields.io/badge/license-MIT-orange?style=for-the-badge)

**Trae CN / TRAE WORK / 国际版 Trae 多账号管理小工具**

[功能特性](#-功能特性) • [快速开始](#-快速开始) • [使用指南](#-使用指南) • [常见问题](#-常见问题)

</div>

---

## ⭐ Star 星星走起 动动发财手点点 ⭐

> 如果这个项目对你有帮助，请不要吝啬你的 Star ⭐
> 你的支持是我持续更新的最大动力！💪

<div align="center">

### 👆 点击右上角 Star 按钮支持一下吧！ 👆

</div>

---

## 📖 项目简介

TraeJumper 是一款专为 Trae 系列 IDE 用户打造的多账号管理小工具。通过本工具，你可以轻松管理多个 Trae 账号，一键切换账号，实时查看使用量，让你的 Trae IDE 使用体验更加便捷高效！

### 支持的 Trae 应用

| 应用 | 标识 | 说明 |
|------|------|------|
| **Trae CN（国内版）** | `trae-cn` | 字节跳动国内版 AI IDE |
| **TRAE WORK** | `trae-work` | 原 TRAE SOLO CN，2026年6月更名，AI 办公平台 |
| **Trae（国际版）** | `trae` | 海外版 Trae IDE |

> 可在设置页随时切换目标应用，切换后机器码、路径、登录站点、API 端点自动跟随变更。

### 🎯 为什么选择 TraeJumper？

- 🔄 **一键切换账号** - 自动关闭 Trae，切换账号后自动重新打开
- 📊 **实时使用量监控** - 随时查看每个账号的 Token 使用情况
- 🎨 **现代化界面** - 简洁美观的卡片式布局，流畅动画
- 🔒 **安全可靠** - 本地存储，数据安全有保障
- ⚡ **高效便捷** - 支持批量导入导出，快速管理多个账号
- 🛠️ **功能丰富** - 机器码管理、使用记录查询、账号详情查看
- 🖥️ **跨平台** - 支持 Windows 和 macOS

---

## ⚠️ 免责声明

<div align="center">

### 📢 重要提示：请仔细阅读以下声明

</div>

> **本工具仅供学习和技术研究使用，使用前请务必了解以下内容：**

- ⚠️ **风险自负**：使用者需自行承担所有风险，包括但不限于系统损坏、数据丢失、账号异常等
- ⚖️ **法律风险**：本工具可能违反软件使用协议，请自行评估法律风险
- 🚫 **责任豁免**：作者不承担任何直接或间接损失责任
- 📚 **使用限制**：仅限个人学习研究，严禁商业用途
- 🔒 **授权声明**：不得用于绕过软件正当授权机制
- ✅ **同意条款**：继续使用即表示您已理解并同意承担相应风险

<div align="center">

**⚠️ 如果您不同意以上条款，请立即停止使用本工具 ⚠️**

</div>

---

## ✨ 功能特性

### 🎭 账号管理

- ✅ **添加账号**
  - 支持通过 Token 添加账号（支持国内版/国际版）
  - 自动获取账号信息（邮箱、用户名、头像等）
  - 自动绑定机器码

- ✅ **账号切换**
  - 一键切换到指定账号
  - 自动关闭 Trae 客户端
  - 清除旧登录状态（自动识别加密/明文存储）
  - 写入新账号信息
  - 自动重新打开 Trae 客户端
  - 切换前弹出确认对话框

- ✅ **账号信息**
  - 显示账号邮箱、用户名
  - 显示账号状态（正常/异常）
  - 显示账号类型（礼包/普通）
  - 显示当前使用的账号
  - 显示账号添加时间

- ✅ **账号操作**
  - 查看账号详细信息
  - 更新账号 Token
  - 删除账号
  - 复制账号信息

### 📊 使用量监控

- ✅ **实时使用量**
  - 显示今日使用量
  - 显示总使用量
  - 显示剩余额度
  - 使用量进度条可视化

- ✅ **使用记录**
  - 查看详细使用事件
  - 按时间范围筛选
  - 显示每次使用的 Token 数量
  - 显示使用时间和模型信息

### 🔧 机器码管理

- ✅ **Trae 机器码**
  - 查看当前 Trae 机器码
  - 复制机器码到剪贴板
  - 刷新机器码
  - 清除 Trae 登录状态
  - 重置机器码

- ✅ **账号机器码绑定**
  - 每个账号独立绑定机器码
  - 切换账号时自动更新机器码
  - 支持手动绑定机器码

### ⚙️ 系统设置

- ✅ **目标应用切换**
  - 支持 Trae CN / TRAE WORK / 国际版 Trae 三选一
  - 切换后机器码、路径、登录站点、API 端点自动变更
  - 显示各应用的安装状态和数据目录

- ✅ **Trae 路径配置**
  - 自动扫描 Trae 安装路径（支持多路径候选）
  - 手动选择应用文件
  - 保存路径配置
  - 切换账号后自动打开 Trae

- ✅ **数据管理**
  - 导出所有账号数据为 JSON
  - 从 JSON 文件导入账号
  - 清空所有账号数据

### 🎨 界面特性

- ✅ **现代化设计**
  - 简洁美观的卡片式布局
  - 流畅的动画效果
  - 响应式设计

- ✅ **交互体验**
  - Toast 消息提示
  - 确认对话框
  - 加载状态提示
  - 右键菜单
  - 系统托盘（关闭窗口后隐藏到托盘，左键点击恢复，右键菜单退出）

---

## 🚀 快速开始

### 📋 系统要求

- **Windows** 10/11
- **macOS** 10.15+
- Trae IDEWork 已安装（任一变体）
- Node.js 18+ (开发环境)
- Rust 工具链 (开发环境)

### 📥 下载安装

1. 前往 [Releases](https://github.com/marscey/trae-jumper/releases) 页面
2. 下载最新版本的安装包
3. 运行安装程序
4. 启动 TraeJumper

### 🔨 从源码构建

```bash
# 克隆仓库
git clone https://github.com/marscey/trae-jumper.git
cd trae-jumper

# 安装依赖
npm install

# 开发模式运行
npm run tauri dev

# 构建生产版本（macOS 自动生成 .dmg）
npm run tauri build
```

> 构建产物目录：`src-tauri/target/release/bundle/`
> - **macOS**: `.dmg` 安装包（`bundle/dmg/`） + `.app`（`bundle/macos/`）
> - **Windows**: `.msi` 安装包 + `.exe`（`bundle/msi/`）

### 📦 构建 DMG 安装包

macOS 下运行 `npm run tauri build` 后，Tauri 会自动生成 `.dmg` 文件：

```bash
npm run tauri build  # 构建完整发布包

# 产物路径：
# src-tauri/target/release/bundle/dmg/TraeJumper_1.0.0_aarch64.dmg
```

**构建前注意事项：**
1. 确保已安装 Rust 工具链和 Xcode Command Line Tools
2. 首次构建会下载依赖，耗时较长
3. 构建的 `.dmg` 可直接分发，用户拖拽到 Applications 即可安装
4. 如需修改应用版本号，编辑 `src-tauri/tauri.conf.json` 中的 `version` 字段

---

## 📚 使用指南

### 1️⃣ 选择目标应用

首次使用，请先进入 **设置** 页面，在 **目标应用** 区块选择你要管理的 Trae 客户端：

- 系统会自动检测已安装的应用
- 当前选中项会高亮显示
- 切换后机器码、路径、登录站点、API 端点自动跟随变更

### 2️⃣ 配置 Trae 路径

1. 打开应用后，点击左侧菜单的 **设置**
2. 在 **客户端路径** 部分：
   - 点击 **自动扫描** 按钮，系统会自动查找 Trae 应用
   - 或点击 **手动设置** 按钮，选择应用文件
3. 路径配置成功后会显示完整路径

### 3️⃣ 添加账号

#### 方法一：通过 Token 添加

1. 点击右上角的 **添加账号** 按钮
2. 输入你的 Trae Token
3. 点击 **添加** 按钮
4. 系统会自动获取账号信息并保存

#### 获取 Token 的方法

1. 打开 Trae
2. 按 `F12` 打开开发者工具
3. 切换到 `Application` 标签
4. 在左侧找到 `Local Storage` → `vscode-webview://xxx`
5. 找到包含 `iCubeAuthInfo` 的键
6. 复制其中的 `token` 值

### 4️⃣ 切换账号

1. 在账号列表中找到要切换的账号
2. 点击账号卡片上的 **切换** 按钮
3. 在确认对话框中点击 **确定**
4. 系统会自动：
   - 关闭当前运行的 Trae
   - 清除旧账号的登录状态
   - 写入新账号的登录信息
   - 重新打开 Trae

> ⚠️ **注意**：切换账号前请保存 Trae 中的工作内容

### 5️⃣ 查看使用量

#### 查看概览

- 在仪表板页面可以看到所有账号的使用量概览
- 每个账号卡片显示：
  - 今日使用量
  - 总使用量
  - 使用进度条

#### 查看详细记录

1. 点击账号卡片上的 **详情** 按钮
2. 在详情页面切换到 **使用记录** 标签
3. 可以查看：
   - 每次使用的时间
   - 使用的 Token 数量
   - 使用的模型
   - 请求类型

### 6️⃣ 管理机器码

#### 查看 Trae 机器码

1. 进入 **设置** 页面
2. 在 **机器码** 部分可以看到当前机器码
3. 点击 **复制** 按钮可以复制到剪贴板

#### 清除登录状态

1. 在设置页面点击 **清除登录状态** 按钮
2. 确认操作
3. 系统会：
   - 重置 Trae 机器码
   - 清除所有登录信息
   - 删除本地缓存数据

> ⚠️ **注意**：清除登录状态后，Trae 将变成全新安装状态，需要重新登录

### 7️⃣ 数据导入导出

#### 导出账号数据

1. 进入 **设置** 页面
2. 在 **数据管理** 部分点击 **导出** 按钮
3. 选择保存位置
4. 所有账号数据将导出为 JSON 文件

#### 导入账号数据

1. 进入 **设置** 页面
2. 在 **数据管理** 部分点击 **导入** 按钮
3. 选择之前导出的 JSON 文件
4. 账号数据将被导入到应用中

---

## 🔧 本仓库改进（Fork 版）

本仓库基于 [Yang-505/Trae-Account-Manager](https://github.com/Yang-505/Trae-Account-Manager) fork，主要改进如下：

### 1. 多应用变体支持
- **新增 `trae_app.rs`**：定义 Trae 应用变体系统（Trae CN / TRAE WORK / 国际版），各变体独立管理数据目录、安装路径、进程名、登录站点、API Host
- **前端应用选择器**：设置页新增"目标应用"区块，支持随时切换管理目标
- 旧标识 `trae-solo-cn` 自动兼容映射到 `trae-work`

### 2. macOS 适配
- 进程检测/关闭使用 `pgrep`/`pkill`/`osascript` 替代 Windows `taskkill`
- 数据目录从 `APPDATA` 改为 `~/Library/Application Support`
- 安装路径扫描支持 `.app` bundle 格式
- 多路径候选机制（`bundle_paths` / `process_patterns` / `osascript_names`）

### 3. 加密存储支持
- **新增 `crypto.rs`**：从 Trae CN 客户端 `byteCrypto.js` 逆向实现 AES-128-CBC + SHA-512 校验加解密
- 加密格式：`base64( "tc\x05\x10\x00\x00" + randomKey(32B) + AES-128-CBC(SHA512(plain) + plain) )`
- 密钥派生：SHA512(randomKey) + 常量盐 -> SHA512 -> AES key + IV
- 兼容旧版明文存储格式，自动检测加密/非加密值

### 4. 登录流程优化
- 登录 URL 根据当前变体动态选择（`www.trae.cn` / `www.trae.ai`）
- API 端点根据变体自动设置（`api.trae.cn` / `api-sg-central.trae.ai`）
- Origin/Referer 头随变体变化

### 5. TRAE WORK 专属适配
- 确认 TRAE WORK = 原 TRAE SOLO CN（2026-06 更名）
- 兼容新旧安装名（`TRAE SOLO CN.app` / `TraeWork.app`）与数据目录
- 从 SOLO 的 `main.js` 逆向验证加密算法与 Trae CN 完全一致，`crypto.rs` 共用

### 6. 系统托盘（System Tray）支持
- 关闭窗口时隐藏到系统托盘，不退出应用
- 左键点击托盘图标恢复窗口
- 右键托盘菜单：显示窗口 / 退出
- 登录窗口不受影响，可正常关闭

---

## ❓ 常见问题

### Q1: 切换账号后 Trae 没有自动打开？

**A:** 请检查以下几点：
1. 确认已在设置中配置了正确的 Trae 路径
2. 确认应用文件存在且可执行
3. 查看应用日志，确认是否有错误信息

### Q2: 添加账号时提示 Token 无效？

**A:** 请确认：
1. Token 是否正确复制（没有多余的空格或换行）
2. Token 是否已过期
3. 网络连接是否正常

### Q3: 切换账号后 Trae 还是显示旧账号？

**A:** 这种情况很少见，可以尝试：
1. 手动关闭 Trae
2. 在设置中点击"清除登录状态"
3. 重新切换账号

### Q4: 如何备份我的账号数据？

**A:**
1. 进入设置页面
2. 点击"导出数据"按钮
3. 保存 JSON 文件到安全位置
4. 需要恢复时使用"导入数据"功能

### Q5: 应用数据存储在哪里？

**A:**
- Windows: `%APPDATA%\com.marscey.traejumper\`
- macOS: `~/Library/Application Support/com.marscey.traejumper/`
- 包含账号信息、配置等数据

### Q6: Trae 国内版和国际版有什么区别？

**A:**
- **国内版**：登录 `www.trae.cn`，API `api.trae.cn`，使用豆包/DeepSeek 等国产模型
- **国际版**：登录 `www.trae.ai`，API `api-sg-central.trae.ai`，使用 Claude/GPT 等模型
- 数据目录名不同（`Trae CN` vs `Trae`），storage.json 加密格式一致

### Q7: macOS 安装后提示「已损坏，无法打开」或「无法验证开发者」？

**A:** 这不是安装包真的损坏，而是 macOS 的 **Gatekeeper 安全机制**在拦截。TraeJumper 是开源免费应用，没有购买 Apple 开发者账号（$99/年）做签名公证，从网络下载的未签名应用会被系统打上"隔离"标记并阻止运行。

解决方法（任选其一）：

**方法一：终端命令移除隔离标记（推荐）**

```bash
sudo xattr -rd com.apple.quarantine /Applications/TraeJumper.app
```

执行后输入开机密码，再重新打开应用即可。

**方法二：系统设置放行（macOS 13+）**

1. 双击打开应用被拦截后，进入「系统设置 → 隐私与安全性」
2. 滚动到底部，找到关于 "TraeJumper" 已被阻止的提示
3. 点击「仍要打开」→「打开」

> 💡 安全性说明：本应用代码完全开源（本仓库），可自行审查或从源码构建（`npm run tauri build`），不存在的安全风险。旧版 macOS（12 及以下）可直接在弹窗上「右键 → 打开」绕过。

---

## 🛠️ 技术栈

### 前端

- **React 18** - UI 框架
- **TypeScript** - 类型安全
- **Vite** - 构建工具
- **CSS3** - 样式设计

### 后端

- **Tauri 2** - 桌面应用框架
- **Rust** - 后端逻辑
- **Tokio** - 异步运行时
- **Reqwest** - HTTP 客户端
- **Serde** - 序列化/反序列化
- **AES-128-CBC + SHA-512** - 存储值加密
- **rand** - 加密随机数生成

### 功能模块

- **账号管理** - 多账号存储与切换
- **API 客户端** - Trae API 交互（多端点容灾）
- **机器码管理** - 系统注册表/文件操作
- **文件系统** - Trae 配置文件操作
- **进程管理** - Trae 进程控制
- **应用变体管理** - 多 Trae 变体支持
- **加密解密** - storage.json 加密值读写

---

## 📁 项目结构

```
trae-jumper/
├── src/                      # 前端源码
│   ├── components/           # React 组件
│   │   ├── AccountCard.tsx       # 账号卡片
│   │   ├── AddAccountModal.tsx   # 添加账号弹窗
│   │   ├── ConfirmModal.tsx      # 确认对话框
│   │   ├── DetailModal.tsx       # 详情弹窗
│   │   └── ...
│   ├── pages/                # 页面组件
│   │   ├── Dashboard.tsx         # 仪表板
│   │   ├── Settings.tsx          # 设置页面（含目标应用选择器）
│   │   └── About.tsx             # 关于页面
│   ├── api.ts                # API 接口（含 app 变体 API）
│   ├── types/                # TypeScript 类型定义
│   └── App.tsx               # 主应用组件
├── src-tauri/                # Tauri 后端
│   ├── src/
│   │   ├── account/          # 账号管理模块
│   │   │   ├── account_manager.rs  # 账号管理器（支持加密存储解密）
│   │   │   └── types.rs            # 账号类型定义
│   │   ├── api/              # API 客户端模块
│   │   │   ├── trae_api.rs         # Trae API 客户端（多端点容灾）
│   │   │   └── types.rs            # API 类型定义
│   │   ├── crypto.rs         # 加密解密模块（AES-128-CBC + SHA-512）
│   │   ├── trae_app.rs       # 应用变体管理（Trae CN / WORK / 国际版）
│   │   ├── machine.rs        # 机器码管理（跨平台）
│   │   ├── login.rs          # 浏览器登录
│   │   ├── lib.rs            # Tauri 命令注册
│   │   └── main.rs           # 应用入口
│   ├── Cargo.toml            # Rust 依赖配置
│   └── tauri.conf.json       # Tauri 配置
├── package.json              # Node.js 依赖配置
└── README.md                 # 项目文档
```

---

## 🤝 贡献指南

欢迎贡献代码、报告问题或提出建议！

### 如何贡献

1. Fork 本仓库
2. 创建你的特性分支 (`git checkout -b feature/AmazingFeature`)
3. 提交你的更改 (`git commit -m 'Add some AmazingFeature'`)
4. 推送到分支 (`git push origin feature/AmazingFeature`)
5. 开启一个 Pull Request

### 报告问题

如果你发现了 Bug 或有功能建议，请前往 [Issues](https://github.com/marscey/trae-jumper/issues) 页面提交。

---

## 📄 许可证

本项目采用 MIT 许可证 - 查看 [LICENSE](LICENSE) 文件了解详情

---

## 💖 致谢

感谢所有为本项目做出贡献的开发者！

特别感谢：
- [Tauri](https://tauri.app/) - 优秀的桌面应用框架
- [React](https://react.dev/) - 强大的 UI 框架
- [Rust](https://www.rust-lang.org/) - 安全高效的系统编程语言
- [Yang-505/Trae-Account-Manager](https://github.com/Yang-505/Trae-Account-Manager) - 原项目

---

## 📞 联系方式

- GitHub: [@marscey](https://github.com/marscey)
- Issues: [项目 Issues](https://github.com/marscey/trae-jumper/issues)

---

<div align="center">

## ⭐ 再次提醒：别忘了点 Star 哦！⭐

**如果觉得这个项目不错，请给个 Star 支持一下！**

**你的 Star 是持续更新的动力！💪**

Made with ❤️

</div>

---

## 🎉 Star 历史

[![Star History Chart](https://api.star-history.com/svg?repos=Yang-505/Trae-Account-Manager&type=date&legend=top-left)](https://www.star-history.com/#Yang-505/Trae-Account-Manager&type=date&legend=top-left)