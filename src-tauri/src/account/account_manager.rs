use anyhow::{anyhow, Result};
use std::fs;
use std::path::PathBuf;

use super::types::*;
use crate::api::{CheckinHeaderEntry, CheckinResult, CheckinStatusResult, CreditSummary, DoubaoApiClient, DoubaoCheckinView, DoubaoQuotaView, DoubaoTimelineView, TraeApiClient, UsageQueryResponse, UsageSummary};


// ==== 职责拆分子模块（同一 impl 的分布文件，按领域聚合方法） ====
mod checkin;
mod doubao;
mod import_export;
mod switch;
mod token;
mod usage;

/// 账号管理器
pub struct AccountManager {
    pub(crate) store: AccountStore,
    data_path: PathBuf,
    /// 上次自动签到的日期（YYYY-MM-DD），内存态，重启不保留；
    /// 用于同一天内避免重复触发自动签到（已签到账号本就会跳过，此字段主要减少无谓 API 调用）
    last_auto_checkin_date: Option<String>,
}

/// 从 Trae 客户端 storage.json 读取的当前登录信息。
///
/// `token` 为 None 表示该客户端版本不在 storage.json 暴露登录 token
/// （如 TraeCode CN 2.3.87416+），此时仅能通过 usertag 识别登录身份，
/// 用于活跃账号判定；token 同步/客户端导入功能不可用。
pub(crate) struct ClientLogin {
    pub user_id: String,
    pub email: String,
    pub token: Option<String>,
    /// cloudide 中的 OAuth refresh_token（如果存在）。
    /// TJ OAuth 注入 / 客户端原生 OAuth 登录都会写入此字段。
    /// 有此字段 → 可通过 ExchangeToken（Refresh Flow）静默续签。
    pub refresh_token: Option<String>,
    /// refresh_token 过期时间（RFC3339），用于判断是否需要在过期前轮换。
    pub refresh_expired_at: Option<String>,
    /// 该客户端是否为"新版自管理架构"：storage.json 含 `iCubeAuthInfo://icube-dc:*`
    /// 设备密钥键。此类客户端用设备密钥/OAuth refresh token 自行续签登录态，
    /// 不依赖 TraeJumper 注入/续签 token；调用 GetUserToken 会作废其自管的新 token，
    /// 必须对该客户端活跃的账号完全让位。
    pub self_managed: bool,
}

