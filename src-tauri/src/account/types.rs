use serde::{Deserialize, Serialize};

/// 账号的"签到虚拟设备"档案
///
/// 模拟该账号登录在一台独立虚拟设备上进行每日签到：
/// 添加账号时一次性生成并持久化，此后（无论隔多少天）签到/查状态
/// 请求始终携带同一组值，与真实用户"固定在一台设备上签到"的行为一致。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckinDeviceProfile {
    /// vscode-sessionid：32 位 hex 会话 ID（账号专属）
    pub session_id: String,
    /// x-market-user-id：即 Trae 客户端 machineid 文件内容（UUID）
    pub market_user_id: String,
    /// x-device-id：16 位十进制设备 ID
    pub device_id: String,
    /// x-device-brand：设备型号（从 mac / windows 真实型号池中选定）
    pub device_brand: String,
    /// x-device-type：mac / windows（均有真实抓包佐证）
    pub device_type: String,
}

/// 设备型号池：macOS 型号（x-device-type: "mac"）与 Windows 型号（x-device-type: "windows"）
/// 均有真实抓包佐证。Windows 的 x-device-brand 为主板型号（如抓包实测 "Z390 GAMING X"）。
pub(crate) const DEVICE_MODELS: &[(&str, &str)] = &[
    // (x-device-brand, x-device-type)
    // —— macOS（真实型号）——
    ("MacBookAir10,1", "mac"),
    ("MacBookAir10,2", "mac"),
    ("MacBookPro18,3", "mac"),
    ("MacBookPro18,4", "mac"),
    ("MacBookPro16,1", "mac"),
    ("Mac14,2", "mac"),
    ("Mac14,3", "mac"),
    ("MacBookPro14,3", "mac"),
    // —— Windows（真实主板型号，x-device-brand 即主板型号）——
    ("Z390 GAMING X", "windows"),
    ("B760M-A GAMING WIFI", "windows"),
    ("B660M MORTAR WIFI", "windows"),
    ("H610M PLUS", "windows"),
    ("X570 AORUS ELITE", "windows"),
];

