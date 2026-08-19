use serde::{Deserialize, Serialize};

/// JWT Token 解析后的原始数据
#[derive(Debug, Clone, Deserialize)]
pub struct JwtPayloadRaw {
    pub data: JwtData,
    pub exp: i64,
    pub iat: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JwtData {
    pub id: String,
    pub source: String,
    pub source_id: String,
    pub tenant_id: String,
    #[serde(rename = "type")]
    pub data_type: String,
}

/// JWT Token 解析后的用户信息
#[derive(Debug, Clone)]
pub struct JwtPayload {
    pub user_id: String,
    pub tenant_id: String,
}

/// 通过 Token 获取的用户信息
#[derive(Debug, Clone)]
pub struct TokenUserInfo {
    pub user_id: String,
    pub tenant_id: String,
    pub screen_name: Option<String>,
    pub avatar_url: Option<String>,
    pub email: Option<String>,
}

/// 用户 Token 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetUserTokenResponse {
    #[serde(rename = "ResponseMetadata")]
    pub response_metadata: ResponseMetadata,
    #[serde(rename = "Result")]
    pub result: UserTokenResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseMetadata {
    #[serde(rename = "RequestId")]
    pub request_id: String,
    #[serde(rename = "TraceID")]
    pub trace_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserTokenResult {
    #[serde(rename = "Token")]
    pub token: String,
    #[serde(rename = "ExpiredAt")]
    pub expired_at: String,
    #[serde(rename = "UserID")]
    pub user_id: String,
    #[serde(rename = "TenantID")]
    pub tenant_id: String,
}

/// 用户信息响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetUserInfoResponse {
    #[serde(rename = "ResponseMetadata")]
    pub response_metadata: ResponseMetadata,
    #[serde(rename = "Result")]
    pub result: UserInfoResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfoResult {
    #[serde(rename = "ScreenName")]
    pub screen_name: String,
    #[serde(rename = "Gender")]
    pub gender: String,
    #[serde(rename = "AvatarUrl")]
    pub avatar_url: String,
    #[serde(rename = "UserID")]
    pub user_id: String,
    #[serde(rename = "Description")]
    pub description: String,
    #[serde(rename = "TenantID")]
    pub tenant_id: String,
    #[serde(rename = "RegisterTime")]
    pub register_time: String,
    #[serde(rename = "LastLoginTime")]
    pub last_login_time: String,
    #[serde(rename = "LastLoginType")]
    pub last_login_type: String,
    #[serde(rename = "Region")]
    pub region: String,
    #[serde(rename = "AIRegion")]
    pub ai_region: Option<String>,
    #[serde(rename = "NonPlainTextEmail")]
    pub non_plain_text_email: Option<String>,
    #[serde(rename = "StoreCountry")]
    pub store_country: Option<String>,
}

/// 用户配额/使用量响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitlementListResponse {
    pub is_pay_freshman: bool,
    #[serde(default)]
    pub is_credits_billing: bool,
    #[serde(default)]
    pub is_dollar_usage_billing: bool,
    #[serde(default)]
    pub trial_status: Option<TrialStatus>,
    pub user_entitlement_pack_list: Vec<EntitlementPack>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TrialStatus {
    #[serde(default)]
    pub is_eligible_for_trial: bool,
    #[serde(default)]
    pub is_in_trial: bool,
    #[serde(default)]
    pub trial_end_time: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitlementPack {
    #[serde(default)]
    pub display_desc: String,
    #[serde(default)]
    pub group_name: String,
    #[serde(default)]
    pub group_type: i32,
    #[serde(default)]
    pub is_hide: bool,
    pub entitlement_base_info: EntitlementBaseInfo,
    pub expire_time: i64,
    pub is_last_period: bool,
    pub next_billing_time: i64,
    pub source_id: String,
    pub status: i32,
    pub usage: UsageInfo,
    pub yearly_expire_time: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitlementBaseInfo {
    #[serde(default)]
    pub available_endpoint: i32,
    pub charge_amount: i64,
    pub currency: i32,
    pub end_time: i64,
    pub entitlement_id: String,
    pub product_extra: ProductExtra,
    pub product_id: i32,
    pub product_type: i32,
    pub quota: Quota,
    pub start_time: i64,
    pub user_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductExtra {
    #[serde(default)]
    pub package_extra: Option<PackageExtra>,
    #[serde(default)]
    pub subscription_extra: Option<SubscriptionExtra>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageExtra {
    #[serde(default)]
    pub duration: i32,
    #[serde(default)]
    pub package_duration_type: i32,
    #[serde(default)]
    pub package_name: String,
    #[serde(default)]
    pub package_source_type: i32,
    #[serde(default)]
    pub quota: Quota,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionExtra {
    pub period_type: i32,
    pub quota: Quota,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Quota {
    #[serde(default)]
    pub credits_limit: f64,
    #[serde(default)]
    pub advanced_model_request_limit: i64,
    #[serde(default)]
    pub auto_completion_limit: i64,
    #[serde(default)]
    pub basic_usage_limit: i64,
    #[serde(default)]
    pub bonus_usage_limit: i64,
    #[serde(default)]
    pub enable_early_access: bool,
    #[serde(default)]
    pub enable_ralph_loop: bool,
    #[serde(default)]
    pub enable_solo_agent: bool,
    #[serde(default)]
    pub enable_solo_builder: bool,
    #[serde(default)]
    pub enable_solo_builder_v1: bool,
    #[serde(default)]
    pub enable_solo_coder: bool,
    #[serde(default)]
    pub enable_solo_lite: bool,
    #[serde(default)]
    pub enable_solo_web: bool,
    #[serde(default)]
    pub enable_super_model: bool,
    #[serde(default)]
    pub no_bonus_quota: bool,
    #[serde(default)]
    pub premium_model_fast_request_limit: i64,
    #[serde(default)]
    pub premium_model_slow_request_limit: i64,
    #[serde(default)]
    pub solo_agent_parallel_limit: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UsageInfo {
    #[serde(default)]
    pub credits_amount: f64,
    #[serde(default)]
    pub advanced_model_amount: f64,
    #[serde(default)]
    pub advanced_model_request_usage: f64,
    #[serde(default)]
    pub auto_completion_amount: f64,
    #[serde(default)]
    pub auto_completion_usage: f64,
    #[serde(default)]
    pub basic_usage_amount: f64,
    #[serde(default)]
    pub bonus_usage_amount: f64,
    #[serde(default)]
    pub is_flash_consuming: bool,
    #[serde(default)]
    pub premium_model_fast_amount: f64,
    #[serde(default)]
    pub premium_model_fast_request_usage: f64,
    #[serde(default)]
    pub premium_model_slow_amount: f64,
    #[serde(default)]
    pub premium_model_slow_request_usage: f64,
}

/// 使用记录查询响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageQueryResponse {
    pub total: i64,
    pub user_usage_group_by_sessions: Vec<UsageSession>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageSession {
    pub session_id: String,
    pub usage_time: i64,
    pub mode: String,
    pub model_name: String,
    pub amount_float: f64,
    pub cost_money_float: f64,
    pub use_max_mode: bool,
    pub product_type_list: Vec<i32>,
    pub extra_info: UsageExtraInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageExtraInfo {
    pub cache_read_token: i64,
    pub cache_write_token: i64,
    pub input_token: i64,
    pub output_token: i64,
}

/// 简化的使用量汇总（用于前端展示）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageSummary {
    pub plan_type: String,
    pub reset_time: i64,

    // Fast Request
    pub fast_request_used: f64,
    pub fast_request_limit: i64,
    pub fast_request_left: f64,

    // Extra Package (如周年礼包)
    pub extra_fast_request_used: f64,
    pub extra_fast_request_limit: i64,
    pub extra_fast_request_left: f64,
    pub extra_expire_time: i64,
    pub extra_package_name: String,

    // Slow Request
    pub slow_request_used: f64,
    pub slow_request_limit: i64,
    pub slow_request_left: f64,

    // Advanced Model
    pub advanced_model_used: f64,
    pub advanced_model_limit: i64,
    pub advanced_model_left: f64,

    // Autocomplete
    pub autocomplete_used: f64,
    pub autocomplete_limit: i64,
    pub autocomplete_left: f64,
}

impl Default for UsageSummary {
    fn default() -> Self {
        Self {
            plan_type: "Free".to_string(),
            reset_time: 0,
            fast_request_used: 0.0,
            fast_request_limit: 10,
            fast_request_left: 10.0,
            extra_fast_request_used: 0.0,
            extra_fast_request_limit: 0,
            extra_fast_request_left: 0.0,
            extra_expire_time: 0,
            extra_package_name: String::new(),
            slow_request_used: 0.0,
            slow_request_limit: 50,
            slow_request_left: 50.0,
            advanced_model_used: 0.0,
            advanced_model_limit: 1000,
            advanced_model_left: 1000.0,
            autocomplete_used: 0.0,
            autocomplete_limit: 5000,
            autocomplete_left: 5000.0,
        }
    }
}

// ========================================================================
// 国内版（CN / WORK）积分体系 — 对应 https://www.trae.cn/dashboard#usage
// ========================================================================

/// 单个积分分类的额度/已用/剩余/最近到期
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreditsCategory {
    /// 总额度
    #[serde(default, alias = "totalLimit", alias = "total_limit")]
    pub total_limit: f64,
    /// 已用
    #[serde(default)]
    pub used: f64,
    /// 剩余
    #[serde(default, alias = "remaining")]
    pub left: f64,
    /// 最近一笔到期时间（UTC epoch sec，0 表示无到期或永久）
    #[serde(default, alias = "nearestExpireTime", alias = "nearest_expire_time")]
    pub nearest_expire_time: i64,
}

/// 奖励积分条目：按标题/类型/到期/进度/子笔数展示
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RewardCreditsEntry {
    /// 标题，如 "每月登录赠送" / "老用户福利" / "每日签到" / "套餐 Lite 会员积分" / "邀请奖励"
    #[serde(default, alias = "name")]
    pub title: String,
    /// 适用范围：general / work_exclusive
    #[serde(default, alias = "type")]
    pub scope: String,
    /// 总发放额度
    #[serde(default)]
    pub total: f64,
    /// 已用
    #[serde(default)]
    pub used: f64,
    /// 到期 epoch sec
    #[serde(default, alias = "expireTime", alias = "expire_time")]
    pub expire_time: i64,
    /// 多条目合并时的子笔数（如"共 10 笔"签到）
    #[serde(default, alias = "subCount", alias = "sub_count")]
    pub sub_count: i64,
}

/// 奖励积分汇总
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RewardCredits {
    /// 奖励积分总剩余
    #[serde(default, alias = "totalLeft", alias = "total_left")]
    pub total_left: f64,
    /// 所有奖励条目
    #[serde(default, alias = "items")]
    pub entries: Vec<RewardCreditsEntry>,
}

/// 积分主状态接口（cn_credits_billing_status）的原始响应封装
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditsBillingStatusResponse {
    #[serde(default)]
    pub code: i32,
    #[serde(default)]
    pub message: Option<String>,
    /// true = 当前账号使用积分计费；false = 仍走旧 entitlement 配额（国际版/老账号）
    #[serde(default, alias = "isCreditsBilling", alias = "is_credits_billing")]
    pub is_credits_billing: bool,
    #[serde(default, alias = "shouldForceSwitch", alias = "should_force_switch")]
    pub should_force_switch: bool,

    /// 通用积分（TraeCode + TraeWork）
    #[serde(default, alias = "generalCredits", alias = "general_credits")]
    pub general_credits: CreditsCategory,
    /// Work 专属积分
    #[serde(default, alias = "workExclusiveCredits", alias = "work_exclusive_credits")]
    pub work_exclusive_credits: CreditsCategory,
    /// 会员积分小计（可选）
    #[serde(default, alias = "membershipCredits", alias = "membership_credits")]
    pub membership_credits: Option<CreditsCategory>,
    /// 奖励积分合计 + 明细
    #[serde(default, alias = "rewardCredits", alias = "reward_credits")]
    pub reward_credits: Option<RewardCredits>,

    /// 主套餐有效期结束时间（UTC epoch sec）
    #[serde(default, alias = "planExpireTime", alias = "plan_expire_time")]
    pub plan_expire_time: i64,
    /// 当前套餐：Free / Lite / Pro / Pro+ / Ultra
    #[serde(default, alias = "planName", alias = "plan_name")]
    pub plan_name: String,
    /// 订阅/商业化身份兜底字段（从 web_user_pay_status 拿更准，这里做备份）
    #[serde(default, alias = "userPayIdentityStr", alias = "user_pay_identity_str")]
    pub user_pay_identity_str: Option<String>,
}

/// 网页版 "支付状态" 接口的原始响应（用来拿 Free/Pro 计划名和是否积分计费）
///
/// 路径：POST /trae/api/v2/pay/web_user_pay_status
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WebUserPayStatusResponse {
    #[serde(default)]
    pub code: i32,
    #[serde(default)]
    pub message: Option<String>,

    #[serde(default)]
    pub is_credits_billing: bool,
    #[serde(default)]
    pub is_dollar_usage_billing: bool,
    #[serde(default)]
    pub is_pay_freshman: bool,
    #[serde(default)]
    pub server_time_ms: i64,
    #[serde(default, alias = "userPayIdentity", alias = "user_pay_identity")]
    pub user_pay_identity: i32,
    #[serde(default, alias = "userPayIdentityStr", alias = "user_pay_identity_str")]
    pub user_pay_identity_str: String,

    // 功能开关
    #[serde(default)]
    pub enable_fission: bool,
    #[serde(default)]
    pub enable_solo_builder: bool,
    #[serde(default)]
    pub enable_solo_coder: bool,
    #[serde(default)]
    pub enable_solo_lite: bool,
    #[serde(default)]
    pub enable_solo_web: bool,
}

/// 给前端展示用的简化积分汇总。
/// 当 `is_credits_billing == false` 时前端应回退显示旧 `UsageSummary`。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditSummary {
    pub is_credits_billing: bool,
    pub plan_name: String,
    pub plan_expire_time: i64,
    /// 大号总可用积分：通用剩余 + Work 专属剩余
    pub total_available: f64,
    pub general: CreditsCategory,
    pub work_exclusive: CreditsCategory,
    /// 奖励积分剩余合计
    pub reward_total_left: f64,
    /// 奖励积分条目（用于"每月登录赠送 / 老用户福利 / 签到 / 邀请…"列表）
    pub reward_entries: Vec<RewardCreditsEntry>,
}

impl Default for CreditSummary {
    fn default() -> Self {
        Self {
            is_credits_billing: false,
            plan_name: "Free".to_string(),
            plan_expire_time: 0,
            total_available: 0.0,
            general: CreditsCategory::default(),
            work_exclusive: CreditsCategory::default(),
            reward_total_left: 0.0,
            reward_entries: Vec::new(),
        }
    }
}

