//! Token 生命周期：cookies/OAuth 续签、写回客户端、批量续签调度。

use super::*;

impl AccountManager {
    /// （内部）刷新指定账号 Token，成功返回新 token；失败返回 None，不抛错方便降级
    pub(super) async fn refresh_token_inner(&mut self, account_id: &str) -> Option<String> {
        // 跳过客户端活跃账号，避免 GetUserToken 使客户端 token 失效
        if self.is_active_client_account(account_id) {
            println!("[INFO] 跳过活跃客户端账号 {} 的 cookies 续签（避免客户端登出）", account_id);
            return None;
        }
        match self.refresh_token(account_id).await {
            Ok(()) => {
                self.store
                    .accounts
                    .iter()
                    .find(|a| a.id == account_id)
                    .and_then(|a| a.jwt_token.clone())
            }
            Err(e) => {
                println!("[WARN] refresh_token_inner 失败: {}", e);
                None
            }
        }
    }

    /// 刷新账号 Token（TJ 侧独占模式，不回写客户端）
    ///
    /// 入口 guard：拒绝对活跃客户端账号执行 cookies 续签，
    /// 因为 GetUserToken 会使服务端签发新 token 并使旧 token 失效，
    /// 导致客户端正在使用的 token 被踢掉。
    ///
    /// 注意：TJ 注入过的账号应走 refresh_token_and_write_client（续签+回写+kill/relaunch），
    /// 但 switch_account 切号前会先调本函数"拿有效 token"再自行注入——
    /// 这种场景是合法的，本函数不拦截 injected 状态。
    /// refresh_all_tokens 的 step 3 分流逻辑已确保 injected 账号不会误入本函数。
    pub async fn refresh_token(&mut self, account_id: &str) -> Result<()> {
        let account = self
            .store
            .accounts
            .iter()
            .find(|a| a.id == account_id)
            .ok_or_else(|| anyhow!("账号不存在"))?
            .clone();

        // === OAuth Native 模式：走 ExchangeToken Refresh Flow ===
        // 覆盖两种情况：
        //  a) oauth_enabled=true（TJ OAuth Native 登录）
        //  b) has_been_injected + oauth_refresh_token 存在（TJ OAuth 注入后保存了 refresh_token）
        // ExchangeToken 不作废旧 access_token → 客户端无感，无需 kill+relaunch。
        // 注意：OAuth 账号可以绕过"客户端活跃守卫"，因为 ExchangeToken 不会互踢。
        let has_oauth_refresh = account.oauth_enabled
            || (account.has_been_injected && account.oauth_refresh_token.is_some());
        if has_oauth_refresh && account.oauth_refresh_token.is_some() {
            return self.oauth_refresh_and_write_client(account_id, &account).await;
        }

        // Guard: 客户端原生登录活跃 → 让位避免 GetUserToken 互踢（仅 cookies 模式需要）
        if self.is_active_client_account(account_id) {
            let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
            println!(
                "[{}] [WARN] refresh_token 被调用但目标是活跃客户端账号 {}，已拒绝执行（避免客户端登出）",
                now, account_id
            );
            return Err(anyhow!("活跃客户端账号不能用 cookies 续签，以免踢掉客户端登录态"));
        }

        // cookies 模式必须有 cookies
        if account.cookies.trim().is_empty() {
            return Err(anyhow!(
                "账号 {} 既无 OAuth refresh_token 也无 cookies，无法续签",
                account.name
            ));
        }

        let mut client = TraeApiClient::new(&account.cookies)?;
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let token_result = match client.get_user_token().await {
            Ok(r) => r,
            Err(e) => {
                // "get session empty" 说明 cookies 对应的服务端登录会话已被注销，
                // 常见原因：登录窗口中切换/退出了该账号（登录新账号会注销上一个账号的会话）
                if format!("{}", &e).contains("get session empty") {
                    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
                    println!(
                        "[{}] [WARN refresh_token] 账号 {} 的服务端登录会话已失效（cookies 对应的 session 已被服务端注销，常见原因：登录窗口中切换/退出了该账号）。需重新通过浏览器登录该账号后才能续签",
                        now, account.name
                    );
                }
                return Err(e);
            }
        };
        let now2 = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        if let Some(acc) = self.store.accounts.iter_mut().find(|a| a.id == account_id) {
            acc.jwt_token = Some(token_result.token);
            acc.token_expired_at = Some(token_result.expired_at);
            acc.updated_at = chrono::Utc::now().timestamp();
            // 新 token 由 TraeJumper 独占（未写入客户端），客户端不再持有此会话
            acc.token_shared_with_client = false;
            // 回写服务端刷新后的 cookies（含续签的 sessionid/sid_guard 等），
            // 避免旧 cookie 快照随服务端 session 过期而失效。
            let refreshed_cookies = client.cookies().to_string();
            if !refreshed_cookies.is_empty() && refreshed_cookies != acc.cookies {
                acc.cookies = refreshed_cookies;
            }
            // 标记上次续签时间，用于 12 小时 session 保活节流判断
            acc.last_cookie_renewal_at = Some(chrono::Utc::now().timestamp());
        }

        self.save_store()?;
        Ok(())
    }

