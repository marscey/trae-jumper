//! 豆包（doubao.com）账号体系 API 类型定义。
//!
//! 字段口径来自 `docs/doubao-api/API-INVENTORY.md` 与 `docs/doubao-api/samples/`
//! 实测样本；未在免费样本中观察到的字段一律 `Option` 占位，付费账号样本补全前不臆造。
//!
//! 鉴权模型：纯 Cookie 会话，无 Authorization Bearer/JWT；公共查询参数见
//! `DoubaoApiClient::common_query`。

use serde::{Deserialize, Serialize};

// ============ 通用信封 ============

/// 豆包网关统一信封：`{ code, msg, message, data }`，`code == 0` 视为成功。
#[derive(Debug, Clone, Deserialize)]
pub struct DoubaoEnvelope<T> {
    /// 部分网关（如 D1 passport 接口）不返回 code，缺省按 0（成功）处理。
    #[serde(default)]
    pub code: i32,
    #[serde(default)]
    pub msg: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    pub data: Option<T>,
}

// ============ D1 账户资料 ============

#[derive(Debug, Clone, Deserialize, Default)]
pub struct DoubaoAccountConnect {
    #[serde(default)]
    pub platform: Option<String>,
    #[serde(default)]
    pub platform_screen_name: Option<String>,
    #[serde(default)]
    pub platform_uid: Option<String>,
    #[serde(default)]
    pub open_id: Option<String>,
}

/// `GET https://accounts.doubao.com/passport/account/info/v2/` 的 `data` 平铺结构。
#[derive(Debug, Clone, Deserialize, Default)]
pub struct DoubaoAccountInfo {
    #[serde(default)]
    pub screen_name: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub avatar_url: Option<String>,
    /// 数字型 user_id（部分网关返回 int，统一按字符串读）
    #[serde(default)]
    pub user_id: Option<serde_json::Value>,
    #[serde(default)]
    pub user_id_str: Option<String>,
    #[serde(default)]
    pub sec_user_id: Option<String>,
    #[serde(default)]
    pub mobile: Option<String>,
    #[serde(default)]
    pub country_code: Option<serde_json::Value>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    /// 注册时间（秒级 epoch）
    #[serde(default)]
    pub user_create_time: Option<i64>,
    #[serde(default)]
    pub is_visitor_account: Option<bool>,
    #[serde(default)]
    pub is_blocked: Option<i32>,
    #[serde(default)]
    pub connects: Vec<DoubaoAccountConnect>,
}

impl DoubaoAccountInfo {
    /// 归一化后的稳定 user_id（优先 user_id_str，其次 user_id 转字符串）。
    pub fn normalized_user_id(&self) -> String {
        if let Some(s) = &self.user_id_str {
            if !s.is_empty() {
                return s.clone();
            }
        }
        match &self.user_id {
            Some(v) => v.as_str().map(|s| s.to_string()).or_else(|| v.as_i64().map(|i| i.to_string()))
                .unwrap_or_default(),
            None => String::new(),
        }
    }

    pub fn display_name(&self) -> String {
        self.screen_name
            .clone()
            .or_else(|| self.name.clone())
            .unwrap_or_default()
    }
}

