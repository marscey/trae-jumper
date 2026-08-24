import { invoke } from "@tauri-apps/api/core";
import type {
  Account,
  AccountBrief,
  CheckinConfig,
  CheckinDeviceProfile,
  CheckinHeaderEntry,
  CheckinStatusResult,
  CheckinAllStatusItem,
  CreditSummary,
  DeviceIdStrategy,
  UsageSummary,
  UsageEventsResponse,
  TraeAppInfo,
} from "./types";

// ============ Tauri 环境检测 + 浏览器 Mock ============
export function hasTauri(): boolean {
  try {
    return typeof window !== "undefined" &&
      !!(window as any).__TAURI_INTERNALS__;
  } catch {
    return false;
  }
}

function delay<T>(v: T, ms = 350): Promise<T> {
  return new Promise((r) => setTimeout(() => r(v), ms));
}

const MOCK_ACCOUNTS: AccountBrief[] = [
  {
    id: "mock-1",
    name: "Mock 国内账号 (Free 积分)",
    email: "mock.free@trae.cn",
    avatar_url: "",
    plan_type: "Free",
    is_active: true,
    created_at: Date.now() - 1000 * 60 * 60 * 24 * 10,
    machine_id: "MOCK-MACHINE",
    is_current: true,
    token_expired_at: new Date(Date.now() + 1000 * 60 * 60 * 24 * 30).toISOString(),
  },
  {
    id: "mock-2",
    name: "Mock Lite 订阅",
    email: "mock.lite@trae.cn",
    avatar_url: "",
    plan_type: "Lite",
    is_active: true,
    created_at: Date.now() - 1000 * 60 * 60 * 24 * 50,
    machine_id: "MOCK-MACHINE",
    is_current: false,
    token_expired_at: new Date(Date.now() + 1000 * 60 * 60 * 24 * 60).toISOString(),
  },
  {
    id: "mock-3",
    name: "Mock 国际版账号 (旧配额)",
    email: "mock.intl@trae.ai",
    avatar_url: "",
    plan_type: "Pro",
    is_active: true,
    created_at: Date.now() - 1000 * 60 * 60 * 24 * 100,
    machine_id: "MOCK-MACHINE",
    is_current: false,
    token_expired_at: new Date(Date.now() + 1000 * 60 * 60 * 24 * 10).toISOString(),
  },
];

const MOCK_CREDITS_1: CreditSummary = {
  is_credits_billing: true,
  plan_name: "Free",
  plan_expire_time: Math.floor(Date.now() / 1000) + 31 * 24 * 3600,
  total_available: 2000 + 3622.32,
  general: {
    total_limit: 2000,
    used: 0,
    left: 2000,
    nearest_expire_time: Math.floor(Date.now() / 1000) + 31 * 24 * 3600,
  },
  work_exclusive: {
    total_limit: 3622.32,
    used: 22.32,
    left: 3600.0,
    nearest_expire_time: Math.floor(Date.now() / 1000) + 15 * 24 * 3600,
  },
  reward_total_left: 500,
  reward_entries: [
    { title: "新人登录奖励", scope: "general", total: 500, used: 0, expire_time: Math.floor(Date.now() / 1000) + 7 * 24 * 3600, sub_count: 1 },
  ],
};

const MOCK_CREDITS_2: CreditSummary = {
  is_credits_billing: true,
  plan_name: "Lite",
  plan_expire_time: Math.floor(Date.now() / 1000) + 60 * 24 * 3600,
  total_available: 10000 + 15000,
  general: {
    total_limit: 10000,
    used: 3500,
    left: 6500,
    nearest_expire_time: Math.floor(Date.now() / 1000) + 60 * 24 * 3600,
  },
  work_exclusive: {
    total_limit: 15000,
    used: 4000,
    left: 11000,
    nearest_expire_time: Math.floor(Date.now() / 1000) + 60 * 24 * 3600,
  },
  reward_total_left: 0,
  reward_entries: [],
};