impl CheckinDeviceProfile {
    /// 为账号生成专属的签到虚拟设备档案
    ///
    /// - session_id：随机 32 位 hex（生成后持久化，不再变化）
    /// - market_user_id：UUID v4（对齐真实 machineid 版本位），同账号稳定、跨账号唯一
    /// - device_id：按可配置策略生成（策略一 = 真实设备前缀 + 随机后缀；策略二 = FNV 映射到安全区间）
    /// - device_brand / device_type：按账号哈希从型号池选定（每个账号一台独立"设备"）
    pub fn generate(account_id: &str, strategy: Option<DeviceIdStrategy>, real_device_id: Option<&str>) -> Self {
        use uuid::Uuid;

        // vscode-sessionid：真实客户端为 32 位 hex（等价于无横线 UUID v4）
        let session_id = Uuid::new_v4().simple().to_string();

        // x-market-user-id：与 machineid 文件同格式的 UUID v4
        // 注意：真实客户端 machineid 生成自 Uuid::new_v4()（版本位=4），
        // 用 v5 会导致版本位=5，与服务端认知的真实机器码格式不符（实测 9074）。
        // 生成一次后持久化，跨会话稳定。
        let market_user_id = Uuid::new_v4().to_string();

        // x-device-id：根据策略生成
        // 实测服务端对 device-id 做纯数值上限校验（claim 接口）：不看首位、不看位数、低端无下限，
        // 合法上限紧贴 2^52（4503599627370496）附近 —— 4.5e15 合法、2^52-1 非法；超限返回 9074。
        let strategy = strategy.unwrap_or_default();
        let device_id = match strategy {
            DeviceIdStrategy::RealDevicePrefix => {
                // 策略一：读取本机真实 device-id，取前 13 位 + 3 位随机后缀
                if let Some(real_id) = real_device_id {
                    if real_id.len() >= 13 {
                        let prefix = &real_id[..13];
                        let suffix = {
                            use rand::Rng;
                            let mut rng = rand::thread_rng();
                            rng.gen_range(0u64..=999u64)
                        };
                        format!("{}{:03}", prefix, suffix)
                    } else {
                        // 真实 device-id 不足 13 位，回退 FNV 策略
                        let mut hash: u64 = 0xcbf29ce484222325;
                        for byte in account_id.bytes() {
                            hash ^= byte as u64;
                            hash = hash.wrapping_mul(0x100000001b3);
                        }
                        (1_000_000_000_000_000u64 + (hash % 3_500_000_000_000_000u64)).to_string()
                    }
                } else {
                    // 无真实 device-id，回退 FNV 策略
                    let mut hash: u64 = 0xcbf29ce484222325;
                    for byte in account_id.bytes() {
                        hash ^= byte as u64;
                        hash = hash.wrapping_mul(0x100000001b3);
                    }
                    (1_000_000_000_000_000u64 + (hash % 3_500_000_000_000_000u64)).to_string()
                }
            }
            DeviceIdStrategy::SafeRangeFNV => {
                // 策略二：FNV-1a 哈希映射到安全区间 [1e15, 4.5e15)，数值恒 < 4.5e15 合法上限
                // （实测 4.5e15 合法、2^52-1 非法，边界紧贴 2^52；保留 ~0.5e15 以上余量）
                let mut hash: u64 = 0xcbf29ce484222325;
                for byte in account_id.bytes() {
                    hash ^= byte as u64;
                    hash = hash.wrapping_mul(0x100000001b3);
                }
                (1_000_000_000_000_000u64 + (hash % 3_500_000_000_000_000u64)).to_string()
            }
        };

        // 计算 hash 用于型号选择（所有策略共享同一设备型号池）
        let mut hash: u64 = 0xcbf29ce484222325;
        for byte in account_id.bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        // device_brand：按账号哈希从型号池选定一台"虚拟设备"
        let (device_brand, device_type) = DEVICE_MODELS[(hash as usize) % DEVICE_MODELS.len()];

        Self {
            session_id,
            market_user_id,
            device_id,
            device_brand: device_brand.to_string(),
            device_type: device_type.to_string(),
        }
    }

    /// 检测 device-id 是否为旧版生成（数值 >=4.5e15）
    ///
    /// 服务端对 x-device-id 做纯数值上限校验：不看首位、不看位数、低端无下限，
    /// 只判断数值是否超过上限阈值。实测（claim 接口）4503449720527871（≈4.5034e15）
    /// 合法、2^52-1（4503599627370495）非法，合法上限紧贴 2^52 附近。
    /// 旧版 FNV 映射到 [1e15, 1e16)，其中数值 >=4.5e15 视为旧档案重生成——
    /// 保守取整阈值（[4.5e15, ~4.5034e15) 实测也合法，但重生成无副作用，
    /// 且取整阈值可抵御服务端未来收紧），命中即自动重新生成（自愈）。
    pub fn has_legacy_device_id(&self) -> bool {
        match self.device_id.parse::<u64>() {
            Ok(v) => v >= 4_500_000_000_000_000,
            Err(_) => true,
        }
    }
}

/// 设备 ID 生成策略
/// 账号在 TraeJumper 中创建/导入时的登录来源。
/// 仅新建账号打标；存量账号（旧数据）无法追溯，为 `Unknown`。
/// 用于账号列表展示"登录态来源"，并结合客户端活跃态推导「原生 OAuth / 切号注入」。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountLoginSource {
    /// 从 Trae 客户端已登录状态导入
    ClientImport,
    /// 内部 WebView 登录（登录窗口）
    Webview,
    /// Cookie 登录（普通 cookies 导入）
    Cookie,
    /// 手动 Token 导入
    ManualToken,
    /// 旧数据无来源记录
    Unknown,
}

impl Default for AccountLoginSource {
    fn default() -> Self {
        Self::Unknown
    }
}