// ============ D2 额度概览 ============

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SubDisplay {
    #[serde(default)]
    pub product_name: Option<String>,
    #[serde(default)]
    pub short_name: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct CurrentSubscription {
    #[serde(default)]
    pub display: Option<SubDisplay>,
    #[serde(default)]
    pub sku_key: Option<String>,
    #[serde(default)]
    pub status: Option<i32>,
    #[serde(default)]
    pub start_time: Option<i64>,
    #[serde(default)]
    pub end_time: Option<i64>,
    #[serde(default)]
    pub product_line: Option<String>,
    #[serde(default)]
    pub subscription_type: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct DoubaoMemberInfo {
    /// 注意：该字段在网关上是 camelCase `hasActiveSubscription`。
    #[serde(default, rename = "hasActiveSubscription")]
    pub has_active_subscription: Option<bool>,
    #[serde(default)]
    pub has_enterprise_subscription: Option<bool>,
    #[serde(default)]
    pub usr_type: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct QuotaSummaryRaw {
    #[serde(default)]
    pub category: Option<i32>,
    /// 带千分位的剩余额度字符串，如 "2,526"
    #[serde(default)]
    pub remaining_amount: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct QuotaItemRaw {
    #[serde(default)]
    pub category: Option<i32>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub end_time: Option<i64>,
    #[serde(default)]
    pub time_type: Option<i32>,
    /// 字符串数值，如 "100" / "74"
    #[serde(default)]
    pub total_amount: Option<String>,
    #[serde(default)]
    pub used_amount: Option<String>,
    #[serde(default)]
    pub used_percent: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct QuotaBalanceOverview {
    #[serde(default)]
    pub total_remaining_amount: Option<String>,
    #[serde(default)]
    pub summaries: Vec<QuotaSummaryRaw>,
    #[serde(default)]
    pub top_items: Vec<QuotaItemRaw>,
    #[serde(default)]
    pub has_more: Option<bool>,
}

/// 窗口额度条目（周期/累计窗口内的总量、已用、百分比）。
#[derive(Debug, Clone, Deserialize, Default)]
pub struct WindowLimitRaw {
    #[serde(default)]
    pub end_time: Option<i64>,
    #[serde(default)]
    pub start_time: Option<i64>,
    #[serde(default)]
    pub item_type: Option<i32>,
    /// 字符串数值，如 "3,200" / "718"
    #[serde(default)]
    pub total_amount: Option<String>,
    #[serde(default)]
    pub used_amount: Option<String>,
    #[serde(default)]
    pub used_percent: Option<i32>,
    #[serde(default)]
    pub window_type: Option<i32>,
    #[serde(default)]
    pub less_than_one_percent: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct WindowGroupRaw {
    #[serde(default)]
    pub feature_group: Option<String>,
    #[serde(default)]
    pub feature_group_name: Option<String>,
    #[serde(default)]
    pub window_limits: Vec<WindowLimitRaw>,
}

/// 额度窗口分组区块（赠送 / 付费 / 企业等）。
#[derive(Debug, Clone, Deserialize, Default)]
pub struct WindowSectionRaw {
    #[serde(default)]
    pub entitlement_count: Option<i32>,
    #[serde(default)]
    pub usage_exhausted: Option<bool>,
    #[serde(default)]
    pub window_limit_groups: Vec<WindowGroupRaw>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct IncentiveEntitlement {
    #[serde(default)]
    pub display: Option<SubDisplay>,
    #[serde(default)]
    pub end_time: Option<i64>,
    #[serde(default)]
    pub sku_key: Option<String>,
    #[serde(default)]
    pub status: Option<i32>,
    #[serde(default)]
    pub subscription_type: Option<i32>,
    #[serde(default)]
    pub total_amount: Option<String>,
    #[serde(default)]
    pub used_amount: Option<String>,
    #[serde(default)]
    pub total_used_percent: Option<i32>,
    #[serde(default)]
    pub is_resettable: Option<bool>,
    #[serde(default)]
    pub is_gift: Option<bool>,
}

/// D2 `/alice/commerce/sale/subscription/overview/` 的 `data`。
#[derive(Debug, Clone, Deserialize, Default)]
pub struct OverviewData {
    #[serde(default)]
    pub current_subscription: Option<CurrentSubscription>,
    #[serde(default)]
    pub member_info: Option<DoubaoMemberInfo>,
    #[serde(default)]
    pub quota_balance_overview: Option<QuotaBalanceOverview>,
    #[serde(default)]
    pub incentive_quota_entitlements: Vec<IncentiveEntitlement>,
    /// 付费包条目；免费样本为空，结构保留为通用 JSON，待付费样本补全。
    #[serde(default)]
    pub entitlements: Vec<serde_json::Value>,
    /// 赠送额度窗口聚合（总量/已用/百分比）。
    #[serde(default)]
    pub incentive_quota_section: Option<WindowSectionRaw>,
    /// 主额度窗口聚合。
    #[serde(default)]
    pub window_limit_section: Option<WindowSectionRaw>,
    /// 企业额度窗口聚合。
    #[serde(default)]
    pub enterprise_window_limit_section: Option<WindowSectionRaw>,
    #[serde(default)]
    pub service_current_time: Option<i64>,
}

// ============ D3 分组明细 ============

#[derive(Debug, Clone, Deserialize, Default)]
pub struct QuotaGroupRaw {
    #[serde(default)]
    pub category: Option<i32>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub items: Vec<QuotaItemRaw>,
}

/// D3 `/alice/commerce/sale/subscription/quota_balance/detail/` 的 `data`。
#[derive(Debug, Clone, Deserialize, Default)]
pub struct DetailData {
    #[serde(default)]
    pub total_remaining_amount: Option<String>,
    #[serde(default)]
    pub summaries: Vec<QuotaSummaryRaw>,
    #[serde(default)]
    pub groups: Vec<QuotaGroupRaw>,
}

// ============ D4 消耗流水 ============

#[derive(Debug, Clone, Deserialize, Default)]
pub struct QuotaSourceRaw {
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub display_text: Option<String>,
    #[serde(default)]
    pub quota_source_code: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TimelineUsageItem {
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub item_id: Option<String>,
    #[serde(default)]
    pub model_display_name: Option<String>,
    #[serde(default)]
    pub occurred_at_ms: Option<i64>,
    #[serde(default)]
    pub quota_source: Option<QuotaSourceRaw>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TimelineEntryRaw {
    #[serde(default)]
    pub entry_type: Option<i32>,
    #[serde(default)]
    pub usage: Option<TimelineUsageItem>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct CategoryTabRaw {
    #[serde(default)]
    pub category: Option<i32>,
    #[serde(default)]
    pub level: Option<i32>,
    #[serde(default)]
    pub selected: Option<bool>,
}

/// D4 `/alice/commerce/usage/timeline/` 的 `data`。
#[derive(Debug, Clone, Deserialize, Default)]
pub struct TimelineData {
    #[serde(default)]
    pub categories: Vec<i32>,
    #[serde(default)]
    pub category_tabs: Vec<CategoryTabRaw>,
    #[serde(default)]
    pub consumption_column_title: Option<String>,
    #[serde(default)]
    pub window_start_time_ms: Option<i64>,
    #[serde(default)]
    pub window_end_time_ms: Option<i64>,
    #[serde(default)]
    pub has_more: Option<bool>,
    /// 游标字段名待付费/翻页样本补全，先按常见 `cursor` 读取。
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub entries: Vec<TimelineEntryRaw>,
}

// ============ D5 订阅状态 ============

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SubscriptionStatusData {
    #[serde(default)]
    pub has_subscribed: Option<bool>,
    #[serde(default)]
    pub is_subscribing: Option<bool>,
}

// ============ D8/D9 重置卡 ============

#[derive(Debug, Clone, Deserialize, Default)]
pub struct CardEntryRaw {
    #[serde(default)]
    pub activity_id: Option<String>,
    #[serde(default)]
    pub card_key: Option<String>,
    #[serde(default)]
    pub create_time: Option<i64>,
    #[serde(default)]
    pub expire_time: Option<i64>,
    #[serde(default)]
    pub grant_id: Option<serde_json::Value>,
    #[serde(default)]
    pub status: Option<i32>,
    #[serde(default)]
    pub source_display_text: Option<String>,
    #[serde(default)]
    pub usable_on_current_device: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct CardListData {
    #[serde(default)]
    pub cards: Vec<CardEntryRaw>,
    #[serde(default)]
    pub service_current_time_ms: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct CardBalanceData {
    #[serde(default)]
    pub available_count: Option<i32>,
    #[serde(default)]
    pub available_count_by_card_key: Option<serde_json::Map<String, serde_json::Value>>,
    #[serde(default)]
    pub earliest_expire_time: Option<i64>,
    #[serde(default)]
    pub has_available_card: Option<bool>,
    #[serde(default)]
    pub hide_entry: Option<bool>,
}

// ============ D10 签到状态 + Claim（一体化） ============

/// D10 `/alice/ug/campaign/landing/checkin/` 的 `data`。
///
/// 注意：此接口**既是状态查询也是 claim**——调用即签到发奖励，幂等（已签到则返回状态）。
#[derive(Debug, Clone, Deserialize, Default)]
pub struct DoubaoCheckinData {
    /// 今日是否已签到
    #[serde(default)]
    pub checked_in: bool,
    /// 签到日期 "YYYY-MM-DD"（已签到时为当天日期）
    #[serde(default)]
    pub checkin_date: Option<String>,
    /// 连续签到天数（含今天）
    #[serde(default)]
    pub continuous_days: Option<i32>,
    /// 本次奖励额度（已签到时为 0 或上次值；未签到但 claim 后为实际奖励）
    #[serde(default)]
    pub reward_amount: Option<i64>,
    /// 奖励码（如 "quota_grant"）
    #[serde(default)]
    pub reward_code: Option<String>,
    /// 奖励状态（3=已发放，0=未发放）
    #[serde(default)]
    pub reward_status: Option<i32>,
    /// 奖励类型（如 "quota_grant"）
    #[serde(default)]
    pub reward_type: Option<String>,
    /// 是否幂等（重复调用是否已发过奖）
    #[serde(default)]
    pub idempotent: Option<bool>,
}

/// 签到归一化视图（返回前端卡片展示）。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DoubaoCheckinView {
    pub checked_in: bool,
    pub checkin_date: Option<String>,
    pub continuous_days: i32,
    pub reward_amount: i64,
    pub reward_code: Option<String>,
    pub reward_type: Option<String>,
}

impl From<DoubaoCheckinData> for DoubaoCheckinView {
    fn from(d: DoubaoCheckinData) -> Self {
        Self {
            checked_in: d.checked_in,
            checkin_date: d.checkin_date,
            continuous_days: d.continuous_days.unwrap_or(0),
            reward_amount: d.reward_amount.unwrap_or(0),
            reward_code: d.reward_code,
            reward_type: d.reward_type,
        }
    }
}

// ============ 归一化展示视图（返回前端） ============

/// 解析带千分位的额度字符串为数值（失败回退 0）。
fn parse_amount(s: Option<&str>) -> f64 {
    s.and_then(|v| v.replace(',', "").parse::<f64>().ok()).unwrap_or(0.0)
}

/// 额度聚合视图：进度条 / 汇总卡片使用的总量、已用、剩余与百分比。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QuotaAggregateView {
    pub total_amount: f64,
    pub used_amount: f64,
    pub left_amount: f64,
    pub used_percent: i32,
    /// 聚合口径：`window`=服务端窗口分组汇总；`packages`=本地按额度包明细求和兜底。
    pub source: String,
}

impl QuotaAggregateView {
    /// 由窗口分组列表聚合；无有效窗口时返回 None。
    fn from_windows(sections: &[Option<&WindowSectionRaw>]) -> Option<Self> {
        let mut total = 0.0;
        let mut used = 0.0;
        let mut any = false;
        for section in sections.iter().flatten() {
            for group in &section.window_limit_groups {
                for w in &group.window_limits {
                    let t = parse_amount(w.total_amount.as_deref());
                    let u = parse_amount(w.used_amount.as_deref());
                    if t > 0.0 {
                        any = true;
                        total += t;
                        used += u;
                    }
                }
            }
        }
        if !any {
            return None;
        }
        used = used.min(total);
        let left = total - used;
        let pct = if total > 0.0 {
            ((used / total) * 100.0).round() as i32
        } else {
            0
        };
        Some(Self {
            total_amount: total,
            used_amount: used,
            left_amount: left,
            used_percent: pct,
            source: "window".to_string(),
        })
    }
}

/// 单个额度包（付费 / 赠送 / 活动奖励等，统一视图）。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QuotaPackageView {
    /// 分组类别：3=赠送，7=付费/其他（其余值原样透传）
    pub category: Option<i32>,
    pub display_name: String,
    #[serde(default)]
    pub sku_key: Option<String>,
    /// 原始字符串额度（带千分位/小数，原样展示）
    pub total_amount_str: String,
    pub used_amount_str: String,
    pub total_amount: f64,
    pub used_amount: f64,
    pub used_percent: i32,
    /// 到期时间（毫秒），0/None 表示未知
    pub end_time_ms: Option<i64>,
    #[serde(default)]
    pub status: Option<i32>,
    #[serde(default)]
    pub subscription_type: Option<i32>,
    #[serde(default)]
    pub is_resettable: Option<bool>,
}

impl QuotaPackageView {
    /// 由 D2 top_items / D3 groups.items 的原始条目构造视图。
    fn from_item_raw(i: &QuotaItemRaw, category: Option<i32>) -> Self {
        Self {
            category,
            display_name: i.display_name.clone().unwrap_or_default(),
            sku_key: None,
            total_amount_str: i.total_amount.clone().unwrap_or_default(),
            used_amount_str: i.used_amount.clone().unwrap_or_default(),
            total_amount: parse_amount(i.total_amount.as_deref()),
            used_amount: parse_amount(i.used_amount.as_deref()),
            used_percent: i.used_percent.unwrap_or(0),
            end_time_ms: i.end_time,
            status: None,
            subscription_type: None,
            is_resettable: None,
        }
    }
}

/// 额度概览归一化视图（P1 卡片 + Dashboard 汇总使用）。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DoubaoQuotaView {
    /// 套餐展示名（如 "免费体验" / "个人订阅"）
    pub plan_name: String,
    #[serde(default)]
    pub plan_short_name: Option<String>,
    /// 剩余总额（原始字符串，带千分位，如 "2,526"）
    pub total_remaining: String,
    /// 剩余总额数值
    pub total_remaining_number: f64,
    /// 是否存在有效订阅
    pub has_active_subscription: bool,
    /// 分组汇总（赠送 / 付费等）
    pub summaries: Vec<QuotaSummaryView>,
    /// 额度包明细（合并概览 top_items 与活动奖励 entitlements）
    pub packages: Vec<QuotaPackageView>,
    /// 窗口聚合口径的总量/已用/剩余/百分比；无窗口数据时由额度包求和兜底。
    #[serde(default)]
    pub aggregate: Option<QuotaAggregateView>,
    /// 服务端当前时间（毫秒）
    pub service_current_time_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QuotaSummaryView {
    pub category: Option<i32>,
    pub title: String,
    pub remaining_amount: String,
    pub remaining_number: f64,
}

/// 消耗流水条目归一化视图。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TimelineEntryView {
    pub entry_type: Option<i32>,
    /// 提问/会话标题（display_name）
    pub display_name: String,
    pub item_id: Option<String>,
    pub model_display_name: String,
    /// 发生时间（毫秒）
    pub occurred_at_ms: Option<i64>,
    pub quota_source_display: String,
    pub quota_source_code: Option<String>,
}

/// 消耗流水归一化视图（P2 详情弹窗使用）。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DoubaoTimelineView {
    pub categories: Vec<i32>,
    pub consumption_column_title: String,
    pub window_start_time_ms: Option<i64>,
    pub window_end_time_ms: Option<i64>,
    pub has_more: bool,
    pub cursor: Option<String>,
    pub entries: Vec<TimelineEntryView>,
}

impl DoubaoQuotaView {
    /// 合并 D3 分组明细中的额度包（按 名称 + 到期 + 总量 + 已用 去重，避免与概览/entitlements 重复）。
    pub fn merge_detail(&mut self, detail: DetailData) {
        let mut seen: std::collections::HashSet<(String, Option<i64>, String, String)> = self
            .packages
            .iter()
            .map(|p| {
                (
                    p.display_name.clone(),
                    p.end_time_ms,
                    p.total_amount_str.clone(),
                    p.used_amount_str.clone(),
                )
            })
            .collect();

        for g in &detail.groups {
            for i in &g.items {
                let key = (
                    i.display_name.clone().unwrap_or_default(),
                    i.end_time,
                    i.total_amount.clone().unwrap_or_default(),
                    i.used_amount.clone().unwrap_or_default(),
                );
                if seen.contains(&key) {
                    continue;
                }
                seen.insert(key.clone());
                self.packages.push(QuotaPackageView::from_item_raw(i, g.category.or(i.category)));
            }
        }
    }
}

impl From<OverviewData> for DoubaoQuotaView {
    fn from(o: OverviewData) -> Self {
        let overview = o.quota_balance_overview.clone().unwrap_or_default();

        let plan_name = o
            .current_subscription
            .as_ref()
            .and_then(|s| s.display.as_ref())
            .and_then(|d| d.short_name.clone().or_else(|| d.product_name.clone()))
            .unwrap_or_else(|| "免费体验".to_string());
        let plan_short_name = o
            .current_subscription
            .as_ref()
            .and_then(|s| s.display.as_ref())
            .and_then(|d| d.short_name.clone());

        let total_remaining = overview.total_remaining_amount.clone().unwrap_or_default();
        let summaries = overview
            .summaries
            .iter()
            .map(|s| QuotaSummaryView {
                category: s.category,
                title: s.title.clone().unwrap_or_default(),
                remaining_amount: s.remaining_amount.clone().unwrap_or_default(),
                remaining_number: parse_amount(s.remaining_amount.as_deref()),
            })
            .collect::<Vec<_>>();

        // 活动奖励 entitlements 是全量额度包；top_items 仅为其子集高亮，需按 key 去重后补充
        let mut packages: Vec<QuotaPackageView> = Vec::new();
        let mut seen: std::collections::HashSet<(String, Option<i64>, String, String)> =
            std::collections::HashSet::new();

        for e in &o.incentive_quota_entitlements {
            let display = e.display.as_ref();
            let total_str = e.total_amount.clone().unwrap_or_default();
            let used_str = e.used_amount.clone().unwrap_or_default();
            let name = display
                .and_then(|d| d.title.clone().or_else(|| d.product_name.clone()))
                .unwrap_or_else(|| "活动奖励".to_string());
            let key = (name.clone(), e.end_time, total_str.clone(), used_str.clone());
            if seen.contains(&key) {
                continue;
            }
            seen.insert(key);
            packages.push(QuotaPackageView {
                category: Some(3),
                display_name: name,
                sku_key: e.sku_key.clone(),
                total_amount_str: total_str,
                used_amount_str: used_str,
                total_amount: parse_amount(e.total_amount.as_deref()),
                used_amount: parse_amount(e.used_amount.as_deref()),
                used_percent: e.total_used_percent.unwrap_or(0),
                end_time_ms: e.end_time,
                status: e.status,
                subscription_type: e.subscription_type,
                is_resettable: e.is_resettable,
            });
        }

        for i in &overview.top_items {
            let total_str = i.total_amount.clone().unwrap_or_default();
            let used_str = i.used_amount.clone().unwrap_or_default();
            let key = (
                i.display_name.clone().unwrap_or_default(),
                i.end_time,
                total_str.clone(),
                used_str.clone(),
            );
            if seen.contains(&key) {
                continue;
            }
            seen.insert(key);
            packages.push(QuotaPackageView::from_item_raw(i, i.category));
        }

        // 聚合口径：优先服务端窗口分组汇总；无窗口数据时按额度包求和兜底
        let aggregate = QuotaAggregateView::from_windows(&[
            o.incentive_quota_section.as_ref(),
            o.window_limit_section.as_ref(),
            o.enterprise_window_limit_section.as_ref(),
        ])
        .or_else(|| {
            let mut total = 0.0;
            let mut used = 0.0;
            for p in &packages {
                if p.total_amount > 0.0 {
                    total += p.total_amount;
                    used += p.used_amount.min(p.total_amount);
                }
            }
            if total <= 0.0 {
                return None;
            }
            Some(QuotaAggregateView {
                total_amount: total,
                used_amount: used,
                left_amount: total - used,
                used_percent: ((used / total) * 100.0).round() as i32,
                source: "packages".to_string(),
            })
        });

        Self {
            plan_name,
            plan_short_name,
            total_remaining,
            total_remaining_number: parse_amount(overview.total_remaining_amount.as_deref()),
            has_active_subscription: o
                .member_info
                .as_ref()
                .and_then(|m| m.has_active_subscription)
                .unwrap_or(false),
            summaries,
            packages,
            aggregate,
            service_current_time_ms: o.service_current_time,
        }
    }
}

impl From<TimelineData> for DoubaoTimelineView {
    fn from(t: TimelineData) -> Self {
        let entries = t
            .entries
            .iter()
            .filter_map(|e| {
                let u = e.usage.as_ref()?;
                Some(TimelineEntryView {
                    entry_type: e.entry_type,
                    display_name: u.display_name.clone().unwrap_or_default(),
                    item_id: u.item_id.clone(),
                    model_display_name: u.model_display_name.clone().unwrap_or_default(),
                    occurred_at_ms: u.occurred_at_ms,
                    quota_source_display: u
                        .quota_source
                        .as_ref()
                        .and_then(|q| q.display_text.clone().or_else(|| q.display_name.clone()))
                        .unwrap_or_default(),
                    quota_source_code: u.quota_source.as_ref().and_then(|q| q.quota_source_code.clone()),
                })
            })
            .collect();

        Self {
            categories: t.categories,
            consumption_column_title: t.consumption_column_title.unwrap_or_else(|| "额度消耗".to_string()),
            window_start_time_ms: t.window_start_time_ms,
            window_end_time_ms: t.window_end_time_ms,
            has_more: t.has_more.unwrap_or(false),
            cursor: t.cursor,
            entries,
        }
    }
}

#[cfg(test)]
mod samples_test {
    use super::*;

    fn read_sample(name: &str) -> serde_json::Value {
        let path = format!(
            "{}/../docs/doubao-api/samples/{}",
            env!("CARGO_MANIFEST_DIR"),
            name
        );
        let text = std::fs::read_to_string(path).expect("样本文件应存在");
        serde_json::from_str(&text).expect("样本 JSON 应合法")
    }

    #[test]
    fn parse_d1_account_info_sample() {
        let env: DoubaoEnvelope<DoubaoAccountInfo> =
            serde_json::from_value(read_sample("account-info.json")).unwrap();
        let info = env.data.expect("D1 data 不应为空");
        assert_eq!(info.normalized_user_id(), "1831141604067483");
        assert_eq!(info.display_name(), "飞行森森");
        assert!(info.mobile.as_deref().unwrap_or("").contains('*'));
    }

    #[test]
    fn parse_d2_overview_sample() {
        let env: DoubaoEnvelope<OverviewData> =
            serde_json::from_value(read_sample("subscription-overview.json")).unwrap();
        let view = DoubaoQuotaView::from(env.data.expect("D2 data 不应为空"));
        assert_eq!(view.total_remaining, "2,526");
        assert_eq!(view.total_remaining_number, 2526.0);
        assert!(!view.has_active_subscription);
        assert!(view.packages.len() >= 1, "应合并 top_items 与活动奖励包");
        assert!(view.summaries.iter().any(|s| s.title == "赠送"));
    }

    #[test]
    fn parse_d4_timeline_sample() {
        let env: DoubaoEnvelope<TimelineData> =
            serde_json::from_value(read_sample("usage-timeline.json")).unwrap();
        let view = DoubaoTimelineView::from(env.data.expect("D4 data 不应为空"));
        assert!(!view.entries.is_empty());
        let first = &view.entries[0];
        assert_eq!(first.model_display_name, "Auto");
        assert!(!first.display_name.is_empty());
        assert!(!view.has_more);
    }

    #[test]
    fn merge_d3_detail_dedup() {
        let ov: DoubaoEnvelope<OverviewData> =
            serde_json::from_value(read_sample("subscription-overview.json")).unwrap();
        let mut view = DoubaoQuotaView::from(ov.data.expect("D2 data 不应为空"));
        let before = view.packages.len();

        let det: DoubaoEnvelope<DetailData> =
            serde_json::from_value(read_sample("quota-balance-detail.json")).unwrap();
        view.merge_detail(det.data.expect("D3 data 不应为空"));

        assert!(view.packages.len() >= before, "合并后包数不应减少");
        // D3 样本赠送分组含 14 条额度包
        assert!(view.packages.len() >= 14);
    }

    #[test]
    fn parse_live_overview_aggregate() {
        let env: DoubaoEnvelope<OverviewData> =
            serde_json::from_value(read_sample("subscription-overview-live.json")).unwrap();
        let mut view = DoubaoQuotaView::from(env.data.expect("D2 live data 不应为空"));

        let det: DoubaoEnvelope<DetailData> =
            serde_json::from_value(read_sample("quota-balance-detail-live.json")).unwrap();
        view.merge_detail(det.data.expect("D3 live data 不应为空"));

        // 窗口聚合：总量 3,200 / 已用 718 / 剩余 2,482 / 22%
        let agg = view.aggregate.expect("线上样本应带窗口聚合");
        assert_eq!(agg.source, "window");
        assert_eq!(agg.total_amount, 3200.0);
        assert_eq!(agg.used_amount, 718.0);
        assert_eq!(agg.left_amount, 2482.0);
        assert_eq!(agg.used_percent, 22);
        assert_eq!(view.total_remaining_number, 2482.0);

        // entitlements 全量 14 条，top_items 为其子集，合并 D3 后不应出现重复包
        assert_eq!(view.packages.len(), 14, "额度包不应重复");
    }

    #[test]
    fn parse_d10_checkin_sample() {
        let env: DoubaoEnvelope<DoubaoCheckinData> =
            serde_json::from_value(read_sample("checkin-status.json")).unwrap();
        let data = env.data.expect("D10 data 不应为空");
        assert!(data.checked_in);
        assert_eq!(data.checkin_date.as_deref(), Some("2026-11-23"));
        assert_eq!(data.continuous_days, Some(6));
        assert_eq!(data.reward_amount, Some(400));
        assert_eq!(data.reward_code.as_deref(), Some("quota_grant"));
        assert_eq!(data.reward_status, Some(3));

        let view = DoubaoCheckinView::from(data);
        assert!(view.checked_in);
        assert_eq!(view.reward_amount, 400);
        assert_eq!(view.continuous_days, 6);
    }
}
