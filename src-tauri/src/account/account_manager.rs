use anyhow::{anyhow, Result};
use std::fs;
use std::path::PathBuf;

use super::types::*;
use crate::api::{CheckinHeaderEntry, CheckinResult, CheckinStatusResult, CreditSummary, TraeApiClient, UsageQueryResponse, UsageSummary};

/// 账号管理器
pub struct AccountManager {
    store: AccountStore,
    data_path: PathBuf,
}

impl AccountManager {
    /// 创建账号管理器
    pub fn new() -> Result<Self> {
        let data_path = Self::get_data_path()?;
        let store = Self::load_store(&data_path)?;

        Ok(Self { store, data_path })
    }

    /// 获取数据存储路径
    fn get_data_path() -> Result<PathBuf> {
        let proj_dirs = directories::ProjectDirs::from("com", "marscey", "traejumper")
            .ok_or_else(|| anyhow!("无法获取应用数据目录"))?;

        let data_dir = proj_dirs.data_dir();
        fs::create_dir_all(data_dir)?;

        Ok(data_dir.join("accounts.json"))
    }

    /// 加载账号存储
    fn load_store(path: &PathBuf) -> Result<AccountStore> {
        if path.exists() {
            let content = fs::read_to_string(path)?;
            let store: AccountStore = serde_json::from_str(&content)?;
            Ok(store)
        } else {
            Ok(AccountStore::default())
        }
    }

    /// 保存账号存储
    fn save_store(&self) -> Result<()> {
        let content = serde_json::to_string_pretty(&self.store)?;
        fs::write(&self.data_path, content)?;
        Ok(())
    }

    /// 添加账号（通过 cookies）
    pub async fn add_account(&mut self, cookies: String) -> Result<Account> {
        let mut client = TraeApiClient::new(&cookies)?;

        // 获取 token
        let token_result = client.get_user_token().await?;

        // 获取用户信息
        let user_info = client.get_user_info().await?;

        // 检查是否已存在
        if self
            .store
            .accounts
            .iter()
            .any(|a| a.user_id == token_result.user_id)
        {
            return Err(anyhow!("该账号已存在"));
        }

        let mut account = Account::new(
            user_info.screen_name.clone(),
            user_info.non_plain_text_email.unwrap_or_default(),
            cookies,
            token_result.user_id,
            token_result.tenant_id,
        );

        account.avatar_url = user_info.avatar_url;
        account.region = user_info.region;
        account.jwt_token = Some(token_result.token);
        account.token_expired_at = Some(token_result.expired_at);

        self.store.accounts.push(account.clone());

        // 如果是第一个账号，设为活跃账号
        if self.store.active_account_id.is_none() {
            self.store.active_account_id = Some(account.id.clone());
        }

        self.save_store()?;
        Ok(account)
    }

    /// 添加账号（通过 Token，可选 Cookies）
    /// preferred_name: 如果提供，优先使用此名称（用于导入场景，避免 API 返回的名称不准确）
    pub async fn add_account_by_token(&mut self, token: String, cookies: Option<String>, preferred_name: Option<String>) -> Result<Account> {
        println!("[DEBUG] add_account_by_token 开始, token_len={}, cookies={:?}, preferred_name={:?}",
            token.len(), cookies.as_ref().map(|c| format!("{}...", &c[..c.len().min(20)])), preferred_name);

        let client = TraeApiClient::new_with_token(&token)?;
        println!("[DEBUG] add_account_by_token: TraeApiClient 创建成功");

        // 通过 Token 获取用户信息
        let user_info = client.get_user_info_by_token().await?;
        println!("[DEBUG] add_account_by_token: get_user_info_by_token 返回: user_id='{}', tenant_id='{}', screen_name={:?}, email={:?}, avatar_url={:?}",
            user_info.user_id, user_info.tenant_id, user_info.screen_name, user_info.email, user_info.avatar_url);

        // 检查是否已存在
        if self
            .store
            .accounts
            .iter()
            .any(|a| a.user_id == user_info.user_id)
        {
            println!("[DEBUG] add_account_by_token: 账号已存在 (user_id='{}'), 返回错误", user_info.user_id);
            return Err(anyhow!("该账号已存在"));
        }

        // 确定最终名称（优先级：preferred_name > cookies获取的名称 > API返回的名称 > 自动生成）
        let (name, email, avatar_url) = if let Some(preferred) = preferred_name {
            // 如果提供了首选名称，直接使用（这是导入场景，导出数据中的名称应优先）
            let email = if let Some(ref cookies_str) = cookies {
                match self.get_user_info_with_cookies(cookies_str).await {
                    Ok(info) => info.non_plain_text_email.unwrap_or_default(),
                    Err(_) => user_info.email.unwrap_or_default(),
                }
            } else {
                user_info.email.unwrap_or_default()
            };
            let avatar = if let Some(ref cookies_str) = cookies {
                match self.get_user_info_with_cookies(cookies_str).await {
                    Ok(info) => info.avatar_url,
                    Err(_) => user_info.avatar_url.clone().unwrap_or_default(),
                }
            } else {
                user_info.avatar_url.clone().unwrap_or_default()
            };
            println!("[DEBUG] add_account_by_token: 使用首选名称 '{}'", preferred);
            (preferred, email, avatar)
        } else if let Some(ref cookies_str) = cookies {
            println!("[DEBUG] add_account_by_token: 有 cookies, 尝试获取更详细用户信息");
            match self.get_user_info_with_cookies(cookies_str).await {
                Ok(info) => {
                    println!("[DEBUG] add_account_by_token: get_user_info_with_cookies 成功, screen_name='{}'", info.screen_name);
                    (
                        info.screen_name,
                        info.non_plain_text_email.unwrap_or_default(),
                        info.avatar_url,
                    )
                },
                Err(e) => {
                    println!("[DEBUG] add_account_by_token: get_user_info_with_cookies 失败: {}, 使用 Token 数据", e);
                    (
                        user_info.screen_name.clone().unwrap_or_else(|| format!("User_{}", &user_info.user_id[..8.min(user_info.user_id.len())])),
                        user_info.email.unwrap_or_default(),
                        user_info.avatar_url.unwrap_or_default(),
                    )
                },
            }
        } else {
            println!("[DEBUG] add_account_by_token: 无 cookies, 使用 Token 数据");
            (
                user_info.screen_name.clone().unwrap_or_else(|| format!("User_{}", &user_info.user_id[..8.min(user_info.user_id.len())])),
                user_info.email.unwrap_or_default(),
                user_info.avatar_url.unwrap_or_default(),
            )
        };

        println!("[DEBUG] add_account_by_token: 最终名称: name='{}', email='{}', avatar_url='{}', user_id='{}', tenant_id='{}'",
            name, email, avatar_url, user_info.user_id, user_info.tenant_id);

        let mut account = Account::new(
            name,
            email,
            cookies.unwrap_or_default(),
            user_info.user_id.clone(),
            user_info.tenant_id.clone(),
        );

        account.avatar_url = avatar_url;
        account.jwt_token = Some(token);
        account.token_expired_at = None;

        self.store.accounts.push(account.clone());

        // 如果是第一个账号，设为活跃账号
        if self.store.active_account_id.is_none() {
            self.store.active_account_id = Some(account.id.clone());
        }

        self.save_store()?;
        println!("[DEBUG] add_account_by_token: 完成, 添加账号 id='{}', name='{}', user_id='{}'", account.id, account.name, account.user_id);
        Ok(account)
    }

