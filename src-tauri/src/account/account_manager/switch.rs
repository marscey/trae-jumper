//! 账号切换与客户端登录态：切换写入、机器码绑定、客户端 storage.json 读取与同步。

use super::*;

impl AccountManager {
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

        // 切换 Trae IDE 到该账号 + 设置系统机器码 — 这些全是同步阻塞操作（含 kill_trae 多轮 sleep），
        // 包进 spawn_blocking 防止阻塞 tokio worker 导致 TJ UI 卡死（用户体感 "崩了"）
        let clean_client_data = self.store.switch_as_new_device;
        let login_info_for_blocking = login_info.clone();
        let machine_id_clone = account.machine_id.clone();
        println!("[DEBUG-crash] switch_account: 准备 spawn_blocking (kill+write+launch)");
        let spawn_blocking_result: std::result::Result<(), anyhow::Error> = tokio::task::spawn_blocking(move || -> std::result::Result<(), anyhow::Error> {
            println!("[DEBUG-crash] spawn_blocking: 进入, 开始 switch_trae_account");
            crate::machine::switch_trae_account(
                &login_info_for_blocking,
                machine_id_clone.as_deref(),
                clean_client_data,
            )?;
            println!("[DEBUG-crash] spawn_blocking: switch_trae_account 完成, 开始 set_machine_guid");
            if let Some(mid) = &machine_id_clone {
                match crate::machine::set_machine_guid(mid) {
                    Ok(_) => println!("[INFO] 已切换系统机器码: {}", mid),
                    Err(e) => println!("[WARN] 切换系统机器码失败（可能需要管理员权限）: {}", e),
                }
            }
            println!("[DEBUG-crash] spawn_blocking: 全部完成");
            Ok(())
        })
        .await
        .map_err(|e| anyhow!("阻塞任务被取消: {}", e))?;
        spawn_blocking_result?;

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

    /// 从 Trae 客户端 storage.json 读取当前登录账号信息（userId + token + self_managed 标志）。
    ///
    /// 旧版客户端在 `iCubeAuthInfo://icube.cloudide` 存完整登录信息（含 token）。
    /// 新版客户端（2.3.87413+）的 `iCubeAuthInfo://icube-dc:<device_id>` 只存设备 EC 密钥对（不含登录 token），
    /// 但仍在 `iCubeAuthInfo://usertag` 记录当前登录的 user_id。
    /// 此时仅能识别登录身份（用于活跃账号判定），无法同步 token。
    ///
    /// 不传 variant 时读取当前目标客户端（`trae_app::current()`），
    /// 传 variant 时读取该指定客户端（用于多客户端活跃检测）。
    pub(super) fn read_client_login(&self, variant: Option<&crate::trae_app::TraeAppVariant>) -> Result<ClientLogin> {
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

}
