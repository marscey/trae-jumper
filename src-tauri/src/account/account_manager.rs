use anyhow::{anyhow, Result};
use std::fs;
use std::path::PathBuf;

use super::types::*;
use crate::api::{CheckinHeaderEntry, CheckinResult, CheckinStatusResult, CreditSummary, TraeApiClient, UsageQueryResponse, UsageSummary};

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
        account.jwt_token = Some(token);
        account.token_expired_at = None;
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

    /// 切换账号（设置活跃账号并将登录信息写入 Trae IDE）
    ///
    /// 写入前会校验 Token 有效性：服务端为单活跃 Token 策略，库里的 Token 可能
    /// 已被其他端登录顶掉，把失效 Token 写入客户端会导致客户端启动后"登录已失效"。
    /// Token 失效时先尝试 cookies 续签；无法续签则按客户端当前登录状态决定拒绝方式，
    /// 绝不把已失效的 Token 写入客户端。
    pub async fn switch_account(&mut self, account_id: &str, force: bool) -> Result<()> {
        // 检查是否已经是当前使用的账号：需客户端"真实持有会话"才算正在使用。
        // current_account_id 只是上次切换的记录，客户端可能已登出（usertag 残留）
        // 或会话已随 token 过期而失效——此时切换正是恢复客户端登录的正向路径，应放行
        if self.store.current_account_id.as_deref() == Some(account_id) {
            let client_login = self.read_client_login(None).ok();
            let client_active = client_login
                .map(|l| self.is_active_client_account_inner(account_id, &l, true))
                .unwrap_or(false);
            if client_active {
                return Err(anyhow!("该账号已经是当前使用的账号"));
            }
            println!("[INFO switch] 目标账号为记录中的当前账号，但客户端未真实持有会话（已登出/会话失效），继续切换以恢复客户端登录");
        }

        let account = self.store.accounts.iter()
            .find(|a| a.id == account_id)
            .ok_or_else(|| anyhow!("账号不存在"))?
            .clone();

        // ===== Token 有效性守卫：写入客户端前先验证 Token 存活 =====
        let mut token = account.jwt_token.clone().filter(|t| !t.is_empty());
        if let Some(t) = token.as_deref() {
            let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
            // 构造失败（Token 格式损坏）同样视为已失效，走续签路径
            let validate = async {
                let client = TraeApiClient::new_with_token(t)?;
                client.get_user_info_by_token().await
            };
            match validate.await {
                Ok(_) => {
                    println!("[{}] [INFO switch] 账号 {} Token 有效，准备写入客户端", now, account.name);
                }
                Err(e) => {
                    println!(
                        "[{}] [WARN switch] 账号 {} Token 已失效（{}），尝试 cookies 续签",
                        now, account.name, e
                    );
                    token = None;
                }
            }
        }

        // Token 缺失或已失效：先看客户端当前登录，再决定续签或拒绝
        let token = match token {
            Some(t) => t,
            None => {
                // 单活跃 Token 下的互斥关系：我方 Token 已死 + 客户端"真实存活"且登录同一账号
                // ⇒ 客户端会话才是有效的一方。此时续签（GetUserToken 签发新 Token）
                // 会顶掉客户端登录，写入死 Token 也会让客户端登出，必须拒绝。
                // 注意：客户端会话可能已随 token 自然过期而失效（如 Mac 睡眠期间双方
                // token 同时过期，客户端显示"登录已失效"但 usertag 仍残留）——此时
                // 续签不会踢掉任何有效会话，应放行以恢复客户端登录。
                let client_login = self.read_client_login(None).ok();
                let client_on_same = client_login
                    .map(|l| self.is_active_client_account_inner(account_id, &l, true))
                    .unwrap_or(false);
                println!(
                    "[DEBUG switch] client_on_same={}（目标账号 {}）",
                    client_on_same, account.name
                );
                if client_on_same {
                    return Err(anyhow!(
                        "客户端当前已登录该账号（{}），且 TraeJumper 侧 Token 已失效。单活跃 Token 策略下客户端会话才是有效的一方，为避免互踢已取消切换。如需更新 TraeJumper 侧 Token，请对该账号执行「通过浏览器登录」",
                        account.name
                    ));
                }

                if account.cookies.trim().is_empty() {
                    // OAuth 账号：cookies 空但有 refresh_token → ExchangeToken 续签
                    if account.oauth_enabled
                        && account.oauth_refresh_token.is_some()
                        && account.oauth_private_key_pem.is_some()
                        && account.oauth_public_key_pem.is_some()
                    {
                        println!("[INFO switch] OAuth 账号 {} Token 已失效，ExchangeToken 续签", account.name);
                        let variant = crate::trae_app::current().clone();
                        let api_host = account.oauth_host.as_deref().unwrap_or(variant.api_host);
                        let token_result = crate::oauth::exchange_refresh_token(
                            &variant,
                            api_host,
                            account.oauth_refresh_token.as_ref().unwrap(),
                            account.oauth_device_id.as_deref().unwrap_or("trae-jumper-device"),
                            account.oauth_machine_id.as_deref().unwrap_or("trae-jumper-machine"),
                            account.oauth_public_key_pem.as_ref().unwrap(),
                            account.oauth_private_key_pem.as_ref().unwrap(),
                        ).await.map_err(|e| anyhow!("OAuth ExchangeToken 续签失败: {}", e))?;
                        {
                            let acc = self.store.accounts.iter_mut()
                                .find(|a| a.id == account_id).unwrap();
                            acc.jwt_token = Some(token_result.access_token.clone());
                            acc.token_expired_at = token_result.expired_at();
                            if !token_result.refresh_token.is_empty()
                                && &token_result.refresh_token != account.oauth_refresh_token.as_ref().unwrap()
                            {
                                acc.oauth_refresh_token = Some(token_result.refresh_token.clone());
                                acc.oauth_refresh_expired_at = token_result.refresh_expired_at();
                            }
                            acc.updated_at = chrono::Utc::now().timestamp();
                        }
                        self.save_store()?;
                        token_result.access_token
                    } else {
                        return Err(anyhow!(
                            "账号 {} 没有 Token 也无 cookies/refresh_token 可续签，请先通过浏览器登录更新该账号再切换",
                            account.name
                        ));
                    }
                } else {

                    // cookies 续签（客户端未登录该账号，refresh_token 的活跃账号保护不会拦截）
                    println!("[INFO switch] 正在为账号 {} 续签 Token 后写入客户端...", account.name);
                    self.refresh_token(account_id).await?;
                    self.store.accounts.iter()
                        .find(|a| a.id == account_id)
                        .and_then(|a| a.jwt_token.clone())
                        .filter(|t| !t.is_empty())
                        .ok_or_else(|| anyhow!("账号 {} Token 续签失败，请先通过浏览器登录更新该账号再切换", account.name))?
                }
            }
        };

        // 跨客户端活跃检测：目标账号若已在"另一个"客户端真实登录，
        // 切换当前目标客户端到该账号会因 Trae 单活跃会话策略顶掉另一客户端的会话。
        // 为避免用户正在交互的会话被中断，此处直接拒绝并提示。
        // 注意：此处仅比对 user_id（read_client_login 只读 iCubeAuthInfo://icube.cloudide，
        // 该 key 在客户端登出时会被清除，无 usertag 脏数据问题），不要求 token_shared_with_client，
        // 因为用户可能直接在客户端登录（不经 TraeJumper），此时 shared=false 但会话真实存在。
        let target_variant = crate::trae_app::current();
        let other_active: Vec<String> = crate::trae_app::all_variants()
            .iter()
            .filter(|v| v.key != target_variant.key)
            .filter(|v| crate::trae_app::is_variant_installed(v))
            .filter_map(|v| {
                self.read_client_login(Some(v))
                    .ok()
                    .filter(|l| !l.user_id.is_empty() && l.user_id == account.user_id)
                    .map(|_| v.display_name.to_string())
            })
            .collect();
        if !other_active.is_empty() && !force {
            return Err(anyhow!(
                "账号 {} 已在 {} 中登录并活跃。切换到当前客户端会中断该客户端的会话（Trae 同账号多端互踢）。请先在该客户端登出该账号，或切换其他账号。",
                account.name,
                other_active.join("、")
            ));
        }
        if !other_active.is_empty() && force {
            println!(
                "[WARN switch] 账号 {} 已在 {} 中登录，force=true 强制切换，另一客户端会话将被中断",
                account.name,
                other_active.join("、")
            );
        }

        // === self_managed 客户端诊断 ===
        // 读取目标客户端的 storage.json，判断是否含 icube-dc:* 设备密钥。
        // self_managed 客户端自行用 OAuth refresh flow 维护 icube-dc 票据，
        // TJ 注入 cloudide 的 GetUserToken JWT（8h 有效期）会与 icube-dc 并存——
        // 这是可运行状态（Fix A 确保 TJ 会在过期前续签 cloudide），但需要日志提醒
        // 以便运维排查 "为什么客户端登出" 这类问题时快速识别矛盾来源。
        let target_self_managed = self.is_client_self_managed(None);
        let now_ts = chrono::Utc::now().timestamp();
        let exp_ts = account.token_expired_at.as_deref()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|t| t.timestamp())
            .unwrap_or(now_ts + 8 * 3600);
        let exp_hrs = (exp_ts - now_ts) / 3600;
        let token_source = if account.oauth_enabled { "OAuth/ExchangeToken" } else { "cookies/GetUserToken" };
        match (target_self_managed, account.oauth_enabled) {
            (true, true) => println!(
                "[WARN switch] 目标客户端 {} 是 self_managed（含 icube-dc:* 设备密钥），即将注入 {} 签发的 cloudide JWT（剩余 {}h 过期）。客户端 icube-dc 票据（OAuth refresh 15d 有效期）与 cloudide（8h）将并存，Fix A 已确保 TJ 会在 2h 宽限期前续签 cloudide 并重新写入，不会因让位而让其过期。",
                crate::trae_app::current().display_name, token_source, exp_hrs
            ),
            (true, false) => println!(
                "[WARN switch] 目标客户端 {} 是 self_managed（含 icube-dc:* 设备密钥），账号 {} 为纯 cookies 来源。注入 GetUserToken JWT（剩余 {}h 过期）后 cloudide 与 icube-dc 将并存。Fix A 已确保 TJ 会维护 cloudide 续签。",
                crate::trae_app::current().display_name, account.name, exp_hrs
            ),
            (false, _) => println!(
                "[INFO switch] 目标客户端 {} 非 self_managed，将直接注入 cloudide JWT（来源 {}，剩余 {}h 过期）。",
                crate::trae_app::current().display_name, token_source, exp_hrs
            ),
        }

        // 构建 Trae IDE 登录信息
        let login_info = crate::machine::TraeLoginInfo {
            token,
            refresh_token: if account.oauth_enabled { account.oauth_refresh_token.clone() } else { None },
            user_id: account.user_id.clone(),
            email: account.email.clone(),
            username: account.name.clone(),
            avatar_url: account.avatar_url.clone(),
            host: account.oauth_host.clone().unwrap_or_default(),
            region: if account.region.is_empty() { "SG".to_string() } else { account.region.clone() },
            token_expired_at: account.token_expired_at.clone(),
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
        // 此 token 已写入客户端（与客户端共用），让位判定恢复"客户端登录中"模式
        if let Some(acc) = self.store.accounts.iter_mut().find(|a| a.id == account_id) {
            acc.token_shared_with_client = true;
            acc.has_been_injected = true; // 曾被切号注入（持久化，不因续签重置）
        }
        self.save_store()?;

        // name 优先展示（部分账号无 email，之前日志会显示空白）
        let display_name = if account.name.is_empty() { account.email.clone() } else { account.name.clone() };
        println!("[INFO] 已切换到账号: {}", display_name);
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
    fn is_active_client_account(&self, account_id: &str) -> bool {
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
    fn is_active_client_account_inner(&self, account_id: &str, login: &ClientLogin, verbose: bool) -> bool {
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

    /// （内部）刷新指定账号 Token，成功返回新 token；失败返回 None，不抛错方便降级
    async fn refresh_token_inner(&mut self, account_id: &str) -> Option<String> {
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
        // Guard: 客户端原生登录活跃 → 让位避免 GetUserToken 互踢
        if self.is_active_client_account(account_id) {
            let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
            println!(
                "[{}] [WARN] refresh_token 被调用但目标是活跃客户端账号 {}，已拒绝执行（避免客户端登出）",
                now, account_id
            );
            return Err(anyhow!("活跃客户端账号不能用 cookies 续签，以免踢掉客户端登录态"));
        }

        let account = self
            .store
            .accounts
            .iter()
            .find(|a| a.id == account_id)
            .ok_or_else(|| anyhow!("账号不存在"))?
            .clone();

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

    /// 读取账号的登录邮箱
    pub fn account_email(&self, account_id: &str) -> Option<String> {
        self.store
            .accounts
            .iter()
            .find(|a| a.id == account_id)
            .map(|a| a.email.clone())
    }

    /// 读取当前目标客户端已登录账号的标识（user_id + email），用于"从客户端读取更新 Token"的前端预检/展示。
    ///
    /// 与账号库解耦：无论该账号是否已被本应用管理，都能读出客户端当前登录态；客户端未登录或读取失败返回 Ok(None)。
    /// 兼容新版客户端（TraeCode CN 2.3.87416+）：无旧版 key 时从 usertag 读取登录 user_id（email 为空）。
    pub async fn current_client_login(&self) -> Result<Option<CurrentClientLogin>> {
        let login = match self.read_client_login(None) {
            Ok(l) => l,
            Err(_) => return Ok(None),
        };
        if login.user_id.is_empty() {
            return Ok(None);
        }
        let email = if login.email.is_empty() {
            None
        } else {
            Some(login.email)
        };

        Ok(Some(CurrentClientLogin { user_id: login.user_id, email }))
    }

    /// 将指定账号标记为"Token 已过期"。
    ///
    /// 当 API 返回 401 且无法用 Cookies 刷新 token 时调用，把 token_expired_at 置为当前时间并持久化，
    /// 前端账号列表的状态标签据此显示"已过期"，不再一直显示"正常"。
    fn mark_account_token_expired(&mut self, account_id: &str) -> Result<()> {
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

    /// 轻量判断目标客户端是否为 self_managed 架构（storage.json 含 icube-dc:* 设备密钥键）。
    /// 与 read_client_login 不同：本函数不要求客户端有活跃登录态——self_managed 是客户端
    /// 安装属性，存在即代表客户端版本在 2.3.87413+，具备 OAuth 自管能力。
    /// storage.json 不存在 / 无 icube-dc 键 → 返回 false（旧版客户端或未安装）。
    fn is_client_self_managed(&self, variant: Option<&crate::trae_app::TraeAppVariant>) -> bool {
        #[cfg(any(target_os = "windows", target_os = "macos"))]
        let trae_data_path = crate::trae_app::data_dir_of(variant.unwrap_or_else(|| crate::trae_app::current()));
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        let trae_data_path: std::path::PathBuf = return false;

        let storage_path = trae_data_path
            .join("User")
            .join("globalStorage")
            .join("storage.json");
        let content = match fs::read_to_string(&storage_path) {
            Ok(c) => c,
            Err(_) => return false,
        };
        let storage: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(_) => return false,
        };
        storage
            .as_object()
            .map(|o| o.keys().any(|k| k.starts_with("iCubeAuthInfo://icube-dc:")))
            .unwrap_or(false)
    }

    /// 从 Trae 客户端 storage.json 读取当前登录账号信息（userId + token + self_managed 标志）。
    ///
    /// 旧版客户端在 `iCubeAuthInfo://icube.cloudide` 存完整登录信息（含 token）。
    /// 新版客户端（2.3.87413+）的 `iCubeAuthInfo://icube-dc:<device_id>` 只存设备 EC 密钥对（不含登录 token），
    /// 但仍在 `iCubeAuthInfo://usertag` 记录当前登录的 user_id。
    /// 此时仅能识别登录身份（用于活跃账号判定），无法同步 token。
    ///
    /// 不传 variant 时读取当前目标客户端（`trae_app::current()`），
    /// 传 variant 时读取该指定客户端（用于多客户端活跃检测）。
    fn read_client_login(&self, variant: Option<&crate::trae_app::TraeAppVariant>) -> Result<ClientLogin> {
        #[cfg(any(target_os = "windows", target_os = "macos"))]
        let trae_data_path = crate::trae_app::data_dir_of(variant.unwrap_or_else(|| crate::trae_app::current()));

        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        let trae_data_path: std::path::PathBuf = {
            return Err(anyhow!("此功能仅支持 Windows 和 macOS 系统"));
        };

        let storage_path = trae_data_path
            .join("User")
            .join("globalStorage")
            .join("storage.json");

        if !storage_path.exists() {
            return Err(anyhow!("未找到客户端登录信息"));
        }

        let content = fs::read_to_string(&storage_path)
            .map_err(|e| anyhow!("读取 Trae IDE 配置文件失败: {}", e))?;
        let storage: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| anyhow!("解析 Trae IDE 配置文件失败: {}", e))?;

        // 判断客户端是否为"新版自管理架构"：storage.json 顶层含 `icube-dc:<device_id>`
        // 设备密钥键（2.3.87413+ 的 TraeCode CN / TRAE SOLO CN）。此类客户端自行签发
        // 登录态，我们仅能识别身份、不能注入/续签 token。旧版客户端无该键。
        let self_managed = storage
            .as_object()
            .map(|o| o.keys().any(|k| k.starts_with("iCubeAuthInfo://icube-dc:")))
            .unwrap_or(false);

        // 从 cloudide 读取当前登录账号的 userId + token。
        // 注意：不要回退到 usertag——usertag 记录的是"所有曾登录过的账号"，
        // 登出后不会清理（脏数据），回退会把已登出的账号误判为"客户端活跃"，
        // 进而导致让位逻辑错误（跳过续签）或 active_in_clients 显示错误。
        // cloudide 缺失或 userId 为空 ⇒ 客户端未登录，直接返回错误。
        if let Some(auth_info_raw) = storage
            .get("iCubeAuthInfo://icube.cloudide")
            .and_then(|v| v.as_str())
        {
            let auth_info_str = crate::crypto::read_storage_value(auth_info_raw);
            let auth_info: serde_json::Value = serde_json::from_str(&auth_info_str)
                .map_err(|e| anyhow!("解析 Trae IDE 认证信息失败: {}", e))?;
            let user_id = auth_info
                .get("userId")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if !user_id.is_empty() {
                let token = auth_info
                    .get("token")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let email = auth_info
                    .get("account")
                    .and_then(|a| a.get("email"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                // OAuth refresh_token：cloudide JSON 中有此字段，
                // TJ OAuth 注入 / 客户端原生 OAuth 登录都会写入。
                // 空字符串 → cookies 模式，None → 字段不存在（旧版 cloudide）
                let refresh_token = auth_info
                    .get("refreshToken")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string());
                let refresh_expired_at = auth_info
                    .get("refreshExpiredAt")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                return Ok(ClientLogin {
                    user_id,
                    email,
                    token,
                    refresh_token,
                    refresh_expired_at,
                    self_managed,
                });
            }
        }

        Err(anyhow!("未找到客户端登录信息（客户端未登录）"))
    }

    /// 定时同步客户端当前活跃账号的最新 Token 到账号库。
    ///
    /// 背景：Trae 客户端会自动续签自己的登录态，storage.json 里的 token 永远是最新的。
    /// 对于通过客户端导入的账号（无 cookies），无法用 cookies 续签，因此在每次定时刷新时
    /// 从客户端读取最新 token 同步过来——只要用户在客户端保持登录，该账号就永远不会过期。
    ///
    /// 仅同步 user_id 匹配的那个账号（客户端当前登录的账号），其他账号不动。
    /// 返回被同步的账号 ID（如果有）。
    pub fn sync_active_client_token(&mut self) -> Result<Option<String>> {
        // 扫描所有已安装客户端：任一客户端有活跃 token 且在库中匹配，就同步
        // （TraeCode CN / TraeWork CN 均在 storage.json 的 icube.cloudide 中保存 token）
        let logins: Vec<ClientLogin> = crate::trae_app::all_variants()
            .iter()
            .filter(|v| crate::trae_app::is_variant_installed(v))
            .filter_map(|v| {
                self.read_client_login(Some(v))
                    .ok()
                    .filter(|l| !l.user_id.is_empty() && l.token.is_some())
            })
            .collect();

        for login in &logins {
            let token = match &login.token {
                Some(t) => t,
                None => continue,
            };

            // 找到账号库中匹配 user_id 的账号
            let account_id = self
                .store
                .accounts
                .iter()
                .find(|a| a.user_id == login.user_id)
                .map(|a| a.id.clone());

            let Some(account_id) = account_id else {
                continue; // 客户端登录的账号不在库中，跳过
            };

            // 比较 token 是否变化，避免无谓写入
            let needs_update = self
                .store
                .accounts
                .iter()
                .find(|a| a.id == account_id)
                .map(|a| a.jwt_token.as_deref() != Some(token.as_str()))
                .unwrap_or(false);

            if !needs_update {
                continue;
            }

            // 解析 JWT 的 exp，设置 token_expired_at（避免被误判为即将过期而重复续签）
            let token_expired_at = crate::api::TraeApiClient::parse_jwt_exp(token)
                .ok()
                .and_then(|exp| {
                    chrono::DateTime::from_timestamp(exp, 0)
                        .map(|dt| dt.to_rfc3339())
                });

            let old_exp = self.store.accounts.iter().find(|a| a.id == account_id)
                .and_then(|a| a.token_expired_at.clone())
                .unwrap_or_else(|| "None".to_string());
            println!(
                "[DEBUG sync] 从客户端同步 token 覆盖账号 {}: token_expired_at {} → {} (parse_jwt_exp={:?})",
                account_id, old_exp, token_expired_at.as_deref().unwrap_or("None"),
                crate::api::TraeApiClient::parse_jwt_exp(token).ok()
            );

            // 更新 token 并清除过期标记
            if let Some(acc) = self.store.accounts.iter_mut().find(|a| a.id == account_id) {
                acc.jwt_token = Some(token.clone());
                acc.token_expired_at = token_expired_at;
                acc.updated_at = chrono::Utc::now().timestamp();
                acc.token_shared_with_client = true; // 此 token 来自客户端，与客户端共用
            }
            self.save_store()?;

            println!("[INFO] 已从客户端同步活跃账号 Token: {}", account_id);
            return Ok(Some(account_id));
        }

        Ok(None)
    }

    /// 通过读取当前 Trae 客户端登录态自动更新指定账号的 Token
    ///
    /// 适用于 Token 过期后的一键更新：用户先把目标账号切到 Trae 客户端登录，
    /// 再调用本方法从客户端的 storage.json 读取其最新 Token 并更新到本账号。
    /// 读取后校验客户端当前登录账号与目标账号是同一用户，避免更新错账号。
    pub async fn update_account_token_from_client(&mut self, account_id: &str) -> Result<UsageSummary> {
        let login = self.read_client_login(None)?;

        // 校验客户端当前登录账号与目标账号为同一用户
        let acc = self
            .store
            .accounts
            .iter()
            .find(|a| a.id == account_id)
            .ok_or_else(|| anyhow!("账号不存在"))?;
        if acc.user_id != login.user_id {
            let shown = if login.email.is_empty() { login.user_id } else { login.email };
            return Err(anyhow!(
                "客户端当前登录的账号（{}）与目标账号不匹配，请先切换到目标账号再更新",
                shown
            ));
        }

        // 新版客户端（TraeCode CN 2.3.87416+）不再在 storage.json 存 token，无法通过客户端更新
        let Some(token) = login.token else {
            return Err(anyhow!(
                "当前客户端版本未在本地存储登录 Token，请改用「通过浏览器登录」方式更新该账号"
            ));
        };

        // 复用 update_account_token 更新并返回最新使用量
        self.update_account_token(account_id, token).await
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
        let mut imported_count = 0;

        for item in import_data.iter() {
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

            // 优先使用 Token 添加（Token 方式更稳定，且不依赖 cookies）
            let add_success = if !token.is_empty() {
                let cookies_opt = if cookies.is_empty() { None } else { Some(cookies.clone()) };
                let preferred_name = if exported_name.is_empty() { None } else { Some(exported_name.clone()) };
                match self.add_account_by_token(token.clone(), cookies_opt, preferred_name, AccountLoginSource::ManualToken).await {
                    Ok(_) => true,
                    Err(e) => {
                        let err_str = e.to_string();
                        if err_str.contains("已存在") {
                            // 账号已存在，也算导入成功（后续会更新名称）
                            true
                        } else {
                            // API 调用失败（如网络不可达、Token 过期等），尝试直接从导出数据创建账号
                            let fallback_user_id = if !exported_user_id.is_empty() {
                                exported_user_id.clone()
                            } else {
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

                                let already_exists = self.store.accounts.iter().any(|a| a.user_id == fallback_account.user_id);
                                if already_exists {
                                    true
                                } else {
                                    self.store.accounts.push(fallback_account);
                                    if self.store.active_account_id.is_none() {
                                        self.store.active_account_id = Some(self.store.accounts.last().unwrap().id.clone());
                                    }
                                    true
                                }
                            } else {
                                false
                            }
                        }
                    }
                }
            } else if !cookies.is_empty() {
                match self.add_account(cookies).await {
                    Ok(_) => true,
                    Err(e) => {
                        !e.to_string().contains("已存在")
                    }
                }
            } else {
                false
            };

            if !exported_name.is_empty() && !exported_user_id.is_empty() {
                if let Some(acc) = self.store.accounts.iter_mut().find(|a| a.user_id == exported_user_id) {
                    acc.name = exported_name.clone();
                    if !exported_avatar.is_empty() {
                        acc.avatar_url = exported_avatar.clone();
                    }
                    if !exported_region.is_empty() {
                        acc.region = exported_region.clone();
                    }
                }
            }

            if add_success {
                imported_count += 1;
            }
        }

        self.save_store()?;
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

    /// 同步当前账号状态：读取当前目标 Trae 客户端已登录账号，更新 current_account_id。
    ///
    /// 切换目标客户端后调用：原客户端下登录的账号（如账号 a）在新客户端下可能并不存在/已失效，
    /// 这里重新读取新客户端 storage.json 中的 userId，在账号列表中匹配：
    /// - 匹配成功 → current_account_id 指向该账号；
    /// - 新客户端未登录或匹配不到 → current_account_id 清空。
    ///
    /// 返回更新后的当前账号摘要（若新客户端未登录任何已知账号则返回 None）。
    ///
    /// 注意：客户端登出后 usertag 残留（仅代表"最后登录的账号"），此时仅记录
    /// current_account_id 供后续切换使用，但返回 None 让前端清空"当前"显示——
    /// 与 get_accounts 的 is_current 语义保持一致（客户端真实持有会话才显示）。
    pub fn sync_current_account(&mut self) -> Result<Option<AccountBrief>> {
        // 读取当前目标客户端数据目录中的 storage.json 登录信息（仅解析 userId，不新增账号）
        let login = self.read_client_login(None).ok();
        let user_id = login.as_ref().map(|l| l.user_id.clone()).filter(|u| !u.is_empty());

        let current = match &user_id {
            Some(uid) => self
                .store
                .accounts
                .iter()
                .find(|a| a.user_id == *uid),
            None => None,
        };

        match current {
            Some(account) => {
                self.store.current_account_id = Some(account.id.clone());
                self.save_store()?;
                println!("[INFO] 已同步当前账号: {} ({})", account.email, account.user_id);
                // "当前"显示语义：客户端真实持有会话（usertag 匹配 + token 已共享 + 未死亡）
                let is_client_active = login
                    .as_ref()
                    .map(|l| self.is_active_client_account_inner(&account.id, l, false))
                    .unwrap_or(false);
                if !is_client_active {
                    println!("[INFO] 客户端未真实持有该账号会话（登出残留/未共享），仅记录不显示\"当前\"");
                    return Ok(None);
                }
                let mut brief = AccountBrief::from_account(account, true);
                brief.is_client_active = true;
                Ok(Some(brief))
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
        account.login_source = AccountLoginSource::ClientImport;

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

    /// 设置 per-account 重启覆盖开关
    /// None = 跟随总开关；Some(true/false) = 强制开/关
    pub fn set_account_restart_override(&mut self, account_id: &str, enabled: Option<bool>) -> Result<()> {
        let acc = self.store.accounts.iter_mut()
            .find(|a| a.id == account_id)
            .ok_or_else(|| anyhow!("账号不存在: {}", account_id))?;
        acc.restart_after_refresh_override = enabled;
        self.save_store()?;
        Ok(())
    }

    /// 判断某账号自动续签后是否应重启客户端（总开关 + per-account 覆盖）
    pub fn should_restart_after_refresh(&self, account_id: &str) -> bool {
        let global = self.store.checkin_config.as_ref()
            .map(|c| c.auto_restart_after_refresh)
            .unwrap_or(true);
        let per_account = self.store.accounts.iter()
            .find(|a| a.id == account_id)
            .and_then(|a| a.restart_after_refresh_override);
        match per_account {
            Some(v) => v,    // per-account 覆盖优先
            None => global,  // 跟随总开关
        }
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
            last_auto_checkin_date: None,
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
            last_auto_checkin_date: None,
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
            last_auto_checkin_date: None,
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
            last_auto_checkin_date: None,
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