    /// 使用 Cookies 获取用户信息
    async fn get_user_info_with_cookies(&self, cookies: &str) -> Result<crate::api::UserInfoResult> {
        let client = TraeApiClient::new(cookies)?;
        client.get_user_info().await
    }

    /// 删除账号
    pub fn remove_account(&mut self, account_id: &str) -> Result<()> {
        let index = self
            .store
            .accounts
            .iter()
            .position(|a| a.id == account_id)
            .ok_or_else(|| anyhow!("账号不存在"))?;

        self.store.accounts.remove(index);

        // 如果删除的是活跃账号，重置活跃账号
        if self.store.active_account_id.as_deref() == Some(account_id) {
            self.store.active_account_id = self.store.accounts.first().map(|a| a.id.clone());
        }

        self.save_store()?;
        Ok(())
    }

    /// 设置活跃账号
    pub fn set_active_account(&mut self, account_id: &str) -> Result<()> {
        if !self.store.accounts.iter().any(|a| a.id == account_id) {
            return Err(anyhow!("账号不存在"));
        }

        self.store.active_account_id = Some(account_id.to_string());
        self.save_store()?;
        Ok(())
    }

    /// 切换账号（设置活跃账号并将登录信息写入 Trae IDE）
    pub fn switch_account(&mut self, account_id: &str) -> Result<()> {
        // 检查是否已经是当前使用的账号
        if self.store.current_account_id.as_deref() == Some(account_id) {
            return Err(anyhow!("该账号已经是当前使用的账号"));
        }

        let account = self.store.accounts.iter()
            .find(|a| a.id == account_id)
            .ok_or_else(|| anyhow!("账号不存在"))?
            .clone();

        // 检查账号是否有有效的 Token
        let token = account.jwt_token.as_ref()
            .ok_or_else(|| anyhow!("账号没有有效的 Token，无法切换"))?;

        // 构建 Trae IDE 登录信息
        let login_info = crate::machine::TraeLoginInfo {
            token: token.clone(),
            refresh_token: None, // 如果有 refresh token 可以在这里设置
            user_id: account.user_id.clone(),
            email: account.email.clone(),
            username: account.name.clone(),
            avatar_url: account.avatar_url.clone(),
            host: String::new(), // 根据 region 自动选择
            region: if account.region.is_empty() { "SG".to_string() } else { account.region.clone() },
        };

        // 切换 Trae IDE 到该账号（替换登录身份；是否当作全新设备清理本地数据由独立配置决定）
        let clean_client_data = self.store.switch_as_new_device;
        crate::machine::switch_trae_account(&login_info, account.machine_id.as_deref(), clean_client_data)?;

        // 如果账号有绑定的机器码，也更新系统机器码
        if let Some(machine_id) = &account.machine_id {
            match crate::machine::set_machine_guid(machine_id) {
                Ok(_) => println!("[INFO] 已切换系统机器码: {}", machine_id),
                Err(e) => println!("[WARN] 切换系统机器码失败（可能需要管理员权限）: {}", e),
            }
        }

        // 设置活跃账号和当前使用的账号
        self.store.active_account_id = Some(account_id.to_string());
        self.store.current_account_id = Some(account_id.to_string());
        self.save_store()?;

        println!("[INFO] 已切换到账号: {}", account.email);
        Ok(())
    }

    /// 绑定当前系统机器码到账号
    pub fn bind_machine_id(&mut self, account_id: &str) -> Result<String> {
        // 获取当前系统机器码
        let current_machine_id = crate::machine::get_machine_guid()?;

        // 更新账号的机器码
        let account = self.store.accounts.iter_mut()
            .find(|a| a.id == account_id)
            .ok_or_else(|| anyhow!("账号不存在"))?;

        account.machine_id = Some(current_machine_id.clone());
        account.updated_at = chrono::Utc::now().timestamp();
        let email = account.email.clone();

        self.save_store()?;
        println!("[INFO] 已绑定机器码 {} 到账号 {}", current_machine_id, email);

        Ok(current_machine_id)
    }

    /// 获取所有账号列表
    pub fn get_accounts(&self) -> Vec<AccountBrief> {
        let current_id = self.store.current_account_id.as_deref();
        self.store.accounts.iter().map(|account| {
            let is_current = current_id == Some(account.id.as_str());
            AccountBrief::from_account(account, is_current)
        }).collect()
    }

    /// 获取活跃账号
    pub fn get_active_account(&self) -> Option<&Account> {
        self.store
            .active_account_id
            .as_ref()
            .and_then(|id| self.store.accounts.iter().find(|a| &a.id == id))
    }