const MOCK_USAGE_3: UsageSummary = {
  plan_type: "Pro",
  reset_time: Math.floor(Date.now() / 1000) + 14 * 24 * 3600,
  fast_request_used: 120,
  fast_request_limit: 500,
  fast_request_left: 380,
  extra_fast_request_used: 0,
  extra_fast_request_limit: 200,
  extra_fast_request_left: 200,
  extra_expire_time: 0,
  extra_package_name: "",
  slow_request_used: 5,
  slow_request_limit: 50,
  slow_request_left: 45,
  advanced_model_used: 10,
  advanced_model_limit: 100,
  advanced_model_left: 90,
  autocomplete_used: 3000,
  autocomplete_limit: 10000,
  autocomplete_left: 7000,
};

async function safeInvoke<T>(cmd: string, args?: Record<string, any>, fallback?: () => Promise<T>): Promise<T> {
  if (hasTauri()) {
    return invoke(cmd, args) as Promise<T>;
  }
  if (fallback) return fallback();
  throw new Error(`未检测到 Tauri 环境，无法调用后端命令 "${cmd}"。请通过 npm run tauri dev 启动桌面端，或在纯浏览器预览时使用 mock 数据。`);
}

// ============ 应用变体相关 API ============

// 获取支持的 Trae 应用列表（含安装状态与当前选择）
export async function getTraeApps(): Promise<TraeAppInfo[]> {
  return safeInvoke("get_trae_apps", undefined, async () => delay([
    { key: "trae-cn", display_name: "TraeCode CN", installed: true, data_dir: "", is_current: true, login_url: "https://www.trae.cn" },
    { key: "trae-work", display_name: "TraeWork CN", installed: true, data_dir: "", is_current: false, login_url: "https://www.trae.cn" },
    { key: "trae-intl", display_name: "Trae (国际版)", installed: false, data_dir: "", is_current: false, login_url: "https://www.trae.ai" },
  ]));
}

// 从 login_url 提取可展示的登录域名（去协议、去 www.），如 https://www.trae.cn -> trae.cn
export function loginDomain(loginUrl?: string): string {
  if (!loginUrl) return "trae.cn";
  try {
    const host = new URL(loginUrl).hostname;
    return host.replace(/^www\./, "");
  } catch {
    return loginUrl;
  }
}

// 切换当前管理的目标应用（TraeCode CN / TraeWork CN / 国际版）
export async function setCurrentTraeApp(appKey: string): Promise<void> {
  return safeInvoke("set_current_trae_app", { appKey }, async () => delay(undefined));
}

// 同步当前账号状态：读取当前目标客户端已登录账号，更新 current_account_id（切换客户端后调用）
export async function syncCurrentAccount(): Promise<AccountBrief | null> {
  return safeInvoke("sync_current_account", undefined, async () => delay(null));
}

// 添加账号（通过 Cookies）
export async function addAccount(cookies: string): Promise<Account> {
  return safeInvoke("add_account", { cookies });
}

// 添加账号（通过 Token，可选 Cookies）
export async function addAccountByToken(token: string, cookies?: string): Promise<Account> {
  return safeInvoke("add_account_by_token", { token, cookies }, async () => delay({
    id: "new-" + Math.random().toString(36).slice(2, 8),
    name: token.slice(0, 8),
    email: "new@trae.cn",
    avatar_url: "",
    cookies: cookies ?? "",
    jwt_token: token,
    token_expired_at: null,
    user_id: "u-new",
    tenant_id: "t-new",
    region: "CN",
    plan_type: "Free",
    created_at: Date.now(),
    updated_at: Date.now(),
    is_active: true,
    machine_id: "MOCK-MACHINE",
  }));
}

// 删除账号
export async function removeAccount(accountId: string): Promise<void> {
  return safeInvoke("remove_account", { accountId }, async () => delay(undefined));
}

// 获取所有账号
export async function getAccounts(): Promise<AccountBrief[]> {
  return safeInvoke("get_accounts", undefined, async () => delay(MOCK_ACCOUNTS));
}

