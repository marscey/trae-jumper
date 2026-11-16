//! 用量与积分查询：配额/积分汇总、用量事件、401 仲裁与活跃账号判定。

use super::*;

impl AccountManager {
    /// 获取账号使用量
    pub async fn get_account_usage(&mut self, account_id: &str) -> Result<UsageSummary> {
        let account = self
            .store
            .accounts
            .iter()
            .find(|a| a.id == account_id)
            .ok_or_else(|| anyhow!("账号不存在"))?
            .clone();

        // 根据账号类型选择不同的方式获取使用量
        let summary = if let Some(token) = &account.jwt_token {
            // 优先使用 Token
            let client = TraeApiClient::new_with_token(token)?;
            match client.get_usage_summary_by_token().await {
                Ok(summary) => summary,
                Err(e) => {
                    let error_msg = e.to_string();
                    // 如果是 401 错误且有 Cookies，归属仲裁后决定是否刷新 Token
                    if error_msg.contains("401") && !account.cookies.is_empty()
                        && !self.arbitrate_401_and_check_active(&account_id) {
                        println!("[INFO] Token 已过期，尝试使用 Cookies 刷新...");
                        // 使用 Cookies 刷新 Token
                        let mut cookie_client = TraeApiClient::new(&account.cookies)?;
                        let token_result = cookie_client.get_user_token().await?;

                        // 更新存储的 Token
                        if let Some(acc) = self.store.accounts.iter_mut().find(|a| a.id == account_id) {
                            acc.jwt_token = Some(token_result.token.clone());
                            acc.token_expired_at = Some(token_result.expired_at.clone());
                            acc.token_shared_with_client = false; // 新 token 为 TraeJumper 独占
                        }
                        self.save_store()?;

                        // 使用新 Token 重新获取使用量
                        let new_client = TraeApiClient::new_with_token(&token_result.token)?;
                        new_client.get_usage_summary_by_token().await?
                    } else if error_msg.contains("401") {
                        // 活跃客户端账号：尝试从客户端同步最新 Token（客户端会自动续签）
                        if self.is_active_client_account(&account_id) {
                            if let Ok(Some(synced_id)) = self.sync_active_client_token() {
                                if synced_id == account_id {
                                    if let Some(acc) = self.store.accounts.iter().find(|a| a.id == account_id) {
                                        if let Some(new_token) = acc.jwt_token.clone() {
                                            // 从客户端同步 Token 成功，重试
                                            let new_client = TraeApiClient::new_with_token(&new_token)?;
                                            return new_client.get_usage_summary_by_token().await;
                                        }
                                    }
                                }
                            }
                        }
                        self.mark_account_token_expired(&account_id)?;
                        return Err(anyhow!("Token 已过期，请更新 Token 或 Cookies"));
                    } else {
                        return Err(e);
                    }
                }
            }
        } else if !account.cookies.is_empty() {
            // 使用 Cookies
            let mut client = TraeApiClient::new(&account.cookies)?;
            client.get_usage_summary().await?
        } else {
            return Err(anyhow!("账号没有有效的 Token 或 Cookies"));
        };

        // 更新账号的 plan_type
        if let Some(acc) = self.store.accounts.iter_mut().find(|a| a.id == account_id) {
            acc.plan_type = summary.plan_type.clone();
            acc.updated_at = chrono::Utc::now().timestamp();
        }
        self.save_store()?;

        Ok(summary)
    }