    /// 续签客户端活跃账号的 Token 并立即写入客户端 storage.json。
    ///
    /// 背景：TraeJumper 写入客户端的是 8 小时短期 JWT，而客户端的
    /// `JWT token refresh is disabled`，不会自动刷新该 token。
    /// 因此必须由 TraeJumper 在 token 过期前续签并重新写入客户端，
    /// 否则 8 小时后客户端会因 token 过期而登出（"登录已失效"）。
    ///
    /// 与 refresh_token 的区别：本方法跳过活跃账号守卫，且续签成功后
    /// 立即写入客户端 storage.json 并保持 token_shared_with_client=true。
    pub(crate) async fn refresh_token_and_write_client(&mut self, account_id: &str) -> Result<()> {
        let account = self.store.accounts.iter()
            .find(|a| a.id == account_id)
            .ok_or_else(|| anyhow!("账号不存在"))?
            .clone();

        // === OAuth Native 模式：走 ExchangeToken Refresh Flow ===
        // 覆盖两种情况：
        //  a) oauth_enabled=true（TJ OAuth Native 登录）
        //  b) has_been_injected + oauth_refresh_token 存在（TJ OAuth 注入后保存了 refresh_token）
        // ExchangeToken 不废废旧 access_token → 无需 kill+relaunch，客户端无感
        let has_oauth_refresh = account.oauth_enabled
            || (account.has_been_injected && account.oauth_refresh_token.is_some());
        if has_oauth_refresh && account.oauth_refresh_token.is_some() {
            return self.oauth_refresh_and_write_client(account_id, &account).await;
        }

        if account.cookies.trim().is_empty() {
            return Err(anyhow!("账号 {} 无 cookies，无法续签", account.name));
        }

        // 找到该账号活跃的【所有】客户端变体。
        // 关键：GetUserToken 会使服务端该账号的所有旧 token 失效，
        // 若账号同时在多个客户端活跃（如 TraeCode CN + TraeWork CN），
        // 只写入其中一个会导致其余客户端持旧 token 被踢掉（401 登出）。
        // 因此必须把新 token 写入所有活跃客户端并逐一重启。
        let active_variants: Vec<crate::trae_app::TraeAppVariant> = crate::trae_app::all_variants()
            .iter()
            .filter(|v| crate::trae_app::is_variant_installed(v))
            .filter_map(|v| {
                self.read_client_login(Some(v))
                    .ok()
                    .filter(|l| !l.user_id.is_empty() && l.user_id == account.user_id)
                    .map(|_| v.clone())
            })
            .collect();

        // === cloudide 缺失回退 ===
        // 正常情况：active_variants 从 read_client_login(cloudide) 匹配。
        // 异常情况：TJ 注入过的账号 has_been_injected=true，但客户端
        // clearUserInfo 删除了 cloudide → 所有 read_client_login 返回 Err → active_variants 空。
        // 此时回退为"写入所有已安装变体"——我们要重新注入 cloudide。
        let active_variants = if active_variants.is_empty() && account.has_been_injected {
            let fallback = crate::trae_app::all_variants()
                .iter()
                .filter(|v| crate::trae_app::is_variant_installed(v))
                .cloned()
                .collect::<Vec<_>>();
            if !fallback.is_empty() {
                let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
                println!(
                    "[{}] [WARN refresh_client] 账号 {} cloudide 票据已被客户端清除，active_variants 空，回退写入所有已安装变体",
                    now, account.name
                );
            }
            fallback
        } else {
            active_variants
        };

        if active_variants.is_empty() {
            return Err(anyhow!("账号 {} 未在任何客户端活跃，且未标记为已注入", account.name));
        }

        // 续签（直接调用底层 API，跳过活跃账号守卫）
        let mut client = TraeApiClient::new(&account.cookies)?;
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let variant_names: Vec<&str> = active_variants.iter().map(|v| v.display_name).collect();
        println!(
            "[{}] [INFO refresh_client] 账号 {} 在 [{}] 活跃，token 即将过期，续签并写入所有客户端",
            now, account.name, variant_names.join(", ")
        );
        let token_result = client.get_user_token().await?;
        println!(
            "[{}] [INFO refresh_client] 账号 {} GetUserToken 成功, expired_at={}",
            now, account.name, token_result.expired_at
        );

        // 更新本地账号
        if let Some(acc) = self.store.accounts.iter_mut().find(|a| a.id == account_id) {
            acc.jwt_token = Some(token_result.token.clone());
            acc.token_expired_at = Some(token_result.expired_at.clone());
            acc.updated_at = chrono::Utc::now().timestamp();
            acc.token_shared_with_client = true; // 续签后立即写入客户端，保持共享
            acc.has_been_injected = true; // 曾被切号注入/写回（持久化）
            let refreshed_cookies = client.cookies().to_string();
            if !refreshed_cookies.is_empty() && refreshed_cookies != acc.cookies {
                acc.cookies = refreshed_cookies;
            }
            acc.last_cookie_renewal_at = Some(chrono::Utc::now().timestamp());
        }
        self.save_store()?;

        // 写入所有活跃客户端的 storage.json
        let login_info = crate::machine::TraeLoginInfo {
            token: token_result.token.clone(),
            refresh_token: None,
            user_id: account.user_id.clone(),
            email: account.email.clone(),
            username: account.name.clone(),
            avatar_url: account.avatar_url.clone(),
            host: String::new(),
            region: if account.region.is_empty() { "SG".to_string() } else { account.region.clone() },
            token_expired_at: Some(token_result.expired_at.clone()),
        };
        for variant in &active_variants {
            if let Err(e) = crate::machine::write_trae_login_info_for_variant(&login_info, Some(variant)) {
                eprintln!(
                    "[WARN refresh_client] 写入 {} storage.json 失败: {}",
                    variant.display_name, e
                );
            } else {
                println!("[INFO refresh_client] 已将新 token 写入 {} storage.json", variant.display_name);
            }
        }

        // GetUserToken 签发新 token 时服务端立即作废旧 token——
        // 运行中的客户端内存里持有的仍是旧 token，会在下次 API 调用时 401 → clearUserInfo → "登录已失效"。
        // 因此必须 kill+relaunch 让客户端重新从 storage.json 加载新 token。
        // 代价是用户正在进行的会话会短暂中断（约 1-2 秒），但远好于硬登出需要重新登录。
        // 可通过 CheckinConfig.auto_restart_after_refresh + per-account override 关闭。
        let should_restart = self.should_restart_after_refresh(account_id);
        if should_restart {
            use std::time::Duration;
            for variant in &active_variants {
                if crate::machine::is_trae_running_for_variant(variant) {
                    let _ = crate::machine::kill_trae_for_variant(variant);
                    tokio::time::sleep(Duration::from_millis(800)).await;
                    let _ = crate::machine::launch_trae_for_variant(variant);
                    println!(
                        "[INFO refresh_client] 已重启 {} 加载新 token",
                        variant.display_name
                    );
                }
            }
        } else {
            println!(
                "[INFO refresh_client] 自动重启已关闭，仅 TJ 侧刷新 token（storage.json 已更新）"
            );
        }

        Ok(())
    }

