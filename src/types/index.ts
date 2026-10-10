// 账号简要信息
export interface AccountBrief {
  id: string;
  name: string;
  email: string;
  avatar_url: string;
  plan_type: string;
  is_active: boolean;
  created_at: number;
  machine_id: string | null;
  is_current: boolean; // 是否是当前 Trae IDE 正在使用的账号
  is_client_active: boolean; // 是否是 Trae 客户端当前登录的账号（单活跃 Token 下由客户端持有有效会话）
  /** 该账号正在哪些 Trae 客户端中登录（显示名称列表），用于多客户端"当前"标签 */
  active_in_clients: string[];
  /** 是否有 cookies（浏览器导入的账号才有，可续签；客户端导入/手动Token无 cookies） */
  has_cookies: boolean;
  token_expired_at: string | null; // Token 过期时间
  /** 上次通过 cookies 续签 token 的时间戳（秒）。cookies 本身无明确过期时间，需每 12h 续签保持 session */
  last_cookie_renewal_at: number | null;
  /** 登录来源：client_import / webview / cookie / manual_token / unknown（新建打标，存量 unknown） */
  login_source: string;
  /** 登录态类型：native_o_auth（客户端原生OAuth）/ injected（切号注入）/ standalone（仅库内） */
  login_type: string;
  /** 今日签到状态（启动时/点击刷新查询，可能为 undefined 表示未查） */
  checkin_status?: CheckinStatusResult;
}

// 完整账号信息
export interface Account {
  id: string;
  name: string;
  email: string;
  avatar_url: string;
  cookies: string;
  jwt_token: string | null;
  token_expired_at: string | null;
  last_cookie_renewal_at: number | null;
  user_id: string;
  tenant_id: string;
  region: string;
  plan_type: string;
  created_at: number;
  updated_at: number;
  is_active: boolean;
  machine_id: string | null;
}

// 使用量汇总
export interface UsageSummary {
  plan_type: string;
  reset_time: number;

  // Fast Request
  fast_request_used: number;
  fast_request_limit: number;
  fast_request_left: number;

  // Extra Package
  extra_fast_request_used: number;
  extra_fast_request_limit: number;
  extra_fast_request_left: number;
  extra_expire_time: number;
  extra_package_name: string;

  // Slow Request
  slow_request_used: number;
  slow_request_limit: number;
  slow_request_left: number;

  // Advanced Model
  advanced_model_used: number;
  advanced_model_limit: number;
  advanced_model_left: number;

  // Autocomplete
  autocomplete_used: number;
  autocomplete_limit: number;
  autocomplete_left: number;
}

// 使用事件
export interface UsageEvent {
  session_id: string;
  usage_time: number;
  mode: string;
  model_name: string;
  amount_float: number;
  cost_money_float: number;
  credits_float: number;
  use_max_mode: boolean;
  product_type_list: number[];
  usage_source: number;
  user_input_preview: string;
  extra_info: {
    cache_read_token: number;
    cache_write_token: number;
    input_token: number;
    output_token: number;
  };
}

// 使用事件响应
export interface UsageEventsResponse {
  total: number;
  user_usage_group_by_sessions: UsageEvent[];
}

// API 错误
export interface ApiError {
  message: string;
}

// Trae 应用变体信息（TRAE CN / TraeWork CN / 国际版）
export interface TraeAppInfo {
  key: string;
  display_name: string;
  installed: boolean;
  data_dir: string;
  is_current: boolean;
  /** 浏览器登录站点，如 https://www.trae.cn */
  login_url: string;
}

// ====================================================================
// 国内版（CN / WORK）积分体系 — CreditSummary
// 与 Rust 端 src-tauri/src/api/types.rs 的 CreditSummary 保持一致
// ====================================================================

export interface CreditsCategory {
  total_limit: number;
  used: number;
  left: number;
  nearest_expire_time: number; // UTC epoch sec，0=永久/未知
}

export interface RewardCreditsEntry {
  title: string;          // "每月登录赠送" / "老用户福利" / "每日签到" / "邀请奖励" 等
  scope: 'general' | 'work_exclusive' | string;
  total: number;          // 发放总额度
  used: number;           // 已用
  expire_time: number;    // UTC epoch sec
  sub_count: number;      // 子笔数（如签到"共 10 笔"）
}

export interface CreditSummary {
  is_credits_billing: boolean; // false 时前端回退旧 UsageSummary 渲染
  plan_name: string;           // "Free" / "Lite" / "Pro" / "Pro+" / "Ultra"
  plan_expire_time: number;    // UTC epoch sec
  total_available: number;     // 大号总可用积分：通用剩余 + Work 专属剩余
  general: CreditsCategory;
  work_exclusive: CreditsCategory;
  reward_total_left: number;   // 奖励积分剩余合计
  reward_entries: RewardCreditsEntry[]; // 奖励积分条目列表
}

// ============ 签到相关 ============

export interface CheckinStatusResult {
  code: number;
  message: string;
  checked_in: boolean; // 今日是否已签到
  credits: number;     // 签到可获得积分（一般 200）
  enable: boolean;     // 签到功能是否可用
}

export interface CheckinAllStatusItem {
  account_id: string;
  account_name: string;
  code: number;
  message: string;
  checked_in: boolean;
  credits: number;
  enable: boolean;
}

// 签到请求头预览条目（"查看签到请求头"弹窗用）
export interface CheckinHeaderEntry {
  name: string;
  value: string;
  /** fixed=固定值 account=账号专属(虚拟设备,持久化) credential=身份凭证 dynamic=每次请求变化 */
  kind: "fixed" | "account" | "credential" | "dynamic" | string;
  /** 用途说明 */
  note: string;
}

// 签到虚拟设备档案（每个账号持久化一套）
export interface CheckinDeviceProfile {
  session_id: string;
  market_user_id: string;
  device_id: string;
  device_brand: string;
  device_type: string;
}

// 设备 ID 生成策略
export type DeviceIdStrategy = "real_device_prefix" | "safe_range_fnv";

// 签到全局配置
export interface CheckinConfig {
  device_id_strategy: DeviceIdStrategy;
  status_delay_min: number;
  status_delay_max: number;
  claim_delay_min: number;
  claim_delay_max: number;
  /** 是否开启自动刷新（定时刷新账号使用量数据） */
  auto_refresh_enabled: boolean;
  /** 自动刷新间隔（分钟），可选值：5 / 10 / 30 / 60 */
  auto_refresh_interval: number;
  /** 是否开启自动签到（晚于设定时间后自动为未签到账号签到） */
  auto_checkin_enabled: boolean;
  /** 自动签到触发时间点（HH:mm，24 小时制），例如 "22:00" */
  auto_checkin_time: string;
  /** 是否开启日志文件运行中自动重建（watchdog 每 interval 秒检查一次，发现日志被外部删除/替换则自动重建） */
  log_watchdog_enabled: boolean;
  /** 日志 watchdog 检查间隔（秒），最小 2 */
  log_watchdog_interval: number;
}