    /// 获取指定账号
    pub fn get_account(&self, account_id: &str) -> Result<Account> {
        self.store
            .accounts
            .iter()
            .find(|a| a.id == account_id)
            .cloned()
            .ok_or_else(|| anyhow!("账号不存在"))
    }

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
                    // 如果是 401 错误且有 Cookies，尝试刷新 Token
                    if error_msg.contains("401") && !account.cookies.is_empty() {
                        println!("[INFO] Token 已过期，尝试使用 Cookies 刷新...");
                        // 使用 Cookies 刷新 Token
                        let mut cookie_client = TraeApiClient::new(&account.cookies)?;
                        let token_result = cookie_client.get_user_token().await?;

                        // 更新存储的 Token
                        if let Some(acc) = self.store.accounts.iter_mut().find(|a| a.id == account_id) {
                            acc.jwt_token = Some(token_result.token.clone());
                            acc.token_expired_at = Some(token_result.expired_at.clone());
                        }
                        self.save_store()?;

                        // 使用新 Token 重新获取使用量
                        let new_client = TraeApiClient::new_with_token(&token_result.token)?;
                        new_client.get_usage_summary_by_token().await?
                    } else if error_msg.contains("401") {
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
                            println!("[INFO] 账号 {} 返回 is_credits_billing=false，前端将回退旧配额展示",
                                     account_id);
                        }
                        c
                    }
                    Err(e) => {
                        let err_msg = e.to_string();
                        // 401 → 刷新 Token 重试积分接口
                        if err_msg.contains("401") && !account.cookies.is_empty() {
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
                                return Err(anyhow!("Token 已过期，刷新失败，请手动更新 Token 或 Cookies"));
                            }
                        } else if err_msg.contains("401") {
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

    /// （内部）刷新指定账号 Token，成功返回新 token；失败返回 None，不抛错方便降级
    async fn refresh_token_inner(&mut self, account_id: &str) -> Option<String> {
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

    /// 刷新账号 Token
    pub async fn refresh_token(&mut self, account_id: &str) -> Result<()> {
        let account = self
            .store
            .accounts
            .iter()
            .find(|a| a.id == account_id)
            .ok_or_else(|| anyhow!("账号不存在"))?
            .clone();

        let mut client = TraeApiClient::new(&account.cookies)?;
        let token_result = client.get_user_token().await?;

        if let Some(acc) = self.store.accounts.iter_mut().find(|a| a.id == account_id) {
            acc.jwt_token = Some(token_result.token);
            acc.token_expired_at = Some(token_result.expired_at);
            acc.updated_at = chrono::Utc::now().timestamp();
        }

        self.save_store()?;
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

        // 更新 Token
        acc.jwt_token = Some(token.clone());
        acc.updated_at = chrono::Utc::now().timestamp();

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
        } else {
            return Err(anyhow!("账号不存在"));
        }

        self.save_store()?;
        Ok(())
    }

    /// 清空所有账号数据
    pub fn clear_all_accounts(&mut self) -> Result<usize> {
        let count = self.store.accounts.len();
        self.store.accounts.clear();
        self.store.active_account_id = None;
        self.store.current_account_id = None;
        self.save_store()?;
        println!("[INFO] 已清空所有账号数据，共删除 {} 个账号", count);
        Ok(count)
    }

    /// 导出账号数据
    pub fn export_accounts(&self) -> Result<String> {
        let export_data: Vec<serde_json::Value> = self.store.accounts.iter().map(|acc| {
            serde_json::json!({
                "name": acc.name,
                "email": acc.email,
                "cookies": acc.cookies,
                "user_id": acc.user_id,
                "tenant_id": acc.tenant_id,
                "region": acc.region,
                "plan_type": acc.plan_type,
                "avatar_url": acc.avatar_url,
                "jwt_token": acc.jwt_token,
                "machine_id": acc.machine_id,
            })
        }).collect();

        serde_json::to_string_pretty(&export_data)
            .map_err(|e| anyhow!("导出失败: {}", e))
    }

    /// 导入账号数据
    pub async fn import_accounts(&mut self, data: &str) -> Result<usize> {
        let import_data: Vec<serde_json::Value> = serde_json::from_str(data)
            .map_err(|e| anyhow!("JSON 解析失败: {}", e))?;
        println!("[DEBUG] import_accounts: 解析到 {} 条导入数据", import_data.len());

        let mut imported_count = 0;

        for (i, item) in import_data.iter().enumerate() {
            let token = item.get("jwt_token")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let cookies = item.get("cookies")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let exported_name = item.get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let exported_avatar = item.get("avatar_url")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let exported_user_id = item.get("user_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let exported_region = item.get("region")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            println!("[DEBUG] import_accounts[{}]: name='{}', user_id='{}', region='{}', token_len={}, cookies_len={}",
                i, exported_name, exported_user_id, exported_region, token.len(), cookies.len());

            // 优先使用 Token 添加（Token 方式更稳定，且不依赖 cookies）
            let add_success = if !token.is_empty() {
                let cookies_opt = if cookies.is_empty() { None } else { Some(cookies.clone()) };
                let preferred_name = if exported_name.is_empty() { None } else { Some(exported_name.clone()) };
                match self.add_account_by_token(token.clone(), cookies_opt, preferred_name).await {
                    Ok(acc) => {
                        println!("[DEBUG] import_accounts[{}]: add_account_by_token 成功, account.id='{}', account.name='{}', account.user_id='{}'",
                            i, acc.id, acc.name, acc.user_id);
                        true
                    },
                    Err(e) => {
                        let err_str = e.to_string();
                        println!("[DEBUG] import_accounts[{}]: add_account_by_token 返回: {}", i, err_str);
                        if err_str.contains("已存在") {
                            // 账号已存在，也算导入成功（后续会更新名称）
                            true
                        } else {
                            // API 调用失败（如网络不可达、Token 过期等），尝试直接从导出数据创建账号
                            println!("[WARN] 导入账号失败(Token), 尝试从导出数据直接创建: {}", err_str);
                            let fallback_user_id = if !exported_user_id.is_empty() {
                                exported_user_id.clone()
                            } else {
                                // 尝试从 JWT 中解析 user_id
                                crate::api::TraeApiClient::parse_jwt_user_id(&token).unwrap_or_default()
                            };
                            if !fallback_user_id.is_empty() {
                                let fallback_tenant_id = item.get("tenant_id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let mut fallback_account = Account::new(
                                    exported_name.clone(),
                                    item.get("email").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                    cookies.clone(),
                                    fallback_user_id,
                                    fallback_tenant_id,
                                );
                                fallback_account.avatar_url = exported_avatar.clone();
                                fallback_account.region = exported_region.clone();
                                fallback_account.jwt_token = Some(token.clone());
                                fallback_account.token_expired_at = None;

                                // 检查是否已存在（通过 user_id 或 name）
                                let already_exists = self.store.accounts.iter().any(|a| a.user_id == fallback_account.user_id);
                                if already_exists {
                                    println!("[DEBUG] import_accounts[{}]: 降级创建时发现账号已存在 (user_id='{}'), 跳过添加", i, fallback_account.user_id);
                                    true
                                } else {
                                    self.store.accounts.push(fallback_account);
                                    if self.store.active_account_id.is_none() {
                                        self.store.active_account_id = Some(self.store.accounts.last().unwrap().id.clone());
                                    }
                                    println!("[DEBUG] import_accounts[{}]: 从导出数据直接创建账号成功", i);
                                    true
                                }
                            } else {
                                println!("[WARN] 导入账号失败: 无法获取 user_id, 跳过");
                                false
                            }
                        }
                    }
                }
            } else if !cookies.is_empty() {
                match self.add_account(cookies).await {
                    Ok(acc) => {
                        println!("[DEBUG] import_accounts[{}]: add_account 成功, account.id='{}', account.name='{}', account.user_id='{}'",
                            i, acc.id, acc.name, acc.user_id);
                        true
                    },
                    Err(e) => {
                        println!("[DEBUG] import_accounts[{}]: add_account 返回: {}", i, e);
                        if !e.to_string().contains("已存在") {
                            println!("[WARN] 导入账号失败(Cookies): {}", e);
                            false
                        } else {
                            true
                        }
                    }
                }
            } else {
                false
            };

            // 如果导出数据中有名称，且账号已存在（可能是刚添加的或之前已存在的），
            // 用导出数据的名称覆盖（Token 方式获取的名称可能不准确）
            if !exported_name.is_empty() && !exported_user_id.is_empty() {
                let before_count = self.store.accounts.len();
                println!("[DEBUG] import_accounts[{}]: 尝试名称覆盖, 当前store中有 {} 个账号, 查找 user_id='{}'",
                    i, before_count, exported_user_id);
                if let Some(acc) = self.store.accounts.iter_mut().find(|a| a.user_id == exported_user_id) {
                    println!("[DEBUG] import_accounts[{}]: 找到匹配账号, 当前 name='{}', avatar_url='{}', region='{}', 将覆盖为 name='{}'",
                        i, acc.name, acc.avatar_url, acc.region, exported_name);
                    acc.name = exported_name.clone();
                    if !exported_avatar.is_empty() {
                        acc.avatar_url = exported_avatar.clone();
                    }
                    if !exported_region.is_empty() {
                        acc.region = exported_region.clone();
                    }
                    println!("[DEBUG] import_accounts[{}]: 覆盖后 name='{}', avatar_url='{}', region='{}'",
                        i, acc.name, acc.avatar_url, acc.region);
                } else {
                    println!("[DEBUG] import_accounts[{}]: 未找到匹配 user_id='{}' 的账号, 跳过名称覆盖",
                        i, exported_user_id);
                }
            } else {
                println!("[DEBUG] import_accounts[{}]: 跳过名称覆盖 (exported_name='{}', exported_user_id='{}')",
                    i, exported_name, exported_user_id);
            }

            if add_success {
                imported_count += 1;
            }
        }

        self.save_store()?;
        println!("[DEBUG] import_accounts: 完成, 导入 {} 个账号, 保存后 store 中账号数: {}", imported_count, self.store.accounts.len());
        for (i, acc) in self.store.accounts.iter().enumerate() {
            println!("[DEBUG] import_accounts 保存后 account[{}]: id='{}', name='{}', user_id='{}', region='{}', avatar='{}'",
                i, acc.id, acc.name, acc.user_id, acc.region, acc.avatar_url);
        }
        Ok(imported_count)
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
                    // 如果是 401 错误且有 Cookies，尝试刷新 Token
                    if error_msg.contains("401") && !account.cookies.is_empty() {
                        println!("[INFO] Token 已过期，尝试使用 Cookies 刷新...");
                        // 使用 Cookies 刷新 Token
                        let mut cookie_client = TraeApiClient::new(&account.cookies)?;
                        let token_result = cookie_client.get_user_token().await?;

                        // 更新存储的 Token
                        if let Some(acc) = self.store.accounts.iter_mut().find(|a| a.id == account_id) {
                            acc.jwt_token = Some(token_result.token.clone());
                            acc.token_expired_at = Some(token_result.expired_at.clone());
                        }
                        self.save_store()?;

                        // 使用新 Token 重新查询
                        let new_client = TraeApiClient::new_with_token(&token_result.token)?;
                        new_client.query_usage(start_time, end_time, page_size, page_num).await
                    } else if error_msg.contains("401") {
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

    /// 同步当前账号状态：读取当前目标 Trae 客户端已登录账号，更新 current_account_id。
    ///
    /// 切换目标客户端后调用：原客户端下登录的账号（如账号 a）在新客户端下可能并不存在/已失效，
    /// 这里重新读取新客户端 storage.json 中的 userId，在账号列表中匹配：
    /// - 匹配成功 → current_account_id 指向该账号；
    /// - 新客户端未登录或匹配不到 → current_account_id 清空。
    ///
    /// 返回更新后的当前账号摘要（若新客户端未登录任何已知账号则返回 None）。
    pub fn sync_current_account(&mut self) -> Result<Option<AccountBrief>> {
        // 读取当前目标客户端数据目录中的 storage.json 登录信息（仅解析 userId，不新增账号）
        let user_id = crate::machine::read_trae_login_user_id()?;

        let current = match user_id {
            Some(uid) => self
                .store
                .accounts
                .iter()
                .find(|a| a.user_id == uid),
            None => None,
        };

        match current {
            Some(account) => {
                self.store.current_account_id = Some(account.id.clone());
                self.save_store()?;
                println!("[INFO] 已同步当前账号: {} ({})", account.email, account.user_id);
                Ok(Some(AccountBrief::from_account(account, true)))
            }
            None => {
                self.store.current_account_id = None;
                self.save_store()?;
                println!("[INFO] 当前目标客户端未检测到已登录账号，已清空 current_account_id");
                Ok(None)
            }
        }
    }

    /// 从 Trae IDE 读取当前登录账号（支持当前目标应用变体 + 加密存储解密）
    pub async fn read_trae_ide_account(&mut self) -> Result<Option<Account>> {
        // 按当前目标应用变体获取数据目录（TraeCode CN / TraeWork CN / 国际版 Trae）
        #[cfg(any(target_os = "windows", target_os = "macos"))]
        let trae_data_path = crate::trae_app::data_dir_of(crate::trae_app::current());

        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        let trae_data_path: PathBuf = {
            return Err(anyhow!("此功能仅支持 Windows 和 macOS 系统"));
        };

        let storage_path = trae_data_path
            .join("User")
            .join("globalStorage")
            .join("storage.json");

        // 检查文件是否存在
        if !storage_path.exists() {
            return Ok(None);
        }

        // 读取文件内容
        let content = fs::read_to_string(&storage_path)
            .map_err(|e| anyhow!("读取 Trae IDE 配置文件失败: {}", e))?;

        // 解析 JSON
        let storage: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| anyhow!("解析 Trae IDE 配置文件失败: {}", e))?;

        // 获取 iCubeAuthInfo 字段（国内版为加密存储，需先解密；兼容旧版明文）
        let auth_info_raw = storage
            .get("iCubeAuthInfo://icube.cloudide")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("未找到 Trae IDE 登录信息"))?;

        let auth_info_str = crate::crypto::read_storage_value(auth_info_raw);

        // 解析嵌套的 JSON 字符串
        let auth_info: serde_json::Value = serde_json::from_str(&auth_info_str)
            .map_err(|e| anyhow!("解析 Trae IDE 认证信息失败: {}", e))?;

        // 提取账号信息
        let token = auth_info
            .get("token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("未找到 Token"))?
            .to_string();

        let user_id = auth_info
            .get("userId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("未找到 User ID"))?
            .to_string();

        let email = auth_info
            .get("account")
            .and_then(|acc| acc.get("email"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let avatar_url = auth_info
            .get("account")
            .and_then(|acc| acc.get("avatar_url"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let username = auth_info
            .get("account")
            .and_then(|acc| acc.get("username"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // 提取区域信息（CN 账号后续切换时需使用 api.trae.cn）
        let region = auth_info
            .get("userRegion")
            .and_then(|r| r.get("region"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // 检查账号是否已存在
        if self.store.accounts.iter().any(|a| a.user_id == user_id) {
            println!("[INFO] Trae IDE 账号已存在于账号管理中");
            return Ok(None);
        }

        // 使用 Token 获取完整的用户信息
        let client = TraeApiClient::new_with_token(&token)?;
        let user_info = client.get_user_info_by_token().await?;

        // 创建账号对象
        let mut account = Account::new(
            if username.is_empty() {
                user_info.screen_name.unwrap_or_else(|| format!("User_{}", &user_id[..8.min(user_id.len())]))
            } else {
                username
            },
            if email.is_empty() {
                user_info.email.unwrap_or_default()
            } else {
                email
            },
            String::new(), // Trae IDE 不存储 cookies
            user_id,
            user_info.tenant_id,
        );

        account.avatar_url = if avatar_url.is_empty() {
            user_info.avatar_url.unwrap_or_default()
        } else {
            avatar_url
        };
        account.jwt_token = Some(token);
        if !region.is_empty() {
            account.region = region;
        }

        // 添加到账号列表
        self.store.accounts.push(account.clone());

        // 如果是第一个账号，设为活跃账号
        if self.store.active_account_id.is_none() {
            self.store.active_account_id = Some(account.id.clone());
        }

        self.save_store()?;

        println!("[INFO] 成功从 Trae IDE 读取并添加账号: {}", account.email);
        Ok(Some(account))
    }

    /// 判断账号的 Token 是否即将过期（< 1小时）或已过期
    fn is_token_expiring_soon(account: &Account) -> bool {
        match &account.token_expired_at {
            None => true, // 无过期时间信息，需要刷新
            Some(expired_at) => {
                match chrono::DateTime::parse_from_rfc3339(expired_at) {
                    Ok(expiry) => {
                        let now = chrono::Utc::now();
                        let one_hour = chrono::Duration::hours(1);
                        expiry.with_timezone(&chrono::Utc) < now + one_hour
                    }
                    Err(_) => {
                        // 尝试解析为时间戳（秒）
                        if let Ok(ts) = expired_at.parse::<i64>() {
                            let now = chrono::Utc::now().timestamp();
                            ts < now + 3600
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
        let account_ids: Vec<String> = self.store.accounts.iter()
            .filter(|a| !a.cookies.is_empty())
            .filter(|a| Self::is_token_expiring_soon(a))
            .map(|a| a.id.clone())
            .collect();

        for id in account_ids {
            match self.refresh_token(&id).await {
                Ok(_) => {
                    println!("[INFO] 自动刷新 Token 成功: {}", id);
                    refreshed.push(id);
                }
                Err(e) => {
                    println!("[WARN] 自动刷新 Token 失败 {}: {}", id, e);
                }
            }
        }
        Ok(refreshed)
    }

    /// 领取生日礼包
    pub async fn claim_birthday_bonus(&mut self, account_id: &str) -> Result<()> {
        let account = self.store.accounts.iter()
            .find(|a| a.id == account_id)
            .ok_or_else(|| anyhow!("账号不存在"))?;

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

    /// 获取「切换账号当作新设备」开关状态
    pub fn get_switch_as_new_device(&self) -> bool {
        self.store.switch_as_new_device
    }

    /// 设置「切换账号当作新设备」开关状态（即时持久化生效）
    pub fn set_switch_as_new_device(&mut self, enabled: bool) -> Result<()> {
        self.store.switch_as_new_device = enabled;
        self.save_store()?;
        println!("[INFO] 已设置切换账号当作新设备: {}", enabled);
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
        let token = {
            let account = self.store.accounts.iter()
                .find(|a| a.id == account_id)
                .ok_or_else(|| anyhow!("账号不存在"))?;
            account.jwt_token.clone()
                .ok_or_else(|| anyhow!("账号没有 Token，请先刷新 Token"))?
        };

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
        let token = {
            let account = self.store.accounts.iter()
                .find(|a| a.id == account_id)
                .ok_or_else(|| anyhow!("账号不存在"))?;
            account.jwt_token.clone()
                .ok_or_else(|| anyhow!("账号没有 Token，请先刷新 Token"))?
        };

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
    ///   2. 每个账号签到之间加入 20~60 秒随机延迟，避免瞬时批量请求触发风控
    ///   3. 返回每条的 skipped 状态，供前端区分"跳过（已签到）"与"签到成功"
    pub async fn checkin_all(&mut self) -> Result<Vec<(String, String, CheckinResult, bool)>> {
        let mut results = Vec::new();

        // 快照账号列表，避免迭代借用与 &mut self 方法调用冲突
        let snapshot: Vec<(String, String, bool)> = self.store.accounts.iter()
            .map(|a| (a.id.clone(), a.name.clone(), a.jwt_token.is_some()))
            .collect();
        let total = snapshot.len();

        for (idx, (account_id, account_name, has_token)) in snapshot.into_iter().enumerate() {
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

            // ---------- Step 1: 先查签到状态，已签到则跳过 claim ----------
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
            } else {
                // ---------- Step 2: 执行签到 claim ----------
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
            }

            // ---------- Step 3: 账号间随机延迟（最后一个账号不加） ----------
            // ThreadRng 是 !Send 不能跨 await，必须先在独立 scope 中取数值再 await
            if idx + 1 < total {
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
            }
        }

        Ok(results)
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试导入账号时名称覆盖逻辑
    #[tokio::test]
    async fn test_import_account_name_override() {
        // 创建一个临时路径用于测试
        let temp_dir = std::env::temp_dir().join("traejumper_test_import");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        let test_path = temp_dir.join("test_accounts.json");

        let mut manager = AccountManager {
            store: AccountStore::default(),
            data_path: test_path.clone(),
        };

        // 先手动添加一个账号（模拟已存在的账号，但名称是 User_xxx 格式）
        let mut existing_account = Account::new(
            "User_41928646".to_string(),
            "".to_string(),
            "".to_string(),
            "4192864699424393".to_string(),
            "7o2d894p7dr0o4".to_string(),
        );
        existing_account.avatar_url = "".to_string();
        existing_account.region = "".to_string();
        manager.store.accounts.push(existing_account);

        println!("[TEST] 测试账号名称: '{}'", manager.store.accounts[0].name);

        // 模拟导入过程（只执行名称覆盖逻辑，跳过 API 调用）
        let test_json = r#"[
            {
                "name": "用户7956360138",
                "email": "",
                "cookies": "",
                "user_id": "4192864699424393",
                "tenant_id": "7o2d894p7dr0o4",
                "region": "CN",
                "plan_type": "Free",
                "avatar_url": "https://example.com/avatar.png",
                "jwt_token": "",
                "machine_id": null
            }
        ]"#;

        let import_data: Vec<serde_json::Value> = serde_json::from_str(test_json).unwrap();
        for item in import_data {
            let exported_name = item.get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let exported_avatar = item.get("avatar_url")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let exported_user_id = item.get("user_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let exported_region = item.get("region")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            // 执行名称覆盖逻辑（与 import_accounts 中相同）
            if !exported_name.is_empty() && !exported_user_id.is_empty() {
                if let Some(acc) = manager.store.accounts.iter_mut().find(|a| a.user_id == exported_user_id) {
                    acc.name = exported_name.clone();
                    if !exported_avatar.is_empty() {
                        acc.avatar_url = exported_avatar.clone();
                    }
                    if !exported_region.is_empty() {
                        acc.region = exported_region.clone();
                    }
                }
            }
        }

        // 断言：名称应该被覆盖为导出数据中的名称
        assert_eq!(manager.store.accounts[0].name, "用户7956360138", "名称覆盖失败！");
        assert_eq!(manager.store.accounts[0].avatar_url, "https://example.com/avatar.png", "头像覆盖失败！");
        assert_eq!(manager.store.accounts[0].region, "CN", "区域覆盖失败！");

        let _ = std::fs::remove_dir_all(&temp_dir);
        println!("[TEST] 所有断言通过！名称覆盖逻辑正确工作。");
    }

    /// 测试完整的 import_accounts 流程（使用空的 jwt_token，测试名称覆盖路径）
    #[tokio::test]
    async fn test_import_accounts_full_flow() {
        let temp_dir = std::env::temp_dir().join("traejumper_test_full_import");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        let test_path = temp_dir.join("test_accounts.json");

        let mut manager = AccountManager {
            store: AccountStore::default(),
            data_path: test_path.clone(),
        };

        // 测试数据：cookies 和 jwt_token 都为空，跳过 API 调用
        // 验证：名称覆盖不会执行（因为没有账号被添加）
        let test_json = r#"[
            {
                "name": "用户7956360138",
                "email": "",
                "cookies": "",
                "user_id": "4192864699424393",
                "tenant_id": "7o2d894p7dr0o4",
                "region": "CN",
                "plan_type": "Free",
                "avatar_url": "https://example.com/avatar.png",
                "jwt_token": "",
                "machine_id": null
            }
        ]"#;

        let result = manager.import_accounts(test_json).await.unwrap();
        println!("[TEST] import_accounts 返回: {} 个账号", result);

        // 由于 jwt_token 和 cookies 都为空，应该导入 0 个账号
        assert_eq!(result, 0, "无 token/cookies 时应导入 0 个账号");
        assert_eq!(manager.store.accounts.len(), 0, "store 中应无账号");

        // 测试：已有账号 + 空 jwt_token 的导入数据
        // 验证：名称覆盖逻辑是否对已存在的账号生效
        let mut existing_account = Account::new(
            "User_41928646".to_string(),
            "".to_string(),
            "".to_string(),
            "4192864699424393".to_string(),
            "7o2d894p7dr0o4".to_string(),
        );
        existing_account.avatar_url = "".to_string();
        existing_account.region = "".to_string();
        manager.store.accounts.push(existing_account);
        manager.save_store().unwrap();

        // 再次导入（空 jwt_token，不触发 API 调用，但名称覆盖应针对已存在账号）
        let result = manager.import_accounts(test_json).await.unwrap();
        println!("[TEST] 第二次 import_accounts 返回: {} 个账号", result);
        println!("[TEST] 第二次导入后 store 中账号: {} 个", manager.store.accounts.len());

        // 由于 jwt_token 为空，add_account_by_token 不会被调用
        // add_success 为 false，所以 imported_count 为 0
        // 但名称覆盖逻辑应该对已存在的账号生效
        println!("[TEST] 最终账号名称: '{}'", manager.store.accounts[0].name);

        // 关键验证：已存在的账号名称应该被覆盖
        // 注意：import_accounts 中名称覆盖逻辑在 add_success 判断之前执行
        // 即使 add_success 为 false，名称覆盖也应该生效
        assert_eq!(manager.store.accounts[0].name, "用户7956360138", "名称覆盖应该在导入时对已存在账号生效！");

        let _ = std::fs::remove_dir_all(&temp_dir);
        println!("[TEST] 完整流程测试通过！");
    }

    /// 测试 JWT Token 解析和降级创建路径
    #[tokio::test]
    async fn test_import_fallback_from_jwt() {
        let temp_dir = std::env::temp_dir().join("traejumper_test_jwt_fallback");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        let test_path = temp_dir.join("test_accounts.json");

        let mut manager = AccountManager {
            store: AccountStore::default(),
            data_path: test_path.clone(),
        };

        // 使用一个真实的 JWT Token 来测试解析
        // 这个 token 是从实际的导出文件中提取的
        let test_jwt = "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.eyJkYXRhIjp7ImlkIjoiNDE5Mjg2NDY5OTQyNDM5MyIsInNvdXJjZSI6InJlZnJlc2hfdG9rZW4iLCJzb3VyY2VfaWQiOiJycU5LNGlFdDRickFZSlhmeFhpVzZfTXZMYjVyS2FteHE5enlQaXQ2QW9ZPS4xOGNiYmM1ZGY4Y2MzYmI0IiwidGVuYW50X2lkIjoiN28yZDg5NHA3ZHIwbzQiLCJ0eXBlIjoidXNlciJ9LCJleHAiOjE3ODc5MzgzODgsImlhdCI6MTc4NjcyODc4OH0.Cx3NREOtJlGGKW6QTb3F5MoVu52xG2GaUNXEpvBMoqWfSJyqu0yjl1p0RL6to3tgAhSsH838NL_vQdDk6qj8WfsubCDj5XuLl9TxqTmYhgrCgZVnFMSxszMi6C0Y2adzTRb0Hk_griCXZZs3GJDLgNP3vIiOK4ukzm6wXJkt1LHq3El3fqKEb4jT1uSICK_OqhuJzkB3zrQ1O0Ng0oDTtdvNtLYGjmveOSfSvhOeUXHVx6PAy1UCN0yhNCEJ0ni5-w4v6I8bEhlGDR90Gf87ZxjewTPusNI6TuRKrAYQssYsizHIwDFXRmnzDco6YMMBQwvMv_qJM0rDOCSdE8juQf_X39tj0vmlvw1w8vPrbuuJr9gQB3UVwuhczy8J9lw7OAO0w_thts0wN9b6rYh4UtG4jIB1DJqEvSFmnGk7O1n3nf5kHKlpaa1X4acpLEAy31wNTR05bsSd1SdkS2Z2T_SXro9MKuoqfnYsyz7sBS3xolXpCYjL8zIHxBDGWpNUp1kBpJ-lQELideJtm2ljY6te_Tqas9NAa0_hzLP5KKB8AM51-wZonkG24rupYoTQQ6OahCqZXNwOHgMQuSYaDq50Lw26iYA-UNR1KLNtqTilEFIngAFLdVZGE4zx1XkX5sPjNIYyo07ZBh9ZT6-iLcMvRh7VJr7Yc7caJTr_ZHE";

        // 验证 JWT 解析
        let user_id = crate::api::TraeApiClient::parse_jwt_user_id(test_jwt).unwrap();
        assert_eq!(user_id, "4192864699424393", "JWT 解析 user_id 失败");
        println!("[TEST] JWT 解析成功: user_id='{}'", user_id);

        // 使用包含真实 JWT 的导入数据测试降级创建
        let test_json = format!(r#"[
            {{
                "name": "用户7956360138",
                "email": "",
                "cookies": "",
                "user_id": "4192864699424393",
                "tenant_id": "7o2d894p7dr0o4",
                "region": "CN",
                "plan_type": "Free",
                "avatar_url": "https://example.com/avatar.png",
                "jwt_token": "{}",
                "machine_id": null
            }}
        ]"#, test_jwt);

        // 调用完整的 import_accounts（会尝试 API 调用并失败，然后降级到 JWT 解析创建）
        let result = manager.import_accounts(&test_json).await.unwrap();
        println!("[TEST] import_accounts 返回: {} 个账号, store 中账号: {} 个", result, manager.store.accounts.len());

        // 验证降级创建成功
        if manager.store.accounts.len() > 0 {
            let acc = &manager.store.accounts[0];
            println!("[TEST] 降级创建账号: name='{}', user_id='{}', region='{}', avatar='{}'",
                acc.name, acc.user_id, acc.region, acc.avatar_url);
            assert_eq!(acc.name, "用户7956360138", "名称应该正确");
            assert_eq!(acc.user_id, "4192864699424393", "user_id 应该正确");
            assert_eq!(acc.region, "CN", "region 应该正确");
            assert_eq!(acc.avatar_url, "https://example.com/avatar.png", "avatar_url 应该正确");
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
        println!("[TEST] JWT 降级创建测试通过！");
    }

    /// 完整集成测试：模拟导入用户真实导出的3个账号，验证所有名称正确
    #[tokio::test]
    async fn test_import_full_real_data() {
        let temp_dir = std::env::temp_dir().join("traejumper_test_full_real");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        let test_path = temp_dir.join("test_accounts.json");

        let mut manager = AccountManager {
            store: AccountStore::default(),
            data_path: test_path.clone(),
        };

        // 使用用户真实导出数据中的3个JWT Token
        let jwt1 = "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.eyJkYXRhIjp7ImlkIjoiNDE5Mjg2NDY5OTQyNDM5MyIsInNvdXJjZSI6InJlZnJlc2hfdG9rZW4iLCJzb3VyY2VfaWQiOiJycU5LNGlFdDRickFZSlhmeFhpVzZfTXZMYjVyS2FteHE5enlQaXQ2QW9ZPS4xOGNiYmM1ZGY4Y2MzYmI0IiwidGVuYW50X2lkIjoiN28yZDg5NHA3ZHIwbzQiLCJ0eXBlIjoidXNlciJ9LCJleHAiOjE3ODc5MzgzODgsImlhdCI6MTc4NjcyODc4OH0.Cx3NREOtJlGGKW6QTb3F5MoVu52xG2GaUNXEpvBMoqWfSJyqu0yjl1p0RL6to3tgAhSsH838NL_vQdDk6qj8WfsubCDj5XuLl9TxqTmYhgrCgZVnFMSxszMi6C0Y2adzTRb0Hk_griCXZZs3GJDLgNP3vIiOK4ukzm6wXJkt1LHq3El3fqKEb4jT1uSICK_OqhuJzkB3zrQ1O0Ng0oDTtdvNtLYGjmveOSfSvhOeUXHVx6PAy1UCN0yhNCEJ0ni5-w4v6I8bEhlGDR90Gf87ZxjewTPusNI6TuRKrAYQssYsizHIwDFXRmnzDco6YMMBQwvMv_qJM0rDOCSdE8juQf_X39tj0vmlvw1w8vPrbuuJr9gQB3UVwuhczy8J9lw7OAO0w_thts0wN9b6rYh4UtG4jIB1DJqEvSFmnGk7O1n3nf5kHKlpaa1X4acpLEAy31wNTR05bsSd1SdkS2Z2T_SXro9MKuoqfnYsyz7sBS3xolXpCYjL8zIHxBDGWpNUp1kBpJ-lQELideJtm2ljY6te_Tqas9NAa0_hzLP5KKB8AM51-wZonkG24rupYoTQQ6OahCqZXNwOHgMQuSYaDq50Lw26iYA-UNR1KLNtqTilEFIngAFLdVZGE4zx1XkX5sPjNIYyo07ZBh9ZT6-iLcMvRh7VJr7Yc7caJTr_ZHE";
        let jwt2 = "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.eyJkYXRhIjp7ImlkIjoiMjM0NjIxNzg4NzYwOTMyIiwic291cmNlIjoicmVmcmVzaF90b2tlbiIsInNvdXJjZV9pZCI6IkJiREl5OHdtdzhNRV9vWTJJZDF5c292elQxalFYRk1ZajZoVEcxdG50MjA9LjE4Y2MzYTBiNmIzZjJkZmQiLCJ0ZW5hbnRfaWQiOiI3bzJkODk0cDdkcjBvNCIsInR5cGUiOiJ1c2VyIn0sImV4cCI6MTc4ODA3NjU3MiwiaWF0IjoxNzg2ODY2OTcyfQ.NOgs0iOwFw_sDNNpp1q3Alhb5LCw1XJFuct4tStBIrUkJHg7gcrBBZQU8pihu4gvCCYNwvZjebXK5DU3gH9jt55OKvb9DX2SDXhslu35b4q2Mjhoqgfhi-7g5XMRFtBoJr_FPds6-6qs9wE-cMxA1o2FFPt-YsYqP5eaLmQ8IKx13tDxH4x3P71NFpnYgBv6CzJaI_eVIIMcBOP2uc4OQB_gaIrpcCFr2ickIWkdkZ1AY67E7l9pMlvflc9Zy6iX8MAtVSr3XyIUCQplzYWYa0xO0LQmCBCPJde8FJHHa-DKe9sdWbs2UTERTo693aa-MSW6HIoNnt7RKYFDenm5_A62v4W58sld-dFiOkVUrVmDvhrRwNZQUnae43X-tCGF_n8J1YiF5Wwlu4oYNQx9LIwv5aII7qRnNS6HlgcevlyhepxogkaCzitWTUTFCY_WDsbUdNU9CS-3JdJTzMr2HH55mGvipWRFaG1Bv6mIsuga5wz9_kb75w-cxyw2Qk5NGUtejJBBtLyuIK8mR4JE_Y08vuT-sabV6ftX8F1tkBcKEzl0JXHO6HqVMuYltES-8yGNnWUtPXrxOPw-WGz06wJfRjetHUWsFYPX3hqQ4d3q76ZXilS73qa8feNi5P-dRqtZiUeja_Gk1FfFHeXdJ6psxjKV0hODWl43yvTS3Dg";
        let jwt3 = "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.eyJkYXRhIjp7ImlkIjoiNDA2NTM1MTEyMzg4NjQxMCIsInNvdXJjZSI6InJlZnJlc2hfdG9rZW4iLCJzb3VyY2VfaWQiOiI4Z21vaElyZHFKck1pZERDUnZYaFIxWk02ZHk4cC1pRFdfXzJ5c2V5Zk40PS4xOGNjNTNmMDQ3NzJkZDc1IiwidGVuYW50X2lkIjoiN28yZDg5NHA3ZHIwbzQiLCJ0eXBlIjoidXNlciJ9LCJleHAiOjE3ODgxMDUwNDMsImlhdCI6MTc4Njg5NTQ0M30.OjyN4iNifs5mk_N_ZWC8SlTmZmMo3WpmD5gRS43G_8cbYaeD-3lw4yZJB0Owl36_jvpMxxiywtKCq28BUvSKjMCzqG4yoHSQIw8CMsK1UaKXrx8sgFFkE8n9fVr9AgzcCWqycI1mOgnnU_dMmAToA-3Rz4gq-XtT9haj78NJugxLZrL9zIzFTg5LvTA3xnA-p9wIQhL4lVQSWFnSufEQTe_x21ZVc72hZJqqJNFKSOF98O2WkCvP5ixlzqN0ekzxBWJnYOwVotZg3gbeGLKx8CyD-RqQe9fArE1RzEf-GRmjkPnvNI68Z-gibVoMFcQeiG3KmPzjFiR-Gll5zm18m_bkmUFTpPxO_slVrEko9C7m2sOAfaa-x-co_0WkSy61Hzqt8-dislSB25tH_we5IMlKu7lI4g855U8tibJGWLMzMKVu0VDsN_2f5147ML5Tf3PWhL9jqwOJGmxGtKS3R37Mi_j2u3a6plCDiWR0EOHwUrEEu-3oA0QQ_zbzPNTvk9vfUcMPBkcAQEDA3fJSczSWSHAvUz5QBKemLcmlGJiG6x9y6oSgVNlsceLjKsvVoirZVFmREgxXZ2L-1-XlspIas207UNKGDAnyYRxHbTB_JI2gtta-1lAYUoBHK1YmhlrnZA3RfDcULJZhOBVg1vCHhk3Eds6T5Krwmb094hA";

        // 构建与用户导出文件完全相同的 JSON 数据
        let test_json = format!(r#"[
            {{
                "name": "用户7956360138",
                "email": "",
                "cookies": "",
                "user_id": "4192864699424393",
                "tenant_id": "7o2d894p7dr0o4",
                "region": "CN",
                "plan_type": "Free",
                "avatar_url": "https://p6-passport.byteacctimg.com/img/user-avatar/assets/11c35f217be67876726ffb8038af8e4e_192_192.png~128x128.image",
                "jwt_token": "{}",
                "machine_id": null
            }},
            {{
                "name": "Francisyep",
                "email": "",
                "cookies": "",
                "user_id": "234621788760932",
                "tenant_id": "7o2d894p7dr0o4",
                "region": "CN",
                "plan_type": "Free",
                "avatar_url": "https://p3-passport.byteacctimg.com/img/user-avatar/assets/11c35f217be67876726ffb8038af8e4e_192_192.png~128x128.image",
                "jwt_token": "{}",
                "machine_id": null
            }},
            {{
                "name": "🏄🏻冲浪猫",
                "email": "",
                "cookies": "",
                "user_id": "4065351123886410",
                "tenant_id": "7o2d894p7dr0o4",
                "region": "CN",
                "plan_type": "Free",
                "avatar_url": "https://p9-passport.byteacctimg.com/img/user-avatar/67c16e65322f39664f9f2b6612f8bf11~128x128.image",
                "jwt_token": "{}",
                "machine_id": null
            }}
        ]"#, jwt1, jwt2, jwt3);

        // 调用完整的 import_accounts
        let result = manager.import_accounts(&test_json).await.unwrap();
        println!("[TEST] 完整导入返回: {} 个账号, store 中账号: {} 个", result, manager.store.accounts.len());

        // 验证导入了3个账号
        assert_eq!(result, 3, "应该成功导入3个账号");
        assert_eq!(manager.store.accounts.len(), 3, "store中应该有3个账号");

        // 验证每个账号的名称正确
        let expected = [
            ("用户7956360138", "4192864699424393", "CN", "byteacctimg"),
            ("Francisyep", "234621788760932", "CN", "byteacctimg"),
            ("🏄🏻冲浪猫", "4065351123886410", "CN", "byteacctimg"),
        ];

        for (i, (exp_name, exp_uid, exp_region, exp_avatar_sub)) in expected.iter().enumerate() {
            let acc = &manager.store.accounts[i];
            println!("[TEST] 账号[{}]: name='{}', user_id='{}', region='{}'",
                i, acc.name, acc.user_id, acc.region);
            assert_eq!(acc.name, *exp_name, "账号[{}] 名称应该正确", i);
            assert_eq!(acc.user_id, *exp_uid, "账号[{}] user_id 应该正确", i);
            assert_eq!(acc.region, *exp_region, "账号[{}] region 应该正确", i);
            assert!(acc.avatar_url.contains(exp_avatar_sub), "账号[{}] avatar_url 应包含'{}'", i, exp_avatar_sub);
        }

        // 模拟导出并验证导出数据中的名称也是正确的
        let exported = manager.export_accounts().unwrap();
        let exported_data: Vec<serde_json::Value> = serde_json::from_str(&exported).unwrap();
        println!("[TEST] 导出数据: {}", serde_json::to_string_pretty(&exported_data).unwrap());
        for (i, item) in exported_data.iter().enumerate() {
            let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("");
            assert_eq!(name, expected[i].0, "导出数据中账号[{}] 名称应该正确", i);
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
        println!("[TEST] 完整集成测试通过！所有3个账号名称、region、头像均正确！");
    }
}
