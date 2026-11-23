//! 每日签到：虚拟设备指纹、签到/状态查询、批量与自动签到、请求头预览。

use super::*;

impl AccountManager {
    /// 领取生日礼包
    pub async fn claim_birthday_bonus(&mut self, account_id: &str) -> Result<()> {
        let account = self.store.accounts.iter()
            .find(|a| a.id == account_id)
            .ok_or_else(|| anyhow!("账号不存在"))?;
        self.require_trae(account)?;

        let token = account.jwt_token.as_ref()
            .ok_or_else(|| anyhow!("账号没有 Token"))?;

        let client = TraeApiClient::new_with_token(token)?;

        // 先查询是否已领取
        let claimed = client.query_birthday_bonus().await?;
        if claimed {
            return Err(anyhow!("该账号已领取过礼包"));
        }

        // 领取礼包
        client.claim_birthday_bonus().await?;

        println!("[INFO] 成功领取礼包: {}", account.email);
        Ok(())
    }

    /// 确保账号已有签到虚拟设备档案；没有则立即生成并持久化
    ///
    /// 新账号在添加时（Account::new）即分配；此方法兜底处理旧版本
    /// 存量账号——首次签到 / 查状态 / 查看请求头时懒生成并保存。
    /// 生成后永久固定：同一账号今天和明天发起的签到请求，
    /// vscode-sessionid / x-market-user-id / x-device-id / x-device-brand / x-device-type 完全一致。
    fn ensure_checkin_device(&mut self, account_id: &str) -> Result<CheckinDeviceProfile> {
        // 已有档案：直接返回（无借用冲突的快速路径）
        if let Some(account) = self.store.accounts.iter().find(|a| a.id == account_id) {
            if let Some(p) = &account.checkin_device {
                // 防护2：旧版 FNV 生成的 device-id 数值 >=4.5e15，服务端会返回 9074 被拒，
                // 命中则自动用新逻辑（数值 <4.5e15）重新生成（自愈，无需手动重置）。
                if !p.has_legacy_device_id() {
                    return Ok(p.clone());
                }
            }
        }

        let config = self.get_checkin_config();
        let real_device_id = crate::machine::get_trae_device_id().ok();
        let account = self.store.accounts.iter_mut()
            .find(|a| a.id == account_id)
            .ok_or_else(|| anyhow!("账号不存在"))?;

        let profile = CheckinDeviceProfile::generate(&account.id, Some(config.device_id_strategy), real_device_id.as_deref());
        account.checkin_device = Some(profile.clone());
        account.updated_at = chrono::Utc::now().timestamp();
        let email = account.email.clone();

        self.save_store()?;
        println!(
            "[INFO] 已为账号 {} 分配签到虚拟设备: brand={}, device-id={}, market-user-id={}",
            email, profile.device_brand, profile.device_id, profile.market_user_id
        );
        Ok(profile)
    }

    /// 重置所有账号的签到虚拟设备档案（用新生成逻辑重新分配）
    ///
    /// 用于存量账号：早期版本生成的 x-market-user-id 是 UUID v5，
    /// 服务端只认可真实客户端的 UUID v4 机器码（实测 v5 会 9074）。
    /// 重置后所有账号用 v4 重新生成档案并持久化。
    pub fn reset_checkin_devices(&mut self) -> Result<usize> {
        let mut count = 0;
        let config = self.get_checkin_config();
        let real_device_id = crate::machine::get_trae_device_id().ok();
        for account in self.store.accounts.iter_mut() {
            let profile = CheckinDeviceProfile::generate(&account.id, Some(config.device_id_strategy), real_device_id.as_deref());
            account.checkin_device = Some(profile.clone());
            account.updated_at = chrono::Utc::now().timestamp();
            println!(
                "[INFO] 已重置账号 {} 签到虚拟设备: brand={}, device-id={}, market-user-id={}",
                account.email, profile.device_brand, profile.device_id, profile.market_user_id
            );
            count += 1;
        }
        self.save_store()?;
        Ok(count)
    }