    /// OAuth 模式手动续签 + 写客户端（ExchangeToken Refresh Flow）
    /// 独立于 cookies 模式，不依赖 cookies。
    async fn oauth_refresh_and_write_client(
        &mut self,
        account_id: &str,
        account: &Account,
    ) -> Result<()> {
        let refresh_token = account.oauth_refresh_token.as_ref()
            .ok_or_else(|| anyhow!("账号未配置 oauth_refresh_token"))?;
        let private_key = account.oauth_private_key_pem.as_ref()
            .ok_or_else(|| anyhow!("账号未配置 oauth_private_key_pem"))?;
        let public_key = account.oauth_public_key_pem.as_ref()
            .ok_or_else(|| anyhow!("账号未配置 oauth_public_key_pem"))?;
        let device_id = account.oauth_device_id.as_deref().unwrap_or("trae-jumper-device");
        let machine_id = account.oauth_machine_id.as_deref().unwrap_or("trae-jumper-machine");
        let variant = crate::trae_app::current().clone();
        let api_host = account.oauth_host.as_deref().unwrap_or(variant.api_host);

        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        println!(
            "[{}] [INFO refresh_client-oauth] 手动续签 OAuth 账号 {}",
            now, account.name
        );

        let token_result = crate::oauth::exchange_refresh_token(
            &variant, api_host, refresh_token, device_id, machine_id, public_key, private_key,
        ).await.map_err(|e| anyhow!("ExchangeToken 失败: {}", e))?;

        // 更新本地
        {
            let acc = self.store.accounts.iter_mut()
                .find(|a| a.id == account_id).unwrap();
            acc.jwt_token = Some(token_result.access_token.clone());
            acc.token_expired_at = token_result.expired_at();
            if !token_result.refresh_token.is_empty() && token_result.refresh_token != *refresh_token {
                acc.oauth_refresh_token = Some(token_result.refresh_token.clone());
                acc.oauth_refresh_expired_at = token_result.refresh_expired_at();
            }
            acc.updated_at = chrono::Utc::now().timestamp();
            acc.token_shared_with_client = true;
            acc.has_been_injected = true;
        }
        self.save_store()?;

        // 写客户端（所有活跃变体）
        let active_variants: Vec<crate::trae_app::TraeAppVariant> = crate::trae_app::all_variants()
            .iter()
            .filter(|v| crate::trae_app::is_variant_installed(v))
            .filter_map(|v| {
                self.read_client_login(Some(v))
                    .ok()
                    .filter(|l| !l.user_id.is_empty() && l.user_id == account.user_id)
                    .map(|_| v.clone())
            })
            .collect();
        if active_variants.is_empty() {
            println!("[INFO refresh_client-oauth] 账号 {} 未在任何客户端活跃，仅更新本地", account.name);
            return Ok(());
        }

        let login_info = crate::machine::TraeLoginInfo {
            token: token_result.access_token.clone(),
            refresh_token: Some(token_result.refresh_token.clone()),
            user_id: account.user_id.clone(),
            email: account.email.clone(),
            username: account.name.clone(),
            avatar_url: account.avatar_url.clone(),
            host: token_result.host.clone().unwrap_or_default(),
            region: account.region.clone(),
            token_expired_at: token_result.expired_at(),
        };
        let should_restart = self.should_restart_after_refresh(account_id);
        for v in &active_variants {
            let _ = crate::machine::write_trae_login_info_for_variant(&login_info, Some(v));
            println!(
                "[INFO refresh_client-oauth] 已写入 {} storage.json",
                v.display_name
            );
            if should_restart {
                // ⚠️ Trae 客户端 verifyStrictTokenInBackground 会对比 storage.json 和内存 token
                // 不一致 → clearUserInfo 登出！所以需要 kill+relaunch 让客户端加载新 token
                if crate::machine::is_trae_running_for_variant(v) {
                    use std::time::Duration;
                    let _ = crate::machine::kill_trae_for_variant(v);
                    tokio::time::sleep(Duration::from_millis(800)).await;
                    let _ = crate::machine::launch_trae_for_variant(v);
                    println!(
                        "[INFO refresh_client-oauth] 已重启 {} 加载新 OAuth token",
                        v.display_name
                    );
                }
            } else {
                println!(
                    "[INFO refresh_client-oauth] 自动重启已关闭，仅 TJ 侧刷新 token（verifyStrict 可能检测到不一致登出）"
                );
            }
        }
        Ok(())
    }