    /// 获取账号积分使用量（CN / WORK 积分体系优先，失败自动回退旧配额 UsageSummary）
    ///
    /// 返回值语义：
    /// - `CreditSummary.is_credits_billing == true`：前端按积分新 UI 渲染
    /// - `CreditSummary.is_credits_billing == false`：前端应回退，再调用 `get_account_usage`
    ///   用旧 `UsageSummary` 显示（国际版 entitlements 配额）
    pub async fn get_account_credits(&mut self, account_id: &str) -> Result<CreditSummary> {
        let account = self
            .store
            .accounts
            .iter()
            .find(|a| a.id == account_id)
            .ok_or_else(|| anyhow!("账号不存在"))?
            .clone();

        // 是否为国内版（CN / WORK）：优先用积分接口
        let is_cn = crate::trae_app::current().is_cn
            || account.region.eq_ignore_ascii_case("cn");

        let try_credits_with_token = |token: &str| {
            let token = token.to_string();
            async move {
                let client = TraeApiClient::new_with_token(&token)?;
                client.get_credits_billing_status_by_token().await
            }
        };

        let try_usage_with_token_as_fallback = |token: &str| {
            let token = token.to_string();
            async move {
                // 旧配额体系兜底：拿到 UsageSummary 后包装成 CreditSummary
                // （is_credits_billing = false，前端再单独 invoke get_account_usage 拿完整字段）
                let client = TraeApiClient::new_with_token(&token)?;
                let summary = client.get_usage_summary_by_token().await?;
                Ok::<CreditSummary, anyhow::Error>(CreditSummary {
                    is_credits_billing: false,
                    plan_name: summary.plan_type.clone(),
                    plan_expire_time: summary.reset_time,
                    ..Default::default()
                })
            }
        };

        // 主流程：Token 优先
        let summary = if let Some(token) = &account.jwt_token {
            if is_cn {
                // CN/WORK：先积分接口
                match try_credits_with_token(token).await {
                    Ok(c) => {
                        // 若接口明确告知"不是积分计费"，直接返回（前端会 fallback）
                        if !c.is_credits_billing {
                            // is_credits_billing=false 时前端回退旧配额展示
                        }
                        c
                    }
                    Err(e) => {
                        let err_msg = e.to_string();
                        // 401 → 归属仲裁（区分被客户端顶死 / 自然过期）后决定是否续签
                        if err_msg.contains("401") && !account.cookies.is_empty()
                            && !self.arbitrate_401_and_check_active(&account_id) {
                            println!("[INFO] 积分接口 401，尝试使用 Cookies 刷新 Token...");
                            let refreshed = self.refresh_token_inner(&account_id).await;
                            if let Some(new_token) = refreshed {
                                match try_credits_with_token(&new_token).await {
                                    Ok(c) => c,
                                    Err(e2) => {
                                        println!("[WARN] 刷新后积分接口仍失败: {}，回退旧配额", e2);
                                        try_usage_with_token_as_fallback(&new_token).await?
                                    }
                                }
                            } else {
                                self.mark_account_token_expired(&account_id)?;
                                return Err(anyhow!("Token 已过期，刷新失败，请手动更新 Token 或 Cookies"));
                            }
                        } else if err_msg.contains("401") {
                            // 无 Cookies：尝试从客户端同步活跃账号的最新 Token（客户端会自动续签）
                            // 若当前账号正好是客户端登录的账号，同步后用新 Token 重试
                            if let Ok(Some(synced_id)) = self.sync_active_client_token() {
                                if synced_id == account_id {
                                    if let Some(acc) = self.store.accounts.iter().find(|a| a.id == account_id) {
                                        if let Some(new_token) = acc.jwt_token.clone() {
                                            // 从客户端同步 Token 成功，重试积分接口
                                            match try_credits_with_token(&new_token).await {
                                                Ok(c) => {
                                                    self.save_store()?;
                                                    return Ok(c);
                                                }
                                                Err(e2) => {
                                                    println!("[WARN] 同步后积分接口仍失败: {}，回退旧配额", e2);
                                                    let fallback = try_usage_with_token_as_fallback(&new_token).await;
                                                    self.save_store()?;
                                                    return fallback;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            self.mark_account_token_expired(&account_id)?;
                            return Err(anyhow!("Token 已过期，请更新 Token 或 Cookies"));
                        } else {
                            // 其他错误（如非积分账号、接口 404 老版本、网络异常）→ 回退旧配额
                            println!("[WARN] 积分接口异常: {}，回退旧配额展示", err_msg);
                            try_usage_with_token_as_fallback(token).await?
                        }
                    }
                }
            } else {
                // 国际版 GLOBAL：直接返回 is_credits_billing=false，指示前端走旧 UsageSummary
                try_usage_with_token_as_fallback(token).await?
            }
        } else if !account.cookies.is_empty() {
            // 没 Token 但有 Cookies：取 Token 再走同样分支
            let mut cookie_client = TraeApiClient::new(&account.cookies)?;
            let token_result = cookie_client.get_user_token().await?;
            // 顺手更新一下 Account 中存储的 Token
            if let Some(acc) = self.store.accounts.iter_mut().find(|a| a.id == account_id) {
                acc.jwt_token = Some(token_result.token.clone());
                acc.token_expired_at = Some(token_result.expired_at.clone());
                acc.token_shared_with_client = false; // 新 token 为 TraeJumper 独占
            }
            self.save_store()?;

            if is_cn {
                match try_credits_with_token(&token_result.token).await {
                    Ok(c) => c,
                    Err(_) => try_usage_with_token_as_fallback(&token_result.token).await?,
                }
            } else {
                try_usage_with_token_as_fallback(&token_result.token).await?
            }
        } else {
            return Err(anyhow!("账号没有有效的 Token 或 Cookies"));
        };

        // 同步 plan_type（保留在 Account 主字段里，账号卡等继续用）
        if let Some(acc) = self.store.accounts.iter_mut().find(|a| a.id == account_id) {
            if !summary.plan_name.trim().is_empty() {
                acc.plan_type = summary.plan_name.clone();
            }
            acc.updated_at = chrono::Utc::now().timestamp();
        }
        self.save_store()?;

        Ok(summary)
    }

    /// 检查指定账号是否是客户端当前活跃的账号
    /// 用于避免对活跃账号调用 GetUserToken（会使服务端踢掉客户端正在使用的 token）
    pub(super) fn is_active_client_account(&self, account_id: &str) -> bool {
        // 扫描所有已安装客户端：账号在任一客户端真实活跃即返回 true
        // （不能只看当前目标客户端，否则非目标客户端活跃的账号会被误续签踢掉）
        for v in crate::trae_app::all_variants() {
            if !crate::trae_app::is_variant_installed(v) {
                continue;
            }
            if let Ok(login) = self.read_client_login(Some(v)) {
                if !login.user_id.is_empty()
                    && self.is_active_client_account_inner(account_id, &login, true)
                {
                    return true;
                }
            }
        }
        false
    }

    /// 判断客户端会话是否"真实存活"（usertag 匹配 + token 未自然过期）
    ///
    /// 单活跃 Token 语义下的三种状态：
    /// 1. token 有效（exp 在未来）→ 客户端登录中（token 由 switch_account 写入或同步）
    /// 2. token 已 401 但 now < exp → token 被服务端作废 = 客户端刚刷新了登录态
    ///    （客户端会提前刷新、不让自己的 token 过期）→ 客户端持有唯一有效会话，必须"让位"
    /// 3. token 已自然过期（now >= exp）→ 客户端若活着早就刷新了 → 双方 token 已同时死亡
    ///    （典型场景：Mac 睡眠期间 8h 有效期耗尽，唤醒后客户端也提示"登录已失效"）
    ///    → 不再让位，恢复 TraeJumper 侧 cookies 续签，避免"双方都死、谁也不救"的死锁
    pub(super) fn is_active_client_account_inner(&self, account_id: &str, login: &ClientLogin, verbose: bool) -> bool {
        if login.user_id.is_empty() {
            return false;
        }

        // 找到账号库中对应账号的 user_id
        let acc_user_id = self
            .store
            .accounts
            .iter()
            .find(|a| a.id == account_id)
            .map(|a| a.user_id.clone())
            .unwrap_or_default();

        let matched = !acc_user_id.is_empty() && acc_user_id == login.user_id;

        if verbose {
            println!(
                "[DEBUG active_check] account={} acc_user_id={} client_user_id={} matched={} token_available={}",
                account_id, acc_user_id, login.user_id, matched, login.token.is_some()
            );
        }
        if !matched {
            return false;
        }

        // token 归属检查：usertag 只记录"客户端最后登录的账号"，登出后仍残留（脏数据），
        // 不能单独作为"客户端登录中"的依据。只有 token 确实共享给客户端（switch_account
        // 写入 / 客户端同步 / 401 被顶死仲裁恢复）时，客户端才可能持有此会话；
        // TraeJumper 自己续签/导入的独占 token（shared=false）→ 客户端不持有 → 不让位
        let shared = self
            .store
            .accounts
            .iter()
            .find(|a| a.id == account_id)
            .map(|a| a.token_shared_with_client)
            .unwrap_or(false);
        if !shared {
            if verbose {
                println!(
                    "[DEBUG active_check] account={} usertag 匹配但 token 为 TraeJumper 独占（未共享给客户端），不视为客户端登录中",
                    account_id
                );
            }
            return false;
        }

        // token 自然过期检测：exp 已过 1 小时仍无人续签 → 客户端会话已死，不再让位
        // （1 小时宽限期：客户端在线时会提前刷新登录态、不让自己的 token 过期，
        //   TraeJumper 通常在 exp 前后发现 401 而正确让位；而睡眠/关机场景下
        //   双方 token 同时过期，唤醒时早已超出宽限期，据此区分两种状态）
        const CLIENT_DEAD_GRACE_SECS: i64 = 3600;
        if let Some(acc) = self.store.accounts.iter().find(|a| a.id == account_id) {
            if let Some(exp_str) = acc.token_expired_at.as_deref() {
                if let Some(exp) = chrono::DateTime::parse_from_rfc3339(exp_str).ok() {
                    if exp.timestamp() + CLIENT_DEAD_GRACE_SECS <= chrono::Utc::now().timestamp() {
                        if verbose {
                            let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
                            println!(
                                "[{}] [DEBUG active_check] account={} token 已自然过期（exp={}）且客户端未刷新登录态，判定客户端会话已失效，恢复 TraeJumper 续签能力",
                                now, account_id, exp_str
                            );
                        }
                        return false;
                    }
                }
            }
        }

        true
    }

    /// 401 归属仲裁：token 记录的 exp 尚未到期却返回 401，说明被其他端登录顶掉。
    /// 若任一已安装客户端 usertag 匹配该账号 → 大概率是客户端刷新了登录态（客户端持有唯一有效
    /// 会话）→ 恢复 shared 标志并让位（不续签，避免顶回客户端），返回 true。
    /// 其余情况走统一判断（token 归属 + 自然过期死亡检测）。
    /// 仅在拿到 401 后调用（调用方确知 token 已死）。
    fn arbitrate_401_and_check_active(&mut self, account_id: &str) -> bool {
        let now_ts = chrono::Utc::now().timestamp();

        // 扫描所有已安装客户端的登录态
        let logins: Vec<ClientLogin> = crate::trae_app::all_variants()
            .iter()
            .filter(|v| crate::trae_app::is_variant_installed(v))
            .filter_map(|v| {
                self.read_client_login(Some(v))
                    .ok()
                    .filter(|l| !l.user_id.is_empty())
            })
            .collect();

        for login in &logins {
            // usertag 匹配 + 记录的 exp 尚未到期 → token 是被顶死的（自然过期不可能早于 exp）
            let matched_and_preempted = self
                .store
                .accounts
                .iter()
                .find(|a| a.id == account_id)
                .map(|a| {
                    !a.user_id.is_empty()
                        && a.user_id == login.user_id
                        && a.token_expired_at
                            .as_deref()
                            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                            .map(|t| t.timestamp() > now_ts)
                            .unwrap_or(false)
                })
                .unwrap_or(false);
            if matched_and_preempted {
                let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
                let mut changed = false;
                if let Some(acc) = self.store.accounts.iter_mut().find(|a| a.id == account_id) {
                    if !acc.token_shared_with_client {
                        acc.token_shared_with_client = true;
                        changed = true;
                    }
                }
                if changed {
                    if self.save_store().is_err() {
                        println!("[WARN active_check] shared 标志保存失败（不影响本次判定）");
                    }
                    println!(
                        "[{}] [INFO active_check] account={} token 在有效期内被顶死（401），判定客户端刷新了登录态，恢复让位模式",
                        now, account_id
                    );
                }
                return true; // 让位：客户端刚刷新，持有唯一有效会话
            }
        }

        // 自然过期或非客户端账号：走统一判断（含 token 归属 + 死亡宽限检测）
        for login in &logins {
            if self.is_active_client_account_inner(account_id, login, true) {
                return true;
            }
        }
        false
    }

    /// 获取使用事件
    pub async fn get_usage_events(
        &mut self,
        account_id: &str,
        start_time: i64,
        end_time: i64,
        page_num: i32,
        page_size: i32,
    ) -> Result<UsageQueryResponse> {
        let account = self
            .store
            .accounts
            .iter()
            .find(|a| a.id == account_id)
            .ok_or_else(|| anyhow!("账号不存在"))?
            .clone();

        // 根据账号类型选择不同的方式调用 API
        if let Some(token) = &account.jwt_token {
            // 优先使用 Token
            let client = TraeApiClient::new_with_token(token)?;
            match client.query_usage(start_time, end_time, page_size, page_num).await {
                Ok(response) => Ok(response),
                Err(e) => {
                    let error_msg = e.to_string();
                    // 如果是 401 错误且有 Cookies，归属仲裁后决定是否刷新 Token
                    if error_msg.contains("401") && !account.cookies.is_empty()
                        && !self.arbitrate_401_and_check_active(&account_id) {
                        println!("[INFO] Token 已过期，尝试使用 Cookies 刷新...");
                        // 使用 Cookies 刷新 Token
                        let mut cookie_client = TraeApiClient::new(&account.cookies)?;
                        let token_result = cookie_client.get_user_token().await?;

                        // 更新存储的 Token
                        if let Some(acc) = self.store.accounts.iter_mut().find(|a| a.id == account_id) {
                            acc.jwt_token = Some(token_result.token.clone());
                            acc.token_expired_at = Some(token_result.expired_at.clone());
                            acc.token_shared_with_client = false; // 新 token 为 TraeJumper 独占
                        }
                        self.save_store()?;

                        // 使用新 Token 重新查询
                        let new_client = TraeApiClient::new_with_token(&token_result.token)?;
                        new_client.query_usage(start_time, end_time, page_size, page_num).await
                    } else if error_msg.contains("401") {
                        self.mark_account_token_expired(&account_id)?;
                        Err(anyhow!("Token 已过期，请更新 Token 或 Cookies"))
                    } else {
                        Err(e)
                    }
                }
            }
        } else if !account.cookies.is_empty() {
            // 使用 Cookies
            let mut client = TraeApiClient::new(&account.cookies)?;
            // 先获取 token
            client.get_user_token().await?;
            client.query_usage(start_time, end_time, page_size, page_num).await
        } else {
            Err(anyhow!("账号没有有效的 Token 或 Cookies"))
        }
    }

}