    /// 重置单个账号的签到虚拟设备档案（用新生成逻辑重新分配）
    ///
    /// 用于某个账号被风控（如 9074）时，单独换一套设备指纹再试，
    /// 不影响其他账号。新档案用 v4 机器码生成并持久化。
    pub fn reset_checkin_device(&mut self, account_id: &str) -> Result<CheckinDeviceProfile> {
        let config = self.get_checkin_config();
        let real_device_id = crate::machine::get_trae_device_id().ok();
        let account = self
            .store
            .accounts
            .iter_mut()
            .find(|a| a.id == account_id)
            .ok_or_else(|| anyhow!("账号不存在"))?;

        let profile = CheckinDeviceProfile::generate(&account.id, Some(config.device_id_strategy), real_device_id.as_deref());
        account.checkin_device = Some(profile.clone());
        account.updated_at = chrono::Utc::now().timestamp();
        let email = account.email.clone();

        self.save_store()?;
        println!(
            "[INFO] 已重置账号 {} 签到虚拟设备: brand={}, device-id={}, market-user-id={}",
            email, profile.device_brand, profile.device_id, profile.market_user_id
        );
        Ok(profile)
    }

    /// 获取签到全局配置
    pub fn get_checkin_config(&self) -> CheckinConfig {
        self.store.checkin_config.clone().unwrap_or_default()
    }

    /// 更新签到全局配置
    pub fn update_checkin_config(&mut self, config: CheckinConfig) -> Result<()> {
        self.store.checkin_config = Some(config);
        self.save_store()?;
        Ok(())
    }

    /// 重新生成单个账号的 device-id（保持其他字段不变）
    pub fn regenerate_device_id(&mut self, account_id: &str) -> Result<CheckinDeviceProfile> {
        let config = self.get_checkin_config();
        let real_device_id = crate::machine::get_trae_device_id().ok();
        let account = self.store.accounts.iter_mut()
            .find(|a| a.id == account_id)
            .ok_or_else(|| anyhow!("账号不存在"))?;

        let profile = CheckinDeviceProfile::generate(&account.id, Some(config.device_id_strategy), real_device_id.as_deref());
        account.checkin_device = Some(profile.clone());
        account.updated_at = chrono::Utc::now().timestamp();
        let email = account.email.clone();
        self.save_store()?;
        println!("[INFO] 已重新生成账号 {} 的设备 ID: {}", email, profile.device_id);
        Ok(profile)
    }

    /// 更换单个账号的虚拟设备型号（从型号池重新随机分配，不改变其他字段）
    pub fn swap_device_brand(&mut self, account_id: &str) -> Result<CheckinDeviceProfile> {
        let account = self.store.accounts.iter_mut()
            .find(|a| a.id == account_id)
            .ok_or_else(|| anyhow!("账号不存在"))?;

        let mut profile = account.checkin_device.clone()
            .ok_or_else(|| anyhow!("账号没有签到设备档案"))?;

        // 从型号池重新随机分配型号
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        format!("{}_{}", account.id, chrono::Utc::now().timestamp()).hash(&mut hasher);
        let hash = hasher.finish();
        let (device_brand, device_type) = DEVICE_MODELS[(hash as usize) % DEVICE_MODELS.len()];
        profile.device_brand = device_brand.to_string();
        profile.device_type = device_type.to_string();

        account.checkin_device = Some(profile.clone());
        account.updated_at = chrono::Utc::now().timestamp();
        let email = account.email.clone();
        self.save_store()?;
        println!("[INFO] 已更换账号 {} 的虚拟设备型号: {}", email, profile.device_brand);
        Ok(profile)
    }

    /// 查询单个账号今日签到状态（用于列表展示 + 批量签到前跳过已签到账号）
    pub async fn checkin_status(&mut self, account_id: &str) -> Result<CheckinStatusResult> {
        let account = self.store.accounts.iter()
            .find(|a| a.id == account_id)
            .ok_or_else(|| anyhow!("账号不存在"))?
            .clone();
        self.require_trae(&account)?;

        let token = account.jwt_token.clone()
            .ok_or_else(|| anyhow!("账号没有 Token，请先刷新 Token"))?;

        let client = TraeApiClient::new_with_token(&token)?;
        let profile = self.ensure_checkin_device(account_id)?;

        println!(
            "[INFO] 查询签到状态 (device-id={}, market-user-id={}, brand={})",
            profile.device_id, profile.market_user_id, profile.device_brand
        );

        client.checkin_status(&profile).await
    }