    /// 更新账号 Token
    pub async fn update_account_token(&mut self, account_id: &str, token: String) -> Result<UsageSummary> {
        let client = TraeApiClient::new_with_token(&token)?;

        // 验证 Token 并获取用户信息
        let user_info = client.get_user_info_by_token().await?;

        // 查找账号
        let acc = self.store.accounts.iter_mut()
            .find(|a| a.id == account_id)
            .ok_or_else(|| anyhow!("账号不存在"))?;

        // 确保是同一个用户
        if acc.user_id != user_info.user_id {
            return Err(anyhow!("Token 对应的用户与当前账号不匹配"));
        }

        // 更新 Token，并清除过期标记（若有），避免前端仍显示"已过期"
        acc.jwt_token = Some(token.clone());
        acc.updated_at = chrono::Utc::now().timestamp();
        acc.token_expired_at = None;
        acc.token_shared_with_client = false; // 手动更新的 token 为 TraeJumper 独占

        // 获取最新使用量
        let summary = client.get_usage_summary_by_token().await?;
        acc.plan_type = summary.plan_type.clone();

        self.save_store()?;
        Ok(summary)
    }

    /// 更新账号 Token，同时更新 Cookies（用于浏览器登录更新场景）。
    ///
    /// 浏览器登录时从 webview 读取完整 cookies（含 HttpOnly），保存后可用于
    /// 后续 Token 自动续签（refresh_token 调用 GetUserToken 时需要完整认证 cookies）。
    pub async fn update_account_token_with_cookies(
        &mut self,
        account_id: &str,
        token: String,
        cookies: Option<String>,
    ) -> Result<UsageSummary> {
        let client = TraeApiClient::new_with_token(&token)?;

        // 验证 Token 并获取用户信息
        let user_info = client.get_user_info_by_token().await?;

        // 查找账号
        let acc = self.store.accounts.iter_mut()
            .find(|a| a.id == account_id)
            .ok_or_else(|| anyhow!("账号不存在"))?;

        // 确保是同一个用户
        if acc.user_id != user_info.user_id {
            println!(
                "[DEBUG update] user_id 不匹配: 账号库={} 登录窗口={} email={:?}",
                acc.user_id, user_info.user_id, user_info.email
            );
            let logged = user_info.email.clone().unwrap_or_else(|| user_info.user_id.clone());
            let target = if acc.email.is_empty() { acc.id.clone() } else { acc.email.clone() };
            return Err(anyhow!(
                "登录的账号「{}」与要更新的账号「{}」不一致，请在登录窗口中登录正确的账号",
                logged,
                target
            ));
        }
        println!(
            "[DEBUG update] user_id 匹配: {} email={:?}",
            acc.user_id, user_info.email
        );

        // 更新 Token、Cookies，并清除过期标记
        acc.jwt_token = Some(token.clone());
        if let Some(c) = cookies {
            if !c.is_empty() {
                acc.cookies = c;
            }
        }
        acc.updated_at = chrono::Utc::now().timestamp();
        acc.token_shared_with_client = false; // 手动更新的 token 为 TraeJumper 独占
        acc.token_expired_at = None;
        // 浏览器登录重新捕获了 cookies，刷新 session 保活时间戳
        acc.last_cookie_renewal_at = Some(chrono::Utc::now().timestamp());

        // 获取最新使用量
        let summary = client.get_usage_summary_by_token().await?;
        acc.plan_type = summary.plan_type.clone();

        self.save_store()?;
        Ok(summary)
    }

    /// 更新账号 Cookies
    pub async fn update_cookies(&mut self, account_id: &str, cookies: String) -> Result<()> {
        // 验证新 cookies 是否有效
        let mut client = TraeApiClient::new(&cookies)?;
        let token_result = client.get_user_token().await?;

        if let Some(acc) = self.store.accounts.iter_mut().find(|a| a.id == account_id) {
            // 确保是同一个用户
            if acc.user_id != token_result.user_id {
                return Err(anyhow!("Cookies 对应的用户与当前账号不匹配"));
            }

            acc.cookies = cookies;
            acc.jwt_token = Some(token_result.token);
            acc.token_expired_at = Some(token_result.expired_at);
            acc.updated_at = chrono::Utc::now().timestamp();
            acc.token_shared_with_client = false; // 新 token 为 TraeJumper 独占
        } else {
            return Err(anyhow!("账号不存在"));
        }

        self.save_store()?;
        Ok(())
    }