// 获取单个账号详情（包含 token）
export async function getAccount(accountId: string): Promise<Account> {
  return safeInvoke("get_account", { accountId }, async () => {
    const b = MOCK_ACCOUNTS.find((a) => a.id === accountId) ?? MOCK_ACCOUNTS[0];
    return delay({
      id: b.id, name: b.name, email: b.email, avatar_url: b.avatar_url,
      cookies: "mock-cookies", jwt_token: "mock-token",
      token_expired_at: b.token_expired_at, user_id: "u-" + b.id, tenant_id: "t-default",
      region: b.id === "mock-3" ? "INTL" : "CN", plan_type: b.plan_type,
      created_at: b.created_at, updated_at: b.created_at,
      is_active: b.is_active, machine_id: b.machine_id ?? "MOCK-MACHINE",
    });
  });
}

// 设置活跃账号
export async function setActiveAccount(accountId: string): Promise<void> {
  return safeInvoke("switch_account", { accountId }, async () => delay(undefined));
}

// 切换账号（设置活跃账号并更新机器码）
export async function switchAccount(accountId: string): Promise<void> {
  return safeInvoke("switch_account", { accountId }, async () => delay(undefined));
}

// 获取账号使用量
export async function getAccountUsage(accountId: string): Promise<UsageSummary> {
  return safeInvoke("get_account_usage", { accountId }, async () => delay(MOCK_USAGE_3));
}

// 获取账号积分汇总（CN / WORK 优先积分体系，自动回退 UsageSummary）
// 当返回值 is_credits_billing=false 时，前端应再调 getAccountUsage 做旧配额展示
export async function getAccountCredits(accountId: string): Promise<CreditSummary> {
  return safeInvoke("get_account_credits", { accountId }, async () => {
    if (accountId === "mock-1") return delay(MOCK_CREDITS_1);
    if (accountId === "mock-2") return delay(MOCK_CREDITS_2);
    // 国际版账号：返回积分计费=false，前端会回退 getAccountUsage
    return delay({
      is_credits_billing: false,
      plan_name: "Pro",
      plan_expire_time: 0,
      total_available: 0,
      general: { total_limit: 0, used: 0, left: 0, nearest_expire_time: 0 },
      work_exclusive: { total_limit: 0, used: 0, left: 0, nearest_expire_time: 0 },
      reward_total_left: 0,
      reward_entries: [],
    });
  });
}

// 更新账号 Token
export async function updateAccountToken(accountId: string, token: string): Promise<UsageSummary> {
  return safeInvoke("update_account_token", { accountId, token }, async () => delay(MOCK_USAGE_3));
}

// 刷新 Token
export async function refreshToken(accountId: string): Promise<void> {
  return safeInvoke("refresh_token", { accountId }, async () => delay(undefined));
}

// 更新 Cookies
export async function updateCookies(accountId: string, cookies: string): Promise<void> {
  return safeInvoke("update_cookies", { accountId, cookies }, async () => delay(undefined));
}

// 导出账号
export async function exportAccounts(): Promise<string> {
  return safeInvoke("export_accounts", undefined, async () => delay(JSON.stringify({
    accounts: MOCK_ACCOUNTS, active_account_id: MOCK_ACCOUNTS[0]?.id,
  }, null, 2)));
}

// 导入账号
export async function importAccounts(data: string): Promise<number> {
  return safeInvoke("import_accounts", { data }, async () => {
    try {
      const j = JSON.parse(data);
      return delay(Array.isArray(j?.accounts) ? j.accounts.length : 1);
    } catch { return delay(1); }
  });
}

// 清空所有账号数据
export async function clearAllAccounts(): Promise<number> {
  return safeInvoke("clear_all_accounts", undefined, async () => delay(3));
}

