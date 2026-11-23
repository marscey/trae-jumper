//! 豆包平台账号：新增校验、资料刷新、额度概览、消耗流水。
//!
//! 豆包账号为纯 Cookie 会话，不参与 Trae 客户端切换 / Token 续签 / 机器码绑定 /
//! Trae 签到虚拟设备档案。签到（P3）端点待客户端抓包后在本模块预留插槽。

use super::*;

/// 拉取 D2 概览并合并 D3 分组明细额度包；D3 失败不阻断概览展示。
async fn fetch_quota_view(client: &DoubaoApiClient) -> Result<DoubaoQuotaView> {
    let overview = client.subscription_overview().await?;
    let mut view = DoubaoQuotaView::from(overview);
    if let Ok(detail) = client.quota_balance_detail().await {
        view.merge_detail(detail);
    }
    Ok(view)
}

/// 判断是否为豆包网关的“系统错误”（710010702）。
/// 实测：网页端 Cookie 调客户端专属活动接口恒定返回该码。
fn is_doubao_system_error(e: &anyhow::Error) -> bool {
    let s = e.to_string();
    s.contains("710010702") || s.contains("系统错误")
}

/// 带客户端登录态回退的签到调用。
///
/// 1. 先用账号自身 Cookie 调 D10；
/// 2. 若返回 710010702（签到活动仅对桌面客户端登录态开放），
///    读取本机豆包客户端登录态；
/// 3. **仅当客户端登录账号 user_id 与账号一致**时才改用客户端 Cookie 重试，
///    避免给不同账号错签。
async fn checkin_with_fallback(account: &Account) -> Result<crate::api::doubao_types::DoubaoCheckinData> {
    let client = DoubaoApiClient::new(&account.cookies)?;
    let first = client.checkin().await;
    let err = match first {
        Ok(data) => return Ok(data),
        Err(e) => e,
    };
    if !is_doubao_system_error(&err) {
        return Err(err);
    }

    let login = crate::doubao_client::read_client_login().map_err(|ce| {
        anyhow!(
            "豆包签到活动仅对桌面客户端登录账号开放；本机未读取到豆包客户端登录态（{ce}）"
        )
    })?;
    if login.user_id != account.user_id {
        return Err(anyhow!(
            "豆包签到活动仅对桌面客户端登录账号开放：当前账号 user_id={} 不是本机豆包客户端登录账号（{}）。请用「从豆包客户端导入」添加该账号。",
            account.user_id,
            login.user_id
        ));
    }

    println!("[INFO] 豆包签到回退到客户端登录态: account='{}'", account.name);
    DoubaoApiClient::new(&login.cookies)?.checkin().await
}

impl AccountManager {
    /// 校验账号属于豆包平台，否则报错。
    pub(super) fn require_doubao(&self, account: &Account) -> Result<()> {
        if account.platform != AccountPlatform::Doubao {
            return Err(anyhow!("该操作仅支持豆包账号"));
        }
        Ok(())
    }

    /// 校验账号属于 Trae 平台：豆包账号不参与客户端切换 / Token 续签 / 签到等 Trae 专属链路。
    pub(super) fn require_trae(&self, account: &Account) -> Result<()> {
        if account.platform == AccountPlatform::Doubao {
            return Err(anyhow!("豆包账号不支持该操作（仅 Trae 账号可切换/续签/签到）"));
        }
        Ok(())
    }

    /// 通过 Cookie 新增豆包账号：D1 校验身份 → 去重 → 拉一次概览快照。
    ///
    /// 概览拉取失败不阻断新增（Cookie 有效即可入库，额度数据下次刷新再补）。
    pub async fn add_doubao_account(&mut self, cookies: String) -> Result<Account> {
        let client = DoubaoApiClient::new(&cookies)?;
        let info = client.get_account_info().await?;

        let user_id = info.normalized_user_id();
        if user_id.is_empty() {
            return Err(anyhow!("未能从豆包账号信息中解析出 user_id，请确认 Cookie 有效"));
        }

        // 去重：同平台同 user_id 视为已存在
        if self
            .store
            .accounts
            .iter()
            .any(|a| a.platform == AccountPlatform::Doubao && a.user_id == user_id)
        {
            return Err(anyhow!("该豆包账号已存在"));
        }

        // 顺手拉一次概览作为初始快照（失败不阻断）
        let snapshot = match fetch_quota_view(&client).await {
            Ok(view) => Some(view),
            Err(e) => {
                println!("[WARN] 豆包概览拉取失败（不阻断新增）: {}", e);
                None
            }
        };

        let fallback_name = format!("Doubao_{}", &user_id[..8.min(user_id.len())]);
        let name = if info.display_name().is_empty() {
            fallback_name
        } else {
            info.display_name()
        };

        let mut account = Account::new_doubao(name, info.mobile.clone(), cookies, user_id, snapshot);
        account.email = info.email.clone().unwrap_or_default();
        account.avatar_url = info.avatar_url.clone().unwrap_or_default();
        account.region = "CN".to_string();
        account.login_source = AccountLoginSource::ManualToken;
        account.last_cookie_renewal_at = Some(chrono::Utc::now().timestamp());

        self.store.accounts.push(account.clone());

        if self.store.active_account_id.is_none() {
            self.store.active_account_id = Some(account.id.clone());
        }

        self.save_store()?;
        println!(
            "[INFO] 新增豆包账号成功: name='{}' user_id='{}' id='{}'",
            account.name, account.user_id, account.id
        );
        Ok(account)
    }