    /// 将指定账号标记为"Token 已过期"。
    ///
    /// 当 API 返回 401 且无法用 Cookies 刷新 token 时调用，把 token_expired_at 置为当前时间并持久化，
    /// 前端账号列表的状态标签据此显示"已过期"，不再一直显示"正常"。
    pub(super) fn mark_account_token_expired(&mut self, account_id: &str) -> Result<()> {
        // 守卫：客户端正在登录的账号不标记"已过期"。
        // 单活跃 Token 策略下，客户端持有该账号当前唯一有效会话，TraeJumper 侧
        // Token 失效属预期让位行为（为避免 cookies 续签踢掉客户端），
        // 此时账号本身是健康的，标记过期会误导用户以为需要重新登录。
        if self.is_active_client_account(account_id) {
            let name = self.store.accounts.iter()
                .find(|a| a.id == account_id)
                .map(|a| a.name.clone())
                .unwrap_or_else(|| account_id.to_string());
            println!(
                "[INFO] 账号 {} 正由 Trae 客户端登录持有有效会话，不标记过期（单活跃 Token 预期让位行为）",
                name
            );
            return Ok(());
        }
        if let Some(acc) = self.store.accounts.iter_mut().find(|a| a.id == account_id) {
            acc.token_expired_at = Some(chrono::Utc::now().to_rfc3339());
            acc.updated_at = chrono::Utc::now().timestamp();
        }
        self.save_store()
    }

    /// 判断账号的 Token 是否即将过期（< 1小时）或已过期
    fn is_token_expiring_soon(account: &Account) -> bool {
        Self::is_token_expiring_within_hours(account, 1)
    }

    /// 判断 token 是否在指定小时内过期
    fn is_token_expiring_within_hours(account: &Account, hours: i64) -> bool {
        match &account.token_expired_at {
            None => true, // 无过期时间信息，需要刷新
            Some(expired_at) => {
                match chrono::DateTime::parse_from_rfc3339(expired_at) {
                    Ok(expiry) => {
                        let now = chrono::Utc::now();
                        let threshold = chrono::Duration::hours(hours);
                        expiry.with_timezone(&chrono::Utc) < now + threshold
                    }
                    Err(_) => {
                        // 尝试解析为时间戳（秒）
                        if let Ok(ts) = expired_at.parse::<i64>() {
                            let now = chrono::Utc::now().timestamp();
                            ts < now + hours * 3600
                        } else {
                            true // 无法解析，需要刷新
                        }
                    }
                }
            }
        }
    }