// 获取使用事件
export async function getUsageEvents(
  accountId: string,
  startTime: number,
  endTime: number,
  pageNum: number = 1,
  pageSize: number = 20
): Promise<UsageEventsResponse> {
  return safeInvoke("get_usage_events", { accountId, startTime, endTime, pageNum, pageSize }, async () => delay({
    total: 2,
    user_usage_group_by_sessions: [
      {
        session_id: "s1", usage_time: Math.floor(Date.now() / 1000) - 3600,
        mode: "solo-agent-lite", model_name: "trae-latest",
        amount_float: 1.25, cost_money_float: 0.005,
        use_max_mode: false, product_type_list: [0],
        extra_info: {
          cache_read_token: 0, cache_write_token: 0,
          input_token: 1200, output_token: 450,
        },
      },
      {
        session_id: "s2", usage_time: Math.floor(Date.now() / 1000) - 3 * 3600,
        mode: "chat", model_name: "gpt-4.1",
        amount_float: 0.3, cost_money_float: 0.001,
        use_max_mode: true, product_type_list: [0],
        extra_info: {
          cache_read_token: 100, cache_write_token: 0,
          input_token: 500, output_token: 220,
        },
      },
    ],
  }));
}

// 从 Trae IDE 读取当前登录账号
export async function readTraeAccount(): Promise<Account | null> {
  return safeInvoke("read_trae_account", undefined, async () => delay(null));
}

// ============ 机器码相关 API ============

// 获取当前系统机器码
export async function getMachineId(): Promise<string> {
  return safeInvoke("get_machine_id", undefined, async () => delay("MOCK-MACHINE-ID"));
}

// 重置系统机器码（生成新的随机机器码）
export async function resetMachineId(): Promise<string> {
  return safeInvoke("reset_machine_id", undefined, async () => delay("MOCK-" + Math.random().toString(36).slice(2, 10)));
}

// 设置系统机器码为指定值
export async function setMachineId(machineId: string): Promise<void> {
  return safeInvoke("set_machine_id", { machineId }, async () => delay(undefined));
}

// 绑定账号机器码（保存当前系统机器码到账号）
export async function bindAccountMachineId(accountId: string): Promise<string> {
  return safeInvoke("bind_account_machine_id", { accountId }, async () => delay("MOCK-MACHINE-ID"));
}

// ============ Trae IDE 机器码相关 API ============

// 获取 Trae IDE 的机器码
export async function getTraeMachineId(): Promise<string> {
  return safeInvoke("get_trae_machine_id", undefined, async () => delay("MOCK-TRAE-MACHINE"));
}

// 设置 Trae IDE 的机器码
export async function setTraeMachineId(machineId: string): Promise<void> {
  return safeInvoke("set_trae_machine_id", { machineId }, async () => delay(undefined));
}

// 获取 Trae 客户端的本机真实 device-id（ahanet/tt_net_config.config）
// 注意：device-id 是所有 Trae 系产品（TraeCode / TraeWork）共享的本机设备标识，
// 与产品专属的机器码（machineid）不同，对应签到请求头的 x-device-id。
export async function getTraeDeviceId(): Promise<string> {
  return safeInvoke("get_trae_device_id", undefined, async () => delay("MOCK-TRAE-DEVICE-ID"));
}

// 清除 Trae IDE 登录状态（让 IDE 变成全新安装状态）
export async function clearTraeLoginState(): Promise<void> {
  return safeInvoke("clear_trae_login_state", undefined, async () => delay(undefined));
}

// ============ Trae IDE 路径相关 API ============

// 获取保存的 Trae IDE 路径
export async function getTraePath(): Promise<string> {
  return safeInvoke("get_trae_path", undefined, async () => delay("C:\\Program Files\\Trae\\Trae.exe"));
}

// 设置 Trae IDE 路径
export async function setTraePath(path: string): Promise<void> {
  return safeInvoke("set_trae_path", { path }, async () => delay(undefined));
}

// 自动扫描 Trae IDE 路径
export async function scanTraePath(): Promise<string> {
  return safeInvoke("scan_trae_path", undefined, async () => delay("C:\\Program Files\\Trae\\Trae.exe"));
}

// ============ Token 刷新相关 API ============

// 批量刷新所有即将过期的 Token
export async function refreshAllTokens(): Promise<string[]> {
  return safeInvoke("refresh_all_tokens", undefined, async () => delay([]));
}

// ============ 礼包相关 API ============

// 领取礼包
export async function claimGift(accountId: string): Promise<void> {
  return safeInvoke("claim_gift", { accountId }, async () => delay(undefined));
}