impl AccountLoginSource {
    /// 来源的展示名（用于列表标签）
    pub fn label(self) -> &'static str {
        match self {
            AccountLoginSource::ClientImport => "客户端导入",
            AccountLoginSource::Webview => "WebView登录",
            AccountLoginSource::Cookie => "Cookie登录",
            AccountLoginSource::ManualToken => "手动Token",
            AccountLoginSource::Unknown => "未分类",
        }
    }
}

/// 账号在客户端/库内的"登录态类型"，由 AccountManager 结合客户端活跃态推导。
/// 供账号列表展示来源标签。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountLoginType {
    /// 客户端原生 OAuth 登录（客户端自管，非 TraeJumper 切号注入）
    NativeOAuth,
    /// TraeJumper 切号注入（来源见 `login_source`）
    Injected,
    /// 当前未在任何客户端登录，仅存在于 TraeJumper 库内（展示纯来源）
    Standalone,
}

impl Default for AccountLoginType {
    fn default() -> Self {
        Self::Standalone
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceIdStrategy {
    /// 策略一：读取本机真实 device-id，取前 13 位 + 3 位随机后缀（默认，推荐）
    RealDevicePrefix,
    /// 策略二：FNV-1a 哈希映射到安全区间 [1e15, 4.5e15)，数值恒 < 4.5e15 合法上限
    SafeRangeFNV,
}

impl Default for DeviceIdStrategy {
    fn default() -> Self {
        Self::RealDevicePrefix
    }
}

/// 签到全局配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckinConfig {
    /// 设备 ID 生成策略
    pub device_id_strategy: DeviceIdStrategy,
    /// 批量查询状态延迟范围（秒）
    pub status_delay_min: u64,
    pub status_delay_max: u64,
    /// 批量签到领取延迟范围（秒）
    pub claim_delay_min: u64,
    pub claim_delay_max: u64,
    /// 是否开启自动刷新（定时刷新账号使用量数据）
    #[serde(default)]
    pub auto_refresh_enabled: bool,
    /// 自动刷新间隔（分钟），可选值：5 / 10 / 30 / 60
    #[serde(default = "default_auto_refresh_interval")]
    pub auto_refresh_interval: u32,
    /// 是否开启自动签到（晚于设定时间后自动为未签到账号签到）
    #[serde(default)]
    pub auto_checkin_enabled: bool,
    /// 自动签到触发时间点（HH:mm，24 小时制），例如 "22:00"
    #[serde(default = "default_auto_checkin_time")]
    pub auto_checkin_time: String,
    /// 是否开启日志文件运行中自动重建（watchdog 每 interval 秒检查一次，
    /// 发现日志文件被外部删除/替换则自动重建并重新重定向）
    #[serde(default = "default_log_watchdog_enabled")]
    pub log_watchdog_enabled: bool,
    /// 日志 watchdog 检查间隔（秒），最小 2
    #[serde(default = "default_log_watchdog_interval")]
    pub log_watchdog_interval: u32,
}

fn default_auto_refresh_interval() -> u32 {
    10
}

fn default_auto_checkin_time() -> String {
    "22:00".to_string()
}

fn default_log_watchdog_enabled() -> bool {
    true
}

fn default_log_watchdog_interval() -> u32 {
    5
}

impl Default for CheckinConfig {
    fn default() -> Self {
        Self {
            device_id_strategy: DeviceIdStrategy::RealDevicePrefix,
            status_delay_min: 1,
            status_delay_max: 3,
            claim_delay_min: 20,
            claim_delay_max: 60,
            auto_refresh_enabled: true,
            auto_refresh_interval: 10,
            auto_checkin_enabled: false,
            auto_checkin_time: "22:00".to_string(),
            log_watchdog_enabled: true,
            log_watchdog_interval: 5,
        }
    }
}

/// 账号信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub name: String,
    pub email: String,
    pub avatar_url: String,
    pub cookies: String,
    pub jwt_token: Option<String>,
    pub token_expired_at: Option<String>,
    pub user_id: String,
    pub tenant_id: String,
    pub region: String,
    pub plan_type: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub is_active: bool,
    /// 账号关联的机器码
    #[serde(default)]
    pub machine_id: Option<String>,
    /// 签到虚拟设备档案（添加账号时分配，持久化；旧数据懒生成）
    #[serde(default)]
    pub checkin_device: Option<CheckinDeviceProfile>,
    /// 上次通过 cookies 续签 token 的时间戳（秒）。用于节流：cookie 续签至少间隔 12 小时，
    /// 同时保持 web session 活跃，避免服务端因长时间无请求而清理 session。
    #[serde(default)]
    pub last_cookie_renewal_at: Option<i64>,
    /// 当前 token 是否已共享给 Trae 客户端（通过"切换账号"写入或从客户端同步）。
    /// 单活跃 Token 策略下该字段决定"让位"判定：
    /// - true：客户端可能正持有此 token 的会话 → cookies 续签会顶掉客户端，需让位
    /// - false：token 为 TraeJumper 独占（自己续签/导入），客户端不持有 → 正常续签
    /// 客户端 usertag 仅记录"最后登录的账号"，登出后仍残留，不能单独作为
    /// "客户端登录中"的依据，必须结合本字段判断 token 归属。
    #[serde(default)]
    pub token_shared_with_client: bool,
    /// 账号在 TraeJumper 中创建/导入时的登录来源（新建打标；存量 Unknown）
    #[serde(default)]
    pub login_source: AccountLoginSource,
    /// 是否曾被 TraeJumper 切号注入/写回过客户端（持久化，不因续签重置）。
    /// token_shared_with_client 会在普通续签后清 false，无法表达"注入历史"；
    /// 本字段一旦置 true 不再清除，用于登录态类型展示（切号注入 vs 原生 OAuth）。
    #[serde(default)]
    pub has_been_injected: bool,
}