impl AccountManager {
    /// 创建账号管理器
    pub fn new() -> Result<Self> {
        let data_path = Self::get_data_path()?;
        let store = Self::load_store(&data_path)?;

        Ok(Self { store, data_path, last_auto_checkin_date: None })
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
    pub(crate) fn save_store(&self) -> Result<()> {
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
        account.login_source = AccountLoginSource::WebviewCookie;

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
    /// source: 登录来源（手动 Token=ManualToken / 内部 WebView=Webview）
    pub async fn add_account_by_token(&mut self, token: String, cookies: Option<String>, preferred_name: Option<String>, source: AccountLoginSource) -> Result<Account> {
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let cookie_count = cookies.as_ref().map(|c| c.split(';').filter(|s| !s.trim().is_empty()).count()).unwrap_or(0);
        let cookie_names: Vec<&str> = cookies.as_ref().map(|c| {
            c.split(';').filter_map(|s| s.split('=').next().map(|s| s.trim())).filter(|s| !s.is_empty()).collect()
        }).unwrap_or_default();
        let client = TraeApiClient::new_with_token(&token)?;

        // 通过 Token 获取用户信息
        let user_info = client.get_user_info_by_token().await?;

        // 检查是否已存在
        if self
            .store
            .accounts
            .iter()
            .any(|a| a.user_id == user_info.user_id)
        {
            return Err(anyhow!("该账号已存在"));
        }

        // 确定最终名称（优先级：preferred_name > cookies获取的名称 > API返回的名称 > 自动生成）
        let (name, email, avatar_url) = if let Some(preferred) = preferred_name {
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
            (preferred, email, avatar)
        } else if let Some(ref cookies_str) = cookies {
            match self.get_user_info_with_cookies(cookies_str).await {
                Ok(info) => {
                    (
                        info.screen_name,
                        info.non_plain_text_email.unwrap_or_default(),
                        info.avatar_url,
                    )
                },
                Err(_) => {
                    (
                        user_info.screen_name.clone().unwrap_or_else(|| format!("User_{}", &user_info.user_id[..8.min(user_info.user_id.len())])),
                        user_info.email.unwrap_or_default(),
                        user_info.avatar_url.unwrap_or_default(),
                    )
                },
            }
        } else {
            (
                user_info.screen_name.clone().unwrap_or_else(|| format!("User_{}", &user_info.user_id[..8.min(user_info.user_id.len())])),
                user_info.email.unwrap_or_default(),
                user_info.avatar_url.unwrap_or_default(),
            )
        };

        let mut account = Account::new(
            name,
            email,
            cookies.unwrap_or_default(),
            user_info.user_id.clone(),
            user_info.tenant_id.clone(),
        );

        account.avatar_url = avatar_url;
        // 先从 JWT 解析 exp（借用 token，不能放在 move 之后）
        let token_expired_at = crate::api::TraeApiClient::parse_jwt_exp(&token)
            .ok()
            .and_then(|exp| {
                chrono::DateTime::from_timestamp(exp, 0)
                    .map(|dt| dt.to_rfc3339())
            });
        account.jwt_token = Some(token);
        account.token_expired_at = token_expired_at;
        account.login_source = source;
        // 浏览器登录刚捕获 cookies，标记上次续签时间，避免刚登录就被判定为 session 过期而立即续签
        account.last_cookie_renewal_at = Some(chrono::Utc::now().timestamp());

        self.store.accounts.push(account.clone());

        // 如果是第一个账号，设为活跃账号
        if self.store.active_account_id.is_none() {
            self.store.active_account_id = Some(account.id.clone());
        }

        self.save_store()?;
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        println!("[{}] [INFO] 新增账号成功: name='{}' user_id='{}' id='{}'", now, account.name, account.user_id, account.id);
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

    /// 获取所有账号列表
    pub fn get_accounts(&self) -> Vec<AccountBrief> {
        let current_id = self.store.current_account_id.as_deref();
        // 收集所有已安装 Trae 客户端的登录态（用于多客户端"当前"标签展示）
        let client_logins: Vec<(&crate::trae_app::TraeAppVariant, ClientLogin)> =
            crate::trae_app::all_variants()
                .iter()
                .filter(|v| crate::trae_app::is_variant_installed(v))
                .filter_map(|v| {
                    self.read_client_login(Some(v))
                        .ok()
                        .filter(|l| !l.user_id.is_empty())
                        .map(|l| (v, l))
                })
                .collect();

        self.store.accounts.iter().map(|account| {
            let is_current = current_id == Some(account.id.as_str());

            // 豆包账号：不参与 Trae 客户端活跃态 / 注入类型推导，返回精简 brief。
            if account.platform == AccountPlatform::Doubao {
                let mut brief = AccountBrief::from_account(account, false);
                brief.has_cookies = !account.cookies.is_empty();
                brief.login_type = AccountLoginType::StandaloneCookie;
                return brief;
            }

            // 来源 runtime 推断/修正：
            // 1. Unknown 按凭证特征推断：oauth_enabled → WebviewOauth，有 cookies → WebviewCookie
            // 2. 来源与凭证不一致的脏数据修正：oauth_enabled=true 但来源非 WebviewOauth → 强制 WebviewOauth
            //    （OAuth 授权入口无论账号之前是什么来源，统一应为 WebviewOauth）
            let mut brief = AccountBrief::from_account(account, is_current);
            if brief.login_source == AccountLoginSource::Unknown {
                if account.oauth_enabled {
                    brief.login_source = AccountLoginSource::WebviewOauth;
                } else if !account.cookies.is_empty() {
                    brief.login_source = AccountLoginSource::WebviewCookie;
                }
            } else if account.oauth_enabled && brief.login_source != AccountLoginSource::WebviewOauth {
                // OAuth 凭证已启用，修正来源标记为 WebviewOauth
                brief.login_source = AccountLoginSource::WebviewOauth;
            }
            // 收集该账号在哪些客户端中真实登录
            let active_in: Vec<String> = client_logins
                .iter()
                .filter(|(_, l)| self.is_active_client_account_inner(&account.id, l, false))
                .map(|(v, _)| v.display_name.to_string())
                .collect();
            brief.is_client_active = !active_in.is_empty();
            brief.active_in_clients = active_in;
            // 客户端激活类型推导（仅客户端活跃时有实际意义）：
            // - 客户端活跃 && 非 TraeJumper 注入 → NativeOauth（客户端原生登录，TraeJumper 让位）
            // - 客户端活跃 && TraeJumper 注入 → 按来源匹配 InjectedOauth/InjectedCookie/InjectedClient/InjectedManual
            // - 仅库内 → 按凭证匹配 StandaloneOauth/StandaloneCookie/StandaloneManual
            let is_injected = account.has_been_injected || account.token_shared_with_client;
            brief.login_type = if is_injected && brief.is_client_active {
                // 客户端活跃且 TraeJumper 注入：按来源 + OAuth 模式选变体
                match brief.login_source {
                    AccountLoginSource::WebviewOauth => AccountLoginType::InjectedOauth,
                    AccountLoginSource::WebviewCookie => AccountLoginType::InjectedCookie,
                    AccountLoginSource::ClientImport => AccountLoginType::InjectedClient,
                    AccountLoginSource::ManualToken => AccountLoginType::InjectedManual,
                    AccountLoginSource::Unknown => {
                        // 存量 Unknown 账号 fallback 到 oauth_enabled + cookies
                        if account.oauth_enabled { AccountLoginType::InjectedOauth }
                        else if !account.cookies.is_empty() { AccountLoginType::InjectedCookie }
                        else { AccountLoginType::InjectedManual }
                    }
                }
            } else if brief.is_client_active {
                // 客户端活跃但 TraeJumper 从未注入 → 客户端原生 OAuth 登录
                AccountLoginType::NativeOauth
            } else {
                // 仅库内，按凭证类型
                if account.oauth_enabled { AccountLoginType::StandaloneOauth }
                else if !account.cookies.is_empty() { AccountLoginType::StandaloneCookie }
                else { AccountLoginType::StandaloneManual }
            };
            // "当前"标识仅在后端记录的切换目标与客户端真实持有会话同时成立时显示
            brief.is_current = is_current && brief.is_client_active;
            brief
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

    /// 读取账号的登录邮箱
    pub fn account_email(&self, account_id: &str) -> Option<String> {
        self.store
            .accounts
            .iter()
            .find(|a| a.id == account_id)
            .map(|a| a.email.clone())
    }

    /// 刷新账号信息：尝试从远端获取最新展示字段 + 验证/续签 access_token。
    ///
    /// 鉴权知识（curl 实测结论）：
    ///   - `/cloudide/api/v3/trae/GetUserInfo`（返回 screen_name/avatar/mobile 等完整脱敏字段）
    ///     只认 **cookies**（sessionid），不认 `Authorization: Cloud-IDE-JWT`（返回 401 Code:20310）。
    ///   - `/trae/api/v2/pay/user_current_entitlement_list`（返回 entitlement/plan/user_id）
    ///     接受 OAuth cloudide JWT，可用来验证 token 有效性 + 拿基础字段。
    ///
    /// 三路策略：
    ///   1) cookies 路径 → GetUserInfo → 拿完整展示字段（cookies 账号首选）
    ///   2) OAuth ExchangeToken Refresh → 持久化新 token → entitlement 验证
    ///      （OAuth Native / OAuth 注入账号；ExchangeToken 成功就把过期 token 续签了）
    ///   3) jwt_token 兜底 → entitlement 验证 token 是否还有效
    ///
    /// 返回值：只要 entitlement 验证通过（或 cookies GetUserInfo 成功）就算成功。
    /// OAuth 账号走 2 能拿到新 token 但拿不到展示字段（展示字段保留原值）。
    pub async fn refresh_account_info(&mut self, account_id: &str) -> Result<Account> {
        let account = self
            .store
            .accounts
            .iter()
            .find(|a| a.id == account_id)
            .cloned()
            .ok_or_else(|| anyhow!("账号不存在: {}", account_id))?;

        println!(
            "[INFO refresh_account_info] 账号={} name={} oauth_enabled={} cookies.len={} jwt_token.len={:?} has_refresh={}",
            account.id,
            account.name,
            account.oauth_enabled,
            account.cookies.len(),
            account.jwt_token.as_ref().map(|t| t.len()),
            account.oauth_refresh_token.is_some(),
        );

        // 豆包账号：走 D1 资料 + D2 概览快照刷新，不参与 Trae Token 续签链路。
        if account.platform == AccountPlatform::Doubao {
            return self.refresh_doubao_info(account_id).await;
        }

        // GetUserInfo 结果（只有 cookies 路径能拿到完整展示字段）
        let mut user_info: Option<crate::api::UserInfoResult> = None;
        // entitlement 结果（OAuth JWT 能通，用来验证 token 有效性 + 更新基础字段）
        let mut entitlement_user_id: Option<String> = None;
        let mut entitlement_ok = false;
        let mut last_err: Option<String> = None;

        // ── 路径 1: cookies → GetUserInfo（完整展示字段） ──
        if !account.cookies.is_empty() {
            match crate::api::TraeApiClient::new(&account.cookies) {
                Ok(client) => match client.get_user_info().await {
                    Ok(info) => {
                        println!(
                            "[INFO refresh_account_info] cookies 路径成功 screen_name={}",
                            info.screen_name
                        );
                        user_info = Some(info);
                    }
                    Err(e) => {
                        let msg = format!("cookies 路径失败: {}", e);
                        println!("[WARN refresh_account_info] {}", msg);
                        last_err = Some(msg);
                    }
                },
                Err(e) => {
                    let msg = format!("cookies client 构建失败: {}", e);
                    println!("[WARN refresh_account_info] {}", msg);
                    last_err = Some(msg);
                }
            }
        } else {
            println!("[INFO refresh_account_info] 无 cookies，跳过路径 1");
        }

        // ── 路径 2: OAuth ExchangeToken Refresh → entitlement 验证 ──
        let has_oauth = account.oauth_enabled
            || (account.has_been_injected && account.oauth_refresh_token.is_some());
        if user_info.is_none() && has_oauth && account.oauth_refresh_token.is_some() {
            let refresh_token = account.oauth_refresh_token.as_ref().unwrap();
            let private_key = match account.oauth_private_key_pem.as_ref() {
                Some(k) => k,
                None => {
                    let msg = "OAuth 账号缺少 oauth_private_key_pem，无法 refresh";
                    println!("[WARN refresh_account_info] {}", msg);
                    last_err = Some(msg.to_string());
                    "" // 下面的 if 会因 private_key.is_empty() 而跳过
                }
            };
            if !private_key.is_empty() {
                let public_key = account.oauth_public_key_pem.as_deref().unwrap_or("");
                let device_id = account.oauth_device_id.as_deref().unwrap_or("trae-jumper-device");
                let machine_id = account
                    .oauth_machine_id
                    .as_deref()
                    .unwrap_or("trae-jumper-machine");
                let variant = crate::trae_app::current().clone();
                let api_host = account
                    .oauth_host
                    .as_deref()
                    .unwrap_or(variant.api_host);

                println!(
                    "[INFO refresh_account_info] 尝试 OAuth ExchangeToken Refresh host={}",
                    api_host
                );
                match crate::oauth::exchange_refresh_token(
                    &variant,
                    api_host,
                    refresh_token,
                    device_id,
                    machine_id,
                    public_key,
                    private_key,
                )
                .await
                {
                    Ok(token_result) => {
                        let new_access = token_result.access_token.clone();
                        println!(
                            "[INFO refresh_account_info] OAuth Refresh 成功 access_token.len={}",
                            new_access.len()
                        );

                        // 持久化新的 access_token（顺带修复过期问题）
                        {
                            let acc = self
                                .store
                                .accounts
                                .iter_mut()
                                .find(|a| a.id == account_id)
                                .unwrap();
                            acc.jwt_token = Some(new_access.clone());
                            acc.token_expired_at = token_result.expired_at();
                            acc.updated_at = chrono::Utc::now().timestamp();
                            // refresh_token 是一次性的，ExchangeToken 成功后会发新的
                            if !token_result.refresh_token.is_empty()
                                && token_result.refresh_token != *refresh_token
                            {
                                acc.oauth_refresh_token =
                                    Some(token_result.refresh_token.clone());
                                acc.oauth_refresh_expired_at =
                                    token_result.refresh_expired_at();
                                println!(
                                    "[INFO refresh_account_info] refresh_token 已轮换（new.len={}）",
                                    token_result.refresh_token.len()
                                );
                            }
                        }
                        let _ = self.save_store();

                        // 用新 access_token 调 entitlement 验证 token 有效
                        // （GetUserInfo 不认 cloudide JWT，必须走 entitlement）
                        match crate::api::TraeApiClient::new_with_token(&new_access) {
                            Ok(client) => {
                                match client.get_entitlement_list_v2_by_token().await {
                                    Ok(ent) => {
                                        entitlement_ok = true;
                                        if let Some(pack) = ent.user_entitlement_pack_list.first() {
                                            entitlement_user_id = Some(
                                                pack.entitlement_base_info
                                                    .user_id
                                                    .clone(),
                                            );
                                        }
                                        println!(
                                            "[INFO refresh_account_info] OAuth Refresh 后 entitlement 验证通过 user_id={:?}",
                                            entitlement_user_id
                                        );
                                    }
                                    Err(e) => {
                                        let msg = format!(
                                            "OAuth Refresh 后 entitlement 失败: {}",
                                            e
                                        );
                                        println!("[WARN refresh_account_info] {}", msg);
                                        last_err = Some(msg);
                                    }
                                }
                            }
                            Err(e) => {
                                let msg = format!("new_access client 构建失败: {}", e);
                                println!("[WARN refresh_account_info] {}", msg);
                                last_err = Some(msg);
                            }
                        }
                    }
                    Err(e) => {
                        let msg = format!("OAuth ExchangeToken Refresh 失败: {}", e);
                        println!("[WARN refresh_account_info] {}", msg);
                        last_err = Some(msg);
                    }
                }
            }
        }

        // ── 路径 3: 残留 jwt_token → entitlement 验证 ──
        // （兜底，非 OAuth 账号有有效 jwt_token 时验证是否还有效）
        if user_info.is_none() && !entitlement_ok {
            if let Some(token) = &account.jwt_token {
                println!("[INFO refresh_account_info] 尝试 jwt_token entitlement 兜底验证");
                match crate::api::TraeApiClient::new_with_token(token) {
                    Ok(client) => {
                        match client.get_entitlement_list_v2_by_token().await {
                            Ok(ent) => {
                                entitlement_ok = true;
                                if let Some(pack) = ent.user_entitlement_pack_list.first() {
                                    entitlement_user_id = Some(
                                        pack.entitlement_base_info.user_id.clone(),
                                    );
                                }
                                println!(
                                    "[INFO refresh_account_info] jwt_token entitlement 兜底通过 user_id={:?}",
                                    entitlement_user_id
                                );
                            }
                            Err(e) => {
                                let msg = format!("jwt_token entitlement 兜底失败: {}", e);
                                println!("[WARN refresh_account_info] {}", msg);
                                last_err = Some(msg);
                            }
                        }
                    }
                    Err(e) => {
                        let msg = format!("jwt_token client 构建失败: {}", e);
                        println!("[WARN refresh_account_info] {}", msg);
                        last_err = Some(msg);
                    }
                }
            } else {
                println!("[INFO refresh_account_info] 无 jwt_token，跳过路径 3");
            }
        }

        // ── 汇总：有 user_info 就完整更新，否则至少 entitlement 验证通过 ──
        if user_info.is_none() && !entitlement_ok {
            return Err(anyhow!(
                "刷新账号信息失败：{}\n账号 id={}，cookies.len={}，oauth_enabled={}，has_refresh={}，jwt_token={}",
                last_err.unwrap_or_else(|| "所有路径均失败".to_string()),
                account_id,
                account.cookies.len(),
                account.oauth_enabled,
                account.oauth_refresh_token.is_some(),
                account.jwt_token.as_ref().map(|t| t.len()).unwrap_or(0)
            ));
        }

        let mut updated = account.clone();

        // GetUserInfo 路径（cookies）→ 完整展示字段更新
        if let Some(info) = user_info {
            if !info.screen_name.is_empty() && info.screen_name != updated.name {
                updated.name = info.screen_name.clone();
            }
            if !info.avatar_url.is_empty() && info.avatar_url != updated.avatar_url {
                updated.avatar_url = info.avatar_url.clone();
            }
            if info.non_plain_text_mobile.is_some() {
                updated.mobile = info.non_plain_text_mobile.clone();
            }
            if !info.last_login_time.is_empty() {
                updated.last_login_time = Some(info.last_login_time.clone());
            }
            if !info.description.is_empty() {
                updated.description = Some(info.description.clone());
            }
            if !info.region.is_empty() {
                updated.region = info.region.clone();
            }
        }

        // entitlement 路径 → user_id 可能从 entitlement 拿到（OAuth 账号常见情况）
        if let Some(euid) = entitlement_user_id {
            if !euid.is_empty() && euid != updated.user_id {
                println!(
                    "[INFO refresh_account_info] entitlement user_id {} → {}",
                    updated.user_id, euid
                );
                updated.user_id = euid;
            }
        }

        updated.updated_at = chrono::Utc::now().timestamp();

        if let Some(a) = self.store.accounts.iter_mut().find(|a| a.id == account_id) {
            *a = updated.clone();
        }
        self.save_store()?;

        Ok(updated)
    }

}

#[cfg(test)]
mod tests;