    /// 批量查询所有账号的今日签到状态
    pub async fn checkin_status_all(&mut self) -> Result<Vec<(String, String, Option<CheckinStatusResult>)>> {
        let mut results = Vec::new();

        // 快照账号列表，避免迭代借用与 &mut self 方法调用冲突
        let snapshot: Vec<(String, String, bool)> = self.store.accounts.iter()
            .map(|a| (a.id.clone(), a.name.clone(), a.jwt_token.is_some()))
            .collect();
        let total = snapshot.len();

        for (idx, (account_id, account_name, has_token)) in snapshot.into_iter().enumerate() {
            if !has_token {
                results.push((account_id, account_name, None));
                continue;
            }

            match self.checkin_status(&account_id).await {
                Ok(s) => results.push((account_id, account_name, Some(s))),
                Err(e) => {
                    println!("[WARN] 查询签到状态失败 {}: {}", account_name, e);
                    results.push((account_id, account_name, None));
                }
            }

            // 状态查询之间加 1~3 秒小延迟，避免瞬时高密度请求
            // ThreadRng 是 !Send 不能跨 await，所以必须在独立 scope 中生成数值后再 await
            if idx + 1 < total {
                let delay_ms = {
                    use rand::Rng;
                    let mut rng = rand::thread_rng();
                    rng.gen_range(1_000u64..=3_000u64)
                };
                let _ = tokio::time::timeout(
                    std::time::Duration::from_secs(10),
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)),
                ).await;
            }
        }

        Ok(results)
    }

    /// 每日签到（使用账号专属的持久化虚拟设备档案）
    pub async fn checkin(&mut self, account_id: &str) -> Result<CheckinResult> {
        let account = self.store.accounts.iter()
            .find(|a| a.id == account_id)
            .ok_or_else(|| anyhow!("账号不存在"))?
            .clone();
        self.require_trae(&account)?;

        let token = account.jwt_token.clone()
            .ok_or_else(|| anyhow!("账号没有 Token，请先刷新 Token"))?;

        let client = TraeApiClient::new_with_token(&token)?;
        let profile = self.ensure_checkin_device(account_id)?;

        println!(
            "[INFO] 签到 (device-id={}, market-user-id={}, brand={})",
            profile.device_id, profile.market_user_id, profile.device_brand
        );

        let result = client.checkin(&profile).await?;

        if result.code == 0 {
            println!("[INFO] 签到成功");
        } else {
            println!("[WARN] 签到失败: {} - {}", result.code, result.message);
        }

        Ok(result)
    }

    /// 批量签到所有账号
    ///
    /// 行为：
    ///   1. 先查每个账号的签到状态，已签到的直接跳过，避免重复调用 claim 接口
    ///   2. 随机延迟只在「两次真实签到 claim 之间」生效：
    ///      - 跳过已签到账号、查询状态接口都不触发延迟
    ///      - 第一个需要签到的账号立即发起，无需等待
    ///      - 若连续多个账号都需签到，则在每次 claim 前等待随机延迟，避免触发风控
    ///   3. 返回每条的 skipped 状态，供前端区分"跳过（已签到）"与"签到成功"
    pub async fn checkin_all(&mut self) -> Result<Vec<(String, String, CheckinResult, bool)>> {
        let mut results = Vec::new();

        // 快照账号列表，避免迭代借用与 &mut self 方法调用冲突
        let snapshot: Vec<(String, String, bool)> = self.store.accounts.iter()
            .map(|a| (a.id.clone(), a.name.clone(), a.jwt_token.is_some()))
            .collect();

        // 是否存在「待补偿的随机延迟」：仅在上一个账号真正发起过 claim 后置为 true。
        // 这样跳过账号、状态查询都不会引入延迟，第一个需签到账号也能立即发起。
        let mut pending_delay = false;

        for (account_id, account_name, has_token) in snapshot.into_iter() {
            if !has_token {
                results.push((
                    account_id,
                    account_name,
                    CheckinResult {
                        code: -1,
                        message: "账号没有 Token".to_string(),
                    },
                    false,
                ));
                continue;
            }

            // ---------- Step 1: 先查签到状态（不做随机延迟），已签到则跳过 claim ----------
            let already_checked = match self.checkin_status(&account_id).await {
                Ok(status) if status.code == 0 && status.checked_in => true,
                _ => false,
            };

            if already_checked {
                println!("[INFO] 跳过已签到账号: {}", account_name);
                results.push((
                    account_id,
                    account_name,
                    CheckinResult {
                        code: 0,
                        message: "今日已签到".to_string(),
                    },
                    true,
                ));
                continue;
            }

            // ---------- Step 2: 真正发起 claim 前，若上一个账号刚 claim 过，才等待随机延迟 ----------
            // ThreadRng 是 !Send 不能跨 await，必须先在独立 scope 中取数值再 await
            if pending_delay {
                let config = self.get_checkin_config();
                let delay_secs = {
                    use rand::Rng;
                    let mut rng = rand::thread_rng();
                    rng.gen_range(config.claim_delay_min..=config.claim_delay_max)
                };
                println!("[INFO] 等待 {} 秒后签到下一个账号...", delay_secs);
                let _ = tokio::time::timeout(
                    std::time::Duration::from_secs(delay_secs + 5),
                    tokio::time::sleep(std::time::Duration::from_secs(delay_secs)),
                ).await;
                pending_delay = false;
            }

            // ---------- Step 3: 执行签到 claim ----------
            match self.checkin(&account_id).await {
                Ok(r) => results.push((account_id, account_name, r, false)),
                Err(e) => results.push((
                    account_id,
                    account_name,
                    CheckinResult {
                        code: -1,
                        message: e.to_string(),
                    },
                    false,
                )),
            }
            pending_delay = true;
        }

        Ok(results)
    }

    /// 自动签到：在设定时间点之后、零点之前，为今日尚未签到的账号批量签到。
    ///
    /// 触发条件：
    ///   - 配置开启 auto_checkin_enabled
    ///   - 当前本地时间 >= auto_checkin_time（HH:mm）
    ///   - 今日尚未执行过自动签到（内存态去重，跨天自动重置）
    ///
    /// 防跨零点策略：
    ///   - 预留 2 分钟缓冲，确保最后一个账号的 claim 在零点前完成
    ///   - 每一步的延迟 = min(随机延迟, 剩余可用时间 / 剩余待签到账号数)
    ///   - 若可用时间不足以容纳最小延迟，则跳过延迟立即签到，宁可密集也不跨零点
    pub async fn auto_checkin(&mut self) -> Result<usize> {
        use chrono::{Local, Timelike};
        use rand::Rng;

        let config = self.get_checkin_config();
        if !config.auto_checkin_enabled {
            return Ok(0);
        }

        let now = Local::now();
        let today = now.format("%Y-%m-%d").to_string();

        // 同一天内只触发一次
        if self.last_auto_checkin_date.as_deref() == Some(today.as_str()) {
            return Ok(0);
        }

        // 解析触发时间 HH:mm
        let time_str = &config.auto_checkin_time;
        let mut parts = time_str.split(':');
        let trigger_hour: u32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(22);
        let trigger_minute: u32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);

        let now_minutes = now.hour() * 60 + now.minute();
        let trigger_minutes = trigger_hour * 60 + trigger_minute;
        if now_minutes < trigger_minutes {
            return Ok(0);
        }

        // 计算到零点的剩余秒数（含缓冲）
        let secs_until_midnight = (24 * 3600)
            - (now.hour() as u64 * 3600 + now.minute() as u64 * 60 + now.second() as u64);
        const MIDNIGHT_BUFFER_SECS: u64 = 120;
        let available_secs = secs_until_midnight.saturating_sub(MIDNIGHT_BUFFER_SECS);
        if available_secs == 0 {
            println!("[INFO][auto-checkin] 距零点不足 {} 秒，跳过自动签到", MIDNIGHT_BUFFER_SECS);
            return Ok(0);
        }

        // 快照：有 token 的账号
        let snapshot: Vec<(String, String)> = self.store.accounts.iter()
            .filter(|a| a.jwt_token.is_some())
            .map(|a| (a.id.clone(), a.name.clone()))
            .collect();

        if snapshot.is_empty() {
            self.last_auto_checkin_date = Some(today);
            return Ok(0);
        }

        println!("[INFO][auto-checkin] 开始自动签到，待检查账号 {} 个，距零点可用 {} 秒", snapshot.len(), available_secs);

        // 先查每个账号的签到状态，收集需要签到的
        let mut pending: Vec<(String, String)> = Vec::new();
        for (account_id, account_name) in &snapshot {
            let already = match self.checkin_status(account_id).await {
                Ok(status) if status.code == 0 && status.checked_in => true,
                _ => false,
            };
            if already {
                println!("[INFO][auto-checkin] 跳过已签到: {}", account_name);
            } else {
                pending.push((account_id.clone(), account_name.clone()));
            }
        }

        if pending.is_empty() {
            println!("[INFO][auto-checkin] 所有账号今日已签到");
            self.last_auto_checkin_date = Some(today);
            return Ok(0);
        }

        println!("[INFO][auto-checkin] 需签到 {} 个账号", pending.len());

        // 逐个签到，动态调整延迟确保不跨零点
        let total = pending.len();
        let mut done = 0usize;
        let mut consumed_secs: u64 = 0;

        for (idx, (account_id, account_name)) in pending.into_iter().enumerate() {
            // 剩余待签到数（含当前）
            let remaining = (total - idx) as u64;
            // 剩余可用时间
            let remaining_available = available_secs.saturating_sub(consumed_secs);
            // 本次 claim 前最多可等待的时间（留一点给后续账号）
            let max_allowable = if remaining > 1 {
                remaining_available / remaining
            } else {
                remaining_available
            };

            // 第一个账号不等待
            let delay_secs = if idx == 0 {
                0
            } else if max_allowable == 0 {
                // 时间不够，跳过延迟立即签到
                println!("[WARN][auto-checkin] 时间紧张，跳过延迟立即签到: {}", account_name);
                0
            } else {
                let mut rng = rand::thread_rng();
                let base = rng.gen_range(config.claim_delay_min..=config.claim_delay_max);
                base.min(max_allowable)
            };

            if delay_secs > 0 {
                println!("[INFO][auto-checkin] 等待 {} 秒后签到: {}", delay_secs, account_name);
                let _ = tokio::time::timeout(
                    std::time::Duration::from_secs(delay_secs + 5),
                    tokio::time::sleep(std::time::Duration::from_secs(delay_secs)),
                ).await;
                consumed_secs += delay_secs;
            }

            match self.checkin(&account_id).await {
                Ok(r) if r.code == 0 => {
                    done += 1;
                    println!("[INFO][auto-checkin] 签到成功: {}", account_name);
                }
                Ok(r) => {
                    println!("[WARN][auto-checkin] 签到失败 {}: {} - {}", account_name, r.code, r.message);
                }
                Err(e) => {
                    println!("[ERROR][auto-checkin] 签到异常 {}: {}", account_name, e);
                }
            }
        }

        self.last_auto_checkin_date = Some(today);
        println!("[INFO][auto-checkin] 自动签到完成，成功 {}/{}", done, total);
        Ok(done)
    }

    /// 生成账号签到请求头的完整预览（供前端"查看签到请求头"弹窗展示）
    ///
    /// 按 fixed（固定值）/ account（账号专属虚拟设备）/ credential（身份凭证）/
    /// dynamic（每次请求变化）四类标注每个请求头。
    pub fn get_checkin_header_preview(&mut self, account_id: &str) -> Result<Vec<CheckinHeaderEntry>> {
        let account = self.store.accounts.iter()
            .find(|a| a.id == account_id)
            .ok_or_else(|| anyhow!("账号不存在"))?
            .clone();
        self.require_trae(&account)?;

        let profile = self.ensure_checkin_device(account_id)?;

        // authorization 脱敏展示
        let auth_value = match &account.jwt_token {
            Some(t) if t.len() > 24 => format!("Cloud-IDE-JWT {}...（共 {} 字符）", &t[..24], t.len()),
            Some(t) => format!("Cloud-IDE-JWT {}", t),
            None => "（账号暂无 Token，签到前会自动刷新）".to_string(),
        };

        let entry = |name: &str, value: &str, kind: &str, note: &str| CheckinHeaderEntry {
            name: name.to_string(),
            value: value.to_string(),
            kind: kind.to_string(),
            note: note.to_string(),
        };

        Ok(vec![
            // ---- 固定值（所有账号一致，对齐真实客户端抓包）----
            entry("user-agent", "VSCode 1.107.1 (TRAE SOLO CN)", "fixed",
                  "客户端标识，固定值（当前仅 TraeWork CN 客户端有签到入口）"),
            entry("x-market-client-id", "VSCode 1.107.1", "fixed", "市场客户端 ID，固定值"),
            entry("x-user-region", "CN", "fixed", "用户区域，固定值"),
            entry("x-lgw-req-sdk-type", "3", "fixed", "网关 SDK 类型，固定值"),
            entry("package-type", "stable_cn", "fixed", "发行渠道，固定值"),
            entry("app-version", "0.1.52", "fixed", "客户端版本，固定值"),
            entry("content-type", "application/json", "fixed", "请求体类型，固定值"),
            entry("accept", "*/*", "fixed", "可接受响应类型，固定值"),
            entry("accept-language", "zh-CN", "fixed", "客户端界面语言，固定值"),
            entry("accept-encoding", "gzip, deflate, br, zstd", "fixed", "HTTP 压缩协商，固定值"),
            entry("sec-fetch-dest", "empty", "fixed", "Electron 渲染进程自动附加的安全头，固定值"),
            entry("sec-fetch-mode", "no-cors", "fixed", "Electron 渲染进程自动附加的安全头，固定值"),
            entry("sec-fetch-site", "none", "fixed", "Electron 渲染进程自动附加的安全头，固定值"),
            // ---- 账号专属（虚拟设备档案，持久化，跨天不变）----
            entry("vscode-sessionid", &profile.session_id, "account",
                  "会话 ID，账号专属，分配后永久不变"),
            entry("x-market-user-id", &profile.market_user_id, "account",
                  "机器码（即 Trae 客户端 machineid 文件内容），账号专属，永久不变"),
            entry("x-device-id", &profile.device_id, "account",
                  "设备 ID，账号专属，永久不变；服务端按此字段限制每台设备每日签到次数"),
            entry("x-device-brand", &profile.device_brand, "account",
                  "虚拟设备型号（mac / windows 真实型号池），账号专属，永久不变"),
            entry("x-device-type", &profile.device_type, "account",
                  "设备平台（mac / windows，均为真实客户端同构），永久不变"),
            entry("x-lscbd-aid", "787976", "account",
                  "客户端数据上报 App ID（仅 windows 设备发送，macOS 抓包无此头），恒定值"),
            entry("x-lscbd-platform", "windows", "account",
                  "数据上报平台标识，与设备平台一致（仅 windows 设备发送）"),
            // ---- 身份凭证 ----
            entry("authorization", &auth_value, "credential",
                  "账号身份凭证（Cloud-IDE-JWT + Token），随 Token 刷新而变化，已脱敏"),
            // ---- 每次请求变化 ----
            entry("x-request-id", "(每次请求重新生成 UUID)", "dynamic",
                  "请求唯一标识，每次请求都不同（同一账号同一设备亦然）"),
            entry("x-tt-trace-id", "(每次请求重新生成 00-…-…-01)", "dynamic",
                  "链路追踪 ID，每次请求都不同（同一账号同一设备亦然）"),
        ])
    }
}