impl Account {
    pub fn new(
        name: String,
        email: String,
        cookies: String,
        user_id: String,
        tenant_id: String,
    ) -> Self {
        let now = chrono::Utc::now().timestamp();
        let id = uuid_simple();
        Self {
            checkin_device: Some(CheckinDeviceProfile::generate(&id, None, None)),
            id,
            name,
            email,
            avatar_url: String::new(),
            cookies,
            jwt_token: None,
            token_expired_at: None,
            user_id,
            tenant_id,
            region: String::new(),
            plan_type: "Free".to_string(),
            created_at: now,
            updated_at: now,
            is_active: true,
            machine_id: None,
            last_cookie_renewal_at: None,
            token_shared_with_client: false,
            login_source: AccountLoginSource::Unknown,
            has_been_injected: false,
        }
    }
}

/// 账号列表存储结构
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AccountStore {
    pub accounts: Vec<Account>,
    pub active_account_id: Option<String>,
    /// 当前 Trae IDE 正在使用的账号 ID
    #[serde(default)]
    pub current_account_id: Option<String>,
    /// 签到全局配置
    #[serde(default)]
    pub checkin_config: Option<CheckinConfig>,
    /// 切换账号时是否当作全新设备（清理客户端本地数据，如最近项目历史等）。
    /// false（默认）= 仅替换登录身份，保留本地数据；true = 清理客户端本地数据，模拟新设备登录。
    #[serde(default)]
    pub switch_as_new_device: bool,
}

/// 简单的 UUID 生成
fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap();
    format!("{:x}{:x}", duration.as_secs(), duration.subsec_nanos())
}