    /// 刷新豆包账号资料（D1）+ 额度快照（D2）。
    pub async fn refresh_doubao_info(&mut self, account_id: &str) -> Result<Account> {
        let account = self.get_account(account_id)?;
        self.require_doubao(&account)?;

        let client = DoubaoApiClient::new(&account.cookies)?;
        let info = client.get_account_info().await?;

        // 概览失败时保留旧快照，不覆盖
        let snapshot = match fetch_quota_view(&client).await {
            Ok(view) => Some(view),
            Err(e) => {
                println!("[WARN] 豆包概览刷新失败，保留旧快照: {}", e);
                account.doubao_snapshot.clone()
            }
        };

        let plan_name = snapshot
            .as_ref()
            .map(|s| s.plan_name.clone())
            .unwrap_or_else(|| account.plan_type.clone());

        if let Some(acc) = self.store.accounts.iter_mut().find(|a| a.id == account_id) {
            if !info.display_name().is_empty() {
                acc.name = info.display_name();
            }
            if let Some(avatar) = info.avatar_url.clone() {
                acc.avatar_url = avatar;
            }
            acc.mobile = info.mobile.clone();
            acc.email = info.email.clone().unwrap_or_default();
            acc.plan_type = plan_name;
            acc.doubao_snapshot = snapshot;
            acc.updated_at = chrono::Utc::now().timestamp();
        }
        self.save_store()?;

        self.get_account(account_id)
    }

    /// 拉取豆包额度概览（D2），并持久化快照。
    pub async fn get_doubao_quota(&mut self, account_id: &str) -> Result<DoubaoQuotaView> {
        let account = self.get_account(account_id)?;
        self.require_doubao(&account)?;

        let client = DoubaoApiClient::new(&account.cookies)?;
        let view = fetch_quota_view(&client).await?;

        if let Some(acc) = self.store.accounts.iter_mut().find(|a| a.id == account_id) {
            acc.doubao_snapshot = Some(view.clone());
            acc.plan_type = view.plan_name.clone();
            acc.updated_at = chrono::Utc::now().timestamp();
        }
        self.save_store()?;

        Ok(view)
    }

    /// 拉取豆包消耗流水（D4）。
    pub async fn get_doubao_timeline(
        &mut self,
        account_id: &str,
        category: Option<i32>,
        cursor: Option<String>,
        page_size: Option<i32>,
    ) -> Result<DoubaoTimelineView> {
        let account = self.get_account(account_id)?;
        self.require_doubao(&account)?;

        let client = DoubaoApiClient::new(&account.cookies)?;
        let data = client.usage_timeline(category, cursor, page_size).await?;
        Ok(DoubaoTimelineView::from(data))
    }

    // ============ 豆包签到（D10：状态查询 + Claim 一体化） ============

    /// 查询豆包签到状态。
    ///
    /// ⚠️ **副作用警告**：D10 `/alice/ug/campaign/landing/checkin/` 接口同时承担
    /// "查询当前签到状态" 和 "执行签到 claim" 两个职责。对未签到账号，
    /// 调用本函数会**实际执行签到并发放额度**（幂等；已签到账号重复调用安全，仅返回状态）。
    /// 如需严格区分，暂需在后端先调 `doubao_checkin_status_all` 看返回，
    /// 再对 `checked_in=false` 的账号调 claim。
    pub async fn doubao_checkin_status(&mut self, account_id: &str) -> Result<DoubaoCheckinView> {
        let account = self.get_account(account_id)?;
        self.require_doubao(&account)?;

        let data = checkin_with_fallback(&account).await?;
        Ok(DoubaoCheckinView::from(data))
    }

    /// 执行豆包签到（D10 checkin 接口本身即 claim，幂等）。
    ///
    /// 返回签到状态视图（含 checked_in / 奖励额度 / 连续天数）。
    pub async fn doubao_checkin(&mut self, account_id: &str) -> Result<DoubaoCheckinView> {
        let account = self.get_account(account_id)?;
        self.require_doubao(&account)?;

        let data = checkin_with_fallback(&account).await?;
        let view = DoubaoCheckinView::from(data);

        println!(
            "[INFO] 豆包签到: account='{}' checked_in={} reward={} continuous={}",
            account.name, view.checked_in, view.reward_amount, view.continuous_days
        );
        Ok(view)
    }