// ============ 签到相关 API ============

// 签到结果
export interface CheckinResult {
  code: number;
  message: string;
}

export interface CheckinAllResultItem {
  account_id: string;
  account_name: string;
  code: number;
  message: string;
  /** true=已提前签到被跳过；false=实际执行了 claim 接口 */
  skipped?: boolean;
}

// 查询单个账号今日签到状态
export async function checkinStatus(
  accountId: string
): Promise<CheckinStatusResult> {
  return safeInvoke(
    "checkin_status",
    { accountId },
    async () => delay({ code: 0, message: "success", checked_in: false, credits: 200, enable: true })
  );
}

// 批量查询所有账号的今日签到状态
export async function checkinStatusAll(): Promise<CheckinAllStatusItem[]> {
  return safeInvoke("checkin_status_all", undefined, async () => delay([]));
}

// 单个账号签到
export async function checkin(accountId: string): Promise<CheckinResult> {
  return safeInvoke("checkin", { accountId }, async () => delay({ code: 0, message: "success" }));
}

// 批量签到所有账号
export async function checkinAll(): Promise<CheckinAllResultItem[]> {
  return safeInvoke("checkin_all", undefined, async () => delay([]));
}

// 查看账号的签到请求头配置（固定值 / 账号专属虚拟设备 / 凭证 / 每次请求变化）
export async function getCheckinHeaders(
  accountId: string
): Promise<CheckinHeaderEntry[]> {
  return safeInvoke("get_checkin_headers", { accountId }, async () => delay([]));
}

// 重置所有账号的签到虚拟设备档案（v5 → v4 重新生成）
export async function resetCheckinDevices(): Promise<{ count: number }> {
  return safeInvoke("reset_checkin_devices", undefined, async () => delay({ count: 0 }));
}

// 重置单个账号的签到虚拟设备档案（被风控时单独换指纹）
export async function resetCheckinDevice(
  accountId: string
): Promise<CheckinDeviceProfile> {
  return safeInvoke("reset_checkin_device", { accountId }, async () =>
    delay({
      session_id: "",
      market_user_id: "",
      device_id: "",
      device_brand: "",
      device_type: "",
    })
  );
}

// ============ 签到配置 API ============

// 获取签到全局配置
export async function getCheckinConfig(): Promise<CheckinConfig> {
  return safeInvoke("get_checkin_config", undefined, async () => delay({
    device_id_strategy: "real_device_prefix" as DeviceIdStrategy,
    status_delay_min: 1,
    status_delay_max: 3,
    claim_delay_min: 20,
    claim_delay_max: 60,
  }));
}

// 更新签到全局配置
export async function updateCheckinConfig(config: CheckinConfig): Promise<void> {
  return safeInvoke("update_checkin_config", { config }, async () => delay(undefined));
}

// 获取「切换账号当作新设备」开关状态
export async function getSwitchAsNewDevice(): Promise<boolean> {
  return safeInvoke("get_switch_as_new_device", undefined, async () => delay(false));
}

// 设置「切换账号当作新设备」开关状态（即时持久化生效）
export async function setSwitchAsNewDevice(enabled: boolean): Promise<void> {
  return safeInvoke("set_switch_as_new_device", { enabled }, async () => delay(undefined));
}

// 重新生成单个账号的 device-id
export async function regenerateDeviceId(accountId: string): Promise<CheckinDeviceProfile> {
  return safeInvoke("regenerate_device_id", { accountId }, async () =>
    delay({ session_id: "", market_user_id: "", device_id: "", device_brand: "", device_type: "" })
  );
}

// 更换单个账号的虚拟设备型号
export async function swapDeviceBrand(accountId: string): Promise<CheckinDeviceProfile> {
  return safeInvoke("swap_device_brand", { accountId }, async () =>
    delay({ session_id: "", market_user_id: "", device_id: "", device_brand: "", device_type: "" })
  );
}

// ============ 浏览器登录 ============

// 打开浏览器登录窗口
export async function startBrowserLogin(): Promise<void> {
  return safeInvoke("start_browser_login", undefined, async () => delay(undefined));
}