/// 账号简要信息（用于列表展示）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountBrief {
    pub id: String,
    pub name: String,
    pub email: String,
    pub avatar_url: String,
    pub plan_type: String,
    pub is_active: bool,
    pub created_at: i64,
    /// 账号关联的机器码
    pub machine_id: Option<String>,
    /// 是否是当前 Trae IDE 正在使用的账号
    pub is_current: bool,
    /// 是否是当前 Trae 客户端正在登录的账号（单活跃 Token 下客户端持有有效会话，
    /// TraeJumper 侧 Token 失效属预期让位行为，不应显示"已过期"）
    #[serde(default)]
    pub is_client_active: bool,
    /// 该账号正在哪些 Trae 客户端中登录（显示名称列表，如 ["TraeCode CN", "TraeWork CN"]）。
    /// 用于前端展示多客户端"当前"标签，如 "TraeCode当前" / "TraeCode & TraeWork 当前"。
    #[serde(default)]
    pub active_in_clients: Vec<String>,
    /// 是否有 cookies（浏览器导入的账号才有，可进行 cookies 续签；
    /// 客户端导入/手动Token的账号无 cookies，无法续签）
    #[serde(default)]
    pub has_cookies: bool,
    /// Token 过期时间
    pub token_expired_at: Option<String>,
    /// 上次通过 cookies 续签 token 的时间戳（秒）。cookies 本身无明确过期时间，
    /// 但需每 12 小时续签一次以保持 web session 活跃，此字段展示"上次续签时间"。
    #[serde(default)]
    pub last_cookie_renewal_at: Option<i64>,
    /// 账号在 TraeJumper 中创建/导入时的登录来源（来自 Account.login_source）
    #[serde(default)]
    pub login_source: AccountLoginSource,
    /// 账号的"登录态类型"：客户端原生 OAuth / TraeJumper 切号注入 / 仅库内。
    /// 由 AccountManager.get_accounts 结合客户端活跃态与 token_shared_with_client 推导。
    #[serde(default)]
    pub login_type: AccountLoginType,
}

impl From<&Account> for AccountBrief {
    fn from(account: &Account) -> Self {
        Self {
            id: account.id.clone(),
            name: account.name.clone(),
            email: account.email.clone(),
            avatar_url: account.avatar_url.clone(),
            plan_type: account.plan_type.clone(),
            is_active: account.is_active,
            created_at: account.created_at,
            machine_id: account.machine_id.clone(),
            is_current: false, // 默认为 false，由 AccountManager 设置
            is_client_active: false, // 默认为 false，由 AccountManager 设置
            active_in_clients: Vec::new(), // 默认为空，由 AccountManager 设置
            has_cookies: !account.cookies.is_empty(),
            token_expired_at: account.token_expired_at.clone(),
            last_cookie_renewal_at: account.last_cookie_renewal_at,
            login_source: account.login_source,
            login_type: AccountLoginType::Standalone, // 默认为仅库内，由 AccountManager 设置
        }
    }
}

impl AccountBrief {
    /// 从 Account 创建 AccountBrief，并设置 is_current 标记
    pub fn from_account(account: &Account, is_current: bool) -> Self {
        Self {
            id: account.id.clone(),
            name: account.name.clone(),
            email: account.email.clone(),
            avatar_url: account.avatar_url.clone(),
            plan_type: account.plan_type.clone(),
            is_active: account.is_active,
            created_at: account.created_at,
            machine_id: account.machine_id.clone(),
            is_current,
            is_client_active: false, // 默认为 false，由 AccountManager 设置
            active_in_clients: Vec::new(), // 默认为空，由 AccountManager 设置
            has_cookies: !account.cookies.is_empty(),
            token_expired_at: account.token_expired_at.clone(),
            last_cookie_renewal_at: account.last_cookie_renewal_at,
            login_source: account.login_source,
            login_type: AccountLoginType::Standalone, // 默认为仅库内，由 AccountManager 设置
        }
    }
}

/// 当前目标客户端已登录账号的标识（前端"从客户端读取更新 Token"预检/展示用）。
/// 与账号库无关：无论该账号是否已被管理，都能读取客户端当前登录态。
#[derive(Debug, Clone, Serialize)]
pub struct CurrentClientLogin {
    pub user_id: String,
    pub email: Option<String>,
}