    /// 批量刷新所有即将过期的 Token
    pub async fn refresh_all_tokens(&mut self) -> Result<Vec<String>> {
        let mut refreshed = Vec::new();

        // 1) 优先从客户端同步活跃账号的最新 Token（只读不写 storage.json，安全）
        //    活跃账号的 token 由客户端自动续签，我们只同步读取
        if let Ok(Some(id)) = self.sync_active_client_token() {
            refreshed.push(id);
        }

        // 2) 收集所有已安装 Trae 客户端的登录态（用于跳过，避免 cookies 续签踢掉客户端 token）。
        //    必须独立读取客户端登录态，不能依赖 sync_active_client_token 的返回值——
        //    当客户端 token 未变化时 sync_active_client_token 返回 Ok(None)，
        //    会导致活跃账号漏判而被错误地用 cookies 续签。
        //    判断与续签让位逻辑共用 is_active_client_account_inner（含客户端死亡检测）：
        //    token 自然过期超宽限期后（如 Mac 睡眠期间双方 token 同时过期），
        //    客户端会话已失效，不再让位，恢复该账号的自动续签。
        //    多客户端场景：账号在任一客户端活跃都应跳过（避免踢掉该客户端）。
        let active_client_logins: Vec<ClientLogin> = crate::trae_app::all_variants()
            .iter()
            .filter(|v| crate::trae_app::is_variant_installed(v))
            .filter_map(|v| {
                self.read_client_login(Some(v))
                    .ok()
                    .filter(|l| !l.user_id.is_empty())
            })
            .collect();

        // 2.5) 对客户端活跃的账号，token 快过期时续签并重新写入客户端。
        //      背景：TraeJumper 写入客户端的是 8 小时短期 JWT，客户端
        //      `JWT token refresh is disabled` 不会自动刷新，必须由 TraeJumper
        //      在过期前续签并写入，否则客户端会因 token 过期登出。
        //      续签触发条件：token 4 小时内过期（留出充足时间写入客户端，覆盖客户端
        //      verifyStrictTokenInBackground 每 10min 检查周期）。
        //      注意：这里不用 is_active_client_account_inner（含 1h 死亡检测），
        //      因为 token 过期后正是需要续签+写入的时机。
        for a in &self.store.accounts.clone() {
            let has_cookies = !a.cookies.is_empty();
            if !has_cookies {
                continue;
            }
            // 账号在任一已安装客户端 usertag 匹配 且 token 已共享给客户端
            let client_matched = active_client_logins
                .iter()
                .any(|l| l.user_id == a.user_id);
            let shared = a.token_shared_with_client;
            if !client_matched || !shared {
                continue;
            }
            // === self_managed 客户端让位判定 ===
            //
            // 新版客户端（storage 含 icube-dc:* 设备密钥）对"纯原生 OAuth 登录"的账号
            // （source=refresh_token，15d 有效期）自行维护完整票据生命周期，TJ 不应插手。
            //
            // 但存在一个关键矛盾场景：TJ 已通过 switch_account 把 GetUserToken 的 8h
            // JWT 写入了 icube.cloudide（token_shared_with_client=true），与此同时客户端
            // storage 仍有 icube-dc:* 设备密钥 → self_managed=true。此时若一刀切让位，
            // TJ 注入的 cloudide token 将再也没人续签 → 自然过期 → 客户端 verifyStrictTokenInBackground
            // 校验 cloudide 过期 → clearUserInfo 登出（实测 2026-10-17 Ye 账号正是因此登出）。
            //
            // 因此让位规则修正为：
            //   self_managed_active && !token_shared_with_client → 让位（纯自管，TJ 未注入）
            //   self_managed_active && token_shared_with_client  → 不让位！TJ 必须续+写，
            //     维持自己注入的 cloudide token 不死
            //
            // 非 self_managed 客户端：正常走续+写（不受此分支影响）。
            let self_managed_active = active_client_logins
                .iter()
                .any(|l| l.self_managed && l.user_id == a.user_id);
            let should_yield_to_self_managed = self_managed_active && !shared;
            if should_yield_to_self_managed {
                continue;
            }
            // 宽限期扩大到 4h：给 self_managed 客户端 verifyStrictTokenInBackground
            // （每 10min 检查）足够缓冲，避免 cloudide 因过期被客户端清掉
            let expiring_soon = Self::is_token_expiring_within_hours(a, 4);
            if !expiring_soon {
                continue;
            }
            let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
            println!(
                "[{}] [INFO refresh] 客户端活跃账号 {}({}) token 即将过期，续签并写入客户端",
                now, a.name, a.id
            );
            match self.refresh_token_and_write_client(&a.id).await {
                Ok(_) => {
                    refreshed.push(a.id.clone());
                }
                Err(e) => {
                    let now2 = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
                    eprintln!(
                        "[{}] [WARN] 客户端活跃账号 {}({}) 续签写入失败: {}",
                        now2, a.name, a.id, e
                    );
                }
            }
        }

        // 3) 对有 cookies 的非活跃账号，用 cookies 续签
        //    关键：跳过客户端当前活跃的账号——因为调 GetUserToken 会使服务端
        //    签发新 token 并使旧 token 失效，导致客户端正在使用的 token 被踢掉
        //
        //    续签触发条件（满足其一即可）：
        //    a) token 即将过期（1 小时内，含已过期）→ 必须续签避免彻底失效
        //    b) 距上次 cookies 续签超过 12 小时 → 保持 web session 活跃，
        //       防止服务端因长时间无请求而清理 session（session 一旦被清理，
        //       token 到期时就无法续签了）
        let now = chrono::Utc::now().timestamp();
        let mut candidates: Vec<(String, String, bool, bool, usize, bool, bool)> = Vec::new();
        let mut skipped_logs: Vec<String> = Vec::new();
        for a in &self.store.accounts {
            let has_cookies = !a.cookies.is_empty();
            if !has_cookies {
                // OAuth Native / 手动导入的账号没有 cookies，不会走这个分支
                // OAuth 账号有 step 4 的 Refresh Flow 覆盖
                continue;
            }
            // 账号在任一已安装客户端"真实存活"且登录 → 让位跳过（含死亡检测）
            let is_active_client = active_client_logins
                .iter()
                .any(|l| self.is_active_client_account_inner(&a.id, l, false));
            // 任一客户端 usertag 匹配该账号（无论会话死活）——"客户端曾登录过"
            let client_matched = active_client_logins
                .iter()
                .any(|l| l.user_id == a.user_id);

            // === 让位逻辑关键修正 ===
            // 只有"客户端独立登录"(!has_been_injected && !shared) 时才让位。
            // TJ 注入过的账号(has_been_injected=true || shared=true)，不论客户端
            // 当前是否活跃，都不应跳过——TJ 必须对它在客户端的整个生命周期
            // 负全责：到期续签 + 回写客户端 + 被清除后重注入。
            let injected = a.has_been_injected || a.token_shared_with_client;
            if is_active_client && !injected {
                // 客户端原生 OAuth 登录的账号，让位避免 GetUserToken 互踢
                skipped_logs.push(format!(
                    "    {}({}) → 跳过：客户端原生登录活跃中（让位避免互踢）",
                    a.name, a.id
                ));
                continue;
            }

            // === cloudide 缺失自动重注入 ===
            // TJ 注入过的账号，如果**所有已安装变体的 cloudide 都被清除**
            // （clearUserInfo 清空 / 首次启动未注入），立即加入候选——
            // 不等 expiring_soon，直接 refresh + write + kill/relaunch。
            //
            // 注意：只在 active_client_logins 完全为空时触发——说明所有变体的
            // read_client_login 都失败或 userId 为空（被 clearUserInfo 清空）。
            // 如果至少有一个变体有 cloudide（不管是谁的 userId），说明客户端
            // 正常持有某个账号的会话，inject 但 userId 不匹配是"正常切号后
            // 旧账号不在客户端活跃"的状态，不应强制覆盖。
            let all_variants_cloudide_gone = active_client_logins.is_empty();
            let no_variant_has_cloudide = injected && all_variants_cloudide_gone;
            if no_variant_has_cloudide {
                let now_ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
                println!(
                    "[{}] [WARN refresh] 已注入账号 {}({}) 所有已安装变体均无 cloudide 票据（可能被 clearUserInfo 清空），自动重注入",
                    now_ts, a.name, a.id
                );
            }

            let already_expired = a
                .token_expired_at
                .as_deref()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|t| t.with_timezone(&chrono::Utc) < chrono::Utc::now())
                .unwrap_or(false);
            let expiring_soon = Self::is_token_expiring_soon(a);
            let last_renewal = a.last_cookie_renewal_at.unwrap_or(0);
            let stale_session = now - last_renewal > 12 * 3600; // 12 小时未续签
            let cookie_count = a.cookies.split(';').filter(|c| !c.trim().is_empty()).count();

            // 候选条件：过期/临期/session 过期/已注入但无任何变体有 cloudide(强制重注入)
            let needs_renewal = expiring_soon || stale_session || already_expired || no_variant_has_cloudide;
            if needs_renewal {
                candidates.push((a.id.clone(), a.name.clone(), expiring_soon, stale_session, cookie_count, injected, no_variant_has_cloudide));
            } else {
                // 暂不需要续签的账号也记一下，方便确认状态
                let remaining_hrs = a.token_expired_at
                    .as_deref()
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|t| {
                        let diff_h = (t.with_timezone(&chrono::Utc) - chrono::Utc::now()).num_seconds() / 3600;
                        format!("{}h", diff_h.max(0))
                    })
                    .unwrap_or_else(|| "?".to_string());
                skipped_logs.push(format!(
                    "    {}({}) → 暂无需续签（距过期 {}，cookies 上次续签 {}h 前）",
                    a.name, a.id, remaining_hrs, (now - last_renewal) / 3600
                ));
            }
        }

        {
            let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
            println!(
                "[{}] [INFO refresh] cookies 续签扫描：待续签 {} 个，跳过/无需 {} 个",
                now, candidates.len(), skipped_logs.len()
            );
            for line in &skipped_logs {
                println!("{}", line);
            }
        }
        for (id, name, expiring_soon, stale_session, cookie_count, injected, cloudide_gone) in &candidates {
            let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
            let reasons = [
                if *expiring_soon { Some("1h内过期") } else { None },
                if *stale_session { Some("session超12h") } else { None },
                if *cloudide_gone { Some("cloudide被清除→重注入") } else { None },
                if !*expiring_soon && !*stale_session && !*cloudide_gone { Some("已过期重试") } else { None },
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join("+");
            let action = if *injected { "续签+回写客户端" } else { "仅TJ侧续签" };
            println!(
                "[{}] [INFO refresh] 开始续签 {}({}) cookies={} 原因: {} [{}]",
                now, name, id, cookie_count, reasons, action
            );
        }

        // === 核心分流：注入过的账号必须 refresh_token_and_write_client() ===
        // 只调 refresh_token() 会：①GetUserToken 作废客户端现有 token(死票) ②shared=false
        for (id, name, _, _, _, injected, _) in candidates {
            if injected {
                // TJ 注入过的账号：续签 + 写回客户端 + kill/relaunch 让客户端重新读取
                match self.refresh_token_and_write_client(&id).await {
                    Ok(_) => {
                        let now2 = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
                        refreshed.push(id);
                    }
                    Err(e) => {
                        let now2 = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
                        eprintln!("[{}] [WARN] 续签+回写客户端失败 {}({}): {}", now2, name, id, e);
                    }
                }
            } else {
                // 未注入过的账号：TJ 侧独占续签即可
                match self.refresh_token(&id).await {
                    Ok(_) => {
                        let now2 = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
                        refreshed.push(id);
                    }
                    Err(e) => {
                        let now2 = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
                        eprintln!("[{}] [WARN] 自动刷新 Token 失败 {}({}): {}", now2, name, id, e);
                    }
                }
            }
        }

        // 4) OAuth Native 模式账号：用 ExchangeToken(Refresh Flow) 续签
        //    与 cookies 模式完全独立——OAuth 账号没有 cookies，不会被上面的逻辑覆盖到。
        //    触发条件（任一满足即续签）：
        //      a) oauth_enabled + access_token 2h 内过期
        //      b) oauth_enabled + refresh_token 7 天内过期（轮换，防止自然过期）
        //    ExchangeToken 不作废旧 access_token → 客户端无感，无需 kill+relaunch。
        let current_variant = crate::trae_app::current();
        let oauth_account_ids: Vec<String> = self.store.accounts.iter()
            .filter(|a| {
                if !a.oauth_enabled || a.oauth_refresh_token.is_none()
                    || a.oauth_private_key_pem.is_none() || a.oauth_public_key_pem.is_none()
                {
                    return false;
                }
                // 条件 a: access_token 即将过期（2h 内）
                let exp = a.token_expired_at.as_deref().unwrap_or("None");
                let will_expire = Self::is_token_expiring_within_hours(a, 2);
                println!(
                    "[DEBUG step4] OAuth 账号 {} token_expired_at={} will_expire_2h={}",
                    a.name, exp, will_expire
                );
                if will_expire { return true; }
                // 条件 b: refresh_token 即将过期（7 天内，轮换）
                if let Some(exp) = &a.oauth_refresh_expired_at {
                    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(exp) {
                        // FixedOffset → Utc 才能和 chrono::Utc::now() 相减
                        let dt_utc = dt.with_timezone(&chrono::Utc);
                        let days_left = (dt_utc - chrono::Utc::now()).num_days();
                        if days_left <= 7 { return true; }
                    }
                }
                false
            })
            .map(|a| a.id.clone())
            .collect();

        for id in oauth_account_ids {
            let a = match self.store.accounts.iter().find(|x| x.id == id) {
                Some(a) => a.clone(),
                None => continue,
            };
            let refresh_token = a.oauth_refresh_token.as_ref().unwrap();
            let private_key = a.oauth_private_key_pem.as_ref().unwrap();
            let public_key = a.oauth_public_key_pem.as_ref().unwrap();
            let device_id = a.oauth_device_id.as_deref().unwrap_or("trae-jumper-device");
            let machine_id = a.oauth_machine_id.as_deref().unwrap_or("trae-jumper-machine");
            let api_host = a.oauth_host.as_deref().unwrap_or(current_variant.api_host);

            let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
            println!(
                "[{}] [INFO refresh-oauth] OAuth 账号 {}({}) access_token 即将过期，ExchangeToken 续签 host={}",
                now, a.name, a.id, api_host
            );

            let result = crate::oauth::exchange_refresh_token(
                &current_variant,
                api_host,
                refresh_token,
                device_id,
                machine_id,
                public_key,
                private_key,
            )
            .await;

            match result {
                Ok(token_result) => {
                    // 大括号限制 mutable borrow 作用域，避免与后续 &self 调用冲突
                    {
                        let account = self.store.accounts.iter_mut()
                            .find(|x| x.id == id)
                            .unwrap();
                        account.jwt_token = Some(token_result.access_token.clone());
                        account.token_expired_at = token_result.expired_at();
                        if !token_result.refresh_token.is_empty()
                            && token_result.refresh_token != *refresh_token
                        {
                            account.oauth_refresh_token = Some(token_result.refresh_token.clone());
                            account.oauth_refresh_expired_at = token_result.refresh_expired_at();
                        }
                        account.updated_at = chrono::Utc::now().timestamp();
                        account.token_shared_with_client = true;
                        account.has_been_injected = true;
                    }
                    if let Err(e) = self.save_store() {
                        eprintln!("[WARN] OAuth 续签持久化失败: {}", e);
                    }
                    // 拿最新 account 数据（save_store 之后）
                    let account_snapshot = self.store.accounts.iter()
                        .find(|x| x.id == id)
                        .cloned()
                        .unwrap();
                    // 若该账号在某个客户端活跃，把新 token 写回客户端 storage.json
                    let active_variants: Vec<crate::trae_app::TraeAppVariant> = crate::trae_app::all_variants()
                        .iter()
                        .filter(|v| crate::trae_app::is_variant_installed(v))
                        .filter_map(|v| {
                            self.read_client_login(Some(v))
                                .ok()
                                .filter(|l| !l.user_id.is_empty() && l.user_id == account_snapshot.user_id)
                                .map(|_| v.clone())
                        })
                        .collect();
                    if !active_variants.is_empty() {
                        let login_info = crate::machine::TraeLoginInfo {
                            token: token_result.access_token.clone(),
                            refresh_token: token_result.refresh_token.clone().into(),
                            user_id: account_snapshot.user_id.clone(),
                            email: account_snapshot.email.clone(),
                            username: account_snapshot.name.clone(),
                            avatar_url: account_snapshot.avatar_url.clone(),
                            host: token_result.host.clone().unwrap_or_default(),
                            region: account_snapshot.region.clone(),
                            token_expired_at: token_result.expired_at(),
                        };
                        for variant in &active_variants {
                            match crate::machine::write_trae_login_info_for_variant(&login_info, Some(variant)) {
                                Ok(_) => {
                                    let should_restart = self.should_restart_after_refresh(&account_snapshot.id);
                                    if should_restart {
                                        // ⚠️ verifyStrictTokenInBackground 会对比 storage.json 和客户端内存的 token
                                        // 不一致 → clearUserInfo 登出！
                                        if crate::machine::is_trae_running_for_variant(variant) {
                                            use std::time::Duration;
                                            let _ = crate::machine::kill_trae_for_variant(variant);
                                            tokio::time::sleep(Duration::from_millis(800)).await;
                                            let _ = crate::machine::launch_trae_for_variant(variant);
                                            println!(
                                                "[INFO refresh-oauth] 已重启 {} 加载新 OAuth token",
                                                variant.display_name
                                            );
                                        }
                                    } else {
                                        println!(
                                            "[INFO refresh-oauth] 自动重启已关闭，仅 TJ 侧刷新 token"
                                        );
                                    }
                                }
                                Err(e) => println!(
                                    "[WARN refresh-oauth] 写回 {} 失败: {}",
                                    variant.display_name, e
                                ),
                            }
                        }
                    }
                    refreshed.push(id.clone());
                }
                Err(e) => {
                    let now2 = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
                    eprintln!(
                        "[{}] [WARN refresh-oauth] OAuth 账号 {}({}) ExchangeToken 失败: {}",
                        now2, a.name, a.id, e
                    );
                    let err_str = format!("{}", e);
                    if err_str.contains("Refresh Token 已失效") {
                        if let Some(account) = self.store.accounts.iter_mut()
                            .find(|x| x.id == id)
                        {
                            account.oauth_enabled = false;
                            println!(
                                "[WARN refresh-oauth] 标记账号 {} 为非 OAuth 模式",
                                a.name
                            );
                            let _ = self.save_store();
                        }
                    }
                }
            }
        }

        Ok(refreshed)
    }

}