    /// 从本机豆包桌面客户端导入当前登录账号。
    ///
    /// 客户端登录态 Cookie 含 `msToken` / `tt_scid`，是签到活动（客户端专属）的前提。
    /// 已存在同 user_id 的豆包账号时更新其 Cookie（刷新客户端登录态）。
    pub async fn doubao_import_from_client(&mut self) -> Result<Account> {
        #[cfg(debug_assertions)]
        println!("[DEBUG][doubao-import] 开始从豆包客户端读取登录态...");
        let login = crate::doubao_client::read_client_login().map_err(|e| {
            #[cfg(debug_assertions)]
            println!("[DEBUG][doubao-import] read_client_login 失败: {}", e);
            e
        })?;
        println!(
            "[DEBUG][doubao-import] 读成功: user_id={}, cookie_len={}",
            login.user_id, login.cookies.len()
        );

        let client = DoubaoApiClient::new(&login.cookies)?;
        let info = client.get_account_info().await.map_err(|e| {
            #[cfg(debug_assertions)]
            println!("[DEBUG][doubao-import] get_account_info 失败: {}", e);
            e
        })?;
        println!(
            "[DEBUG][doubao-import] get_account_info: name={}, user_id={}",
            info.display_name(),
            info.normalized_user_id()
        );
        let user_id = info.normalized_user_id();
        if user_id.is_empty() {
            return Err(anyhow!("未能从豆包客户端登录态解析出 user_id"));
        }

        // 已存在 → 更新 Cookie
        if let Some(existing) = self
            .store
            .accounts
            .iter_mut()
            .find(|a| a.platform == AccountPlatform::Doubao && a.user_id == user_id)
        {
            existing.cookies = login.cookies.clone();
            existing.last_cookie_renewal_at = Some(chrono::Utc::now().timestamp());
            existing.updated_at = chrono::Utc::now().timestamp();
            existing.login_source = AccountLoginSource::ClientImport;
            if !info.display_name().is_empty() {
                existing.name = info.display_name();
            }
            if let Some(avatar) = info.avatar_url.clone() {
                existing.avatar_url = avatar;
            }
            let id = existing.id.clone();
            self.save_store()?;
            println!("[INFO] 已从豆包客户端刷新登录态: user_id='{}'", user_id);
            return self.get_account(&id);
        }

        // 新增
        let snapshot = match fetch_quota_view(&client).await {
            Ok(view) => Some(view),
            Err(e) => {
                println!("[WARN] 豆包概览拉取失败（不阻断导入）: {}", e);
                None
            }
        };

        let fallback_name = format!("Doubao_{}", &user_id[..8.min(user_id.len())]);
        let name = if info.display_name().is_empty() {
            fallback_name
        } else {
            info.display_name()
        };

        let mut account = Account::new_doubao(name, info.mobile.clone(), login.cookies, user_id, snapshot);
        account.email = info.email.clone().unwrap_or_default();
        account.avatar_url = info.avatar_url.clone().unwrap_or_default();
        account.region = "CN".to_string();
        account.login_source = AccountLoginSource::ClientImport;
        account.last_cookie_renewal_at = Some(chrono::Utc::now().timestamp());

        self.store.accounts.push(account.clone());
        if self.store.active_account_id.is_none() {
            self.store.active_account_id = Some(account.id.clone());
        }
        self.save_store()?;

        println!(
            "[INFO] 从豆包客户端导入账号成功: name='{}' user_id='{}' id='{}'",
            account.name, account.user_id, account.id
        );
        Ok(account)
    }

    /// 批量查询所有豆包账号签到状态（串行，接口幂等）。
    ///
    /// 返回 `(account_id, Option<CheckinView>)` 列表，失败账号状态为 None。
    pub async fn doubao_checkin_status_all(&mut self) -> Vec<(String, Option<DoubaoCheckinView>)> {
        let doubao_ids: Vec<String> = self
            .store
            .accounts
            .iter()
            .filter(|a| a.platform == AccountPlatform::Doubao)
            .map(|a| a.id.clone())
            .collect();

        let mut results = Vec::new();
        for id in doubao_ids {
            let view = match self.doubao_checkin_status(&id).await {
                Ok(v) => Some(v),
                Err(e) => {
                    let acc_name = self
                        .store
                        .accounts
                        .iter()
                        .find(|a| a.id == id)
                        .map(|a| a.name.clone())
                        .unwrap_or_default();
                    println!("[WARN] 豆包签到状态查询失败: account='{}' err={}", acc_name, e);
                    None
                }
            };
            results.push((id, view));
        }
        results
    }

    /// 批量签到所有豆包账号（串行，不并发避免风控）。
    pub async fn doubao_checkin_all(&mut self) -> Result<Vec<DoubaoCheckinView>> {
        let doubao_ids: Vec<String> = self
            .store
            .accounts
            .iter()
            .filter(|a| a.platform == AccountPlatform::Doubao)
            .map(|a| a.id.clone())
            .collect();

        let mut results = Vec::new();
        for id in doubao_ids {
            match self.doubao_checkin(&id).await {
                Ok(view) => results.push(view),
                Err(e) => {
                    let acc_name = self
                        .store
                        .accounts
                        .iter()
                        .find(|a| a.id == id)
                        .map(|a| a.name.clone())
                        .unwrap_or_default();
                    println!("[WARN] 豆包签到失败: account='{}' err={}", acc_name, e);
                }
            }
        }
        Ok(results)
    }
}
