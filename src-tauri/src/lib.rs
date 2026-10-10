mod api;
mod account;
mod machine;
mod login;
mod crypto;
mod trae_app;

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{self, Command};
use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::{
    AppHandle,
    Emitter,
    Manager,
    State,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{TrayIconBuilder, TrayIconEvent, MouseButton, MouseButtonState},
};

use account::{Account, AccountBrief, AccountManager, CheckinConfig, CheckinDeviceProfile, CurrentClientLogin};
use api::{CheckinHeaderEntry, CreditSummary, UsageQueryResponse, UsageSummary};

/// 应用状态
pub struct AppState {
    pub account_manager: Arc<Mutex<AccountManager>>,
}

/// 错误类型
#[derive(Debug, serde::Serialize)]
pub struct ApiError {
    pub message: String,
}

impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> Self {
        Self {
            message: err.to_string(),
        }
    }
}

type Result<T> = std::result::Result<T, ApiError>;

// ============ Tauri 命令 ============

/// 添加账号（通过 Token，可选 Cookies）
#[tauri::command]
async fn add_account_by_token(token: String, cookies: Option<String>, state: State<'_, AppState>) -> Result<Account> {
    let mut manager = state.account_manager.lock().await;
    manager.add_account_by_token(token, cookies, None, account::AccountLoginSource::ManualToken).await.map_err(Into::into)
}

/// 删除账号
#[tauri::command]
async fn remove_account(account_id: String, state: State<'_, AppState>) -> Result<()> {
    let mut manager = state.account_manager.lock().await;
    manager.remove_account(&account_id).map_err(Into::into)
}

/// 获取所有账号
#[tauri::command]
async fn get_accounts(state: State<'_, AppState>) -> Result<Vec<AccountBrief>> {
    let manager = state.account_manager.lock().await;
    Ok(manager.get_accounts())
}

/// 获取单个账号详情
#[tauri::command]
async fn get_account(account_id: String, state: State<'_, AppState>) -> Result<Account> {
    let manager = state.account_manager.lock().await;
    manager.get_account(&account_id).map_err(Into::into)
}

/// 切换账号（设置活跃账号并更新机器码）
/// `force=true` 时跳过跨客户端活跃冲突检测（用户确认接受会中断另一客户端会话）
#[tauri::command]
async fn switch_account(account_id: String, force: Option<bool>, state: State<'_, AppState>) -> Result<()> {
    let mut manager = state.account_manager.lock().await;
    manager.switch_account(&account_id, force.unwrap_or(false)).await.map_err(Into::into)
}

/// 手动「续签并写回客户端」：对指定账号立即 GetUserToken 续签并写入
/// 该账号活跃的客户端 storage.json（不重启运行中的客户端）。
/// 用于实测"写回是否触发客户端 clearUserInfo 登出"，无需等待 8 小时 token 过期。
#[tauri::command]
async fn renew_token_and_write_client(account_id: String, state: State<'_, AppState>) -> Result<String> {
    let mut manager = state.account_manager.lock().await;
    manager.refresh_token_and_write_client(&account_id).await.map_err(|e| ApiError { message: e.to_string() })?;
    Ok("已续签并写入客户端 storage.json（未重启客户端）".to_string())
}

/// 获取账号使用量
#[tauri::command]
async fn get_account_usage(account_id: String, state: State<'_, AppState>) -> Result<UsageSummary> {
    let mut manager = state.account_manager.lock().await;
    manager.get_account_usage(&account_id).await.map_err(Into::into)
}

/// 获取账号积分汇总（CN / WORK 优先积分体系，自动回退旧配额 UsageSummary）
#[tauri::command]
async fn get_account_credits(account_id: String, state: State<'_, AppState>) -> Result<CreditSummary> {
    let mut manager = state.account_manager.lock().await;
    manager.get_account_credits(&account_id).await.map_err(Into::into)
}

/// 更新账号 Token
#[tauri::command]
async fn update_account_token(account_id: String, token: String, state: State<'_, AppState>) -> Result<UsageSummary> {
    let mut manager = state.account_manager.lock().await;
    manager.update_account_token(&account_id, token).await.map_err(Into::into)
}

/// 导出账号
#[tauri::command]
async fn export_accounts(state: State<'_, AppState>) -> Result<String> {
    let manager = state.account_manager.lock().await;
    manager.export_accounts().map_err(Into::into)
}

/// 导入账号
#[tauri::command]
async fn import_accounts(data: String, state: State<'_, AppState>) -> Result<usize> {
    let mut manager = state.account_manager.lock().await;
    manager.import_accounts(&data).await.map_err(Into::into)
}

/// 清空所有账号数据
#[tauri::command]
async fn clear_all_accounts(state: State<'_, AppState>) -> Result<usize> {
    let mut manager = state.account_manager.lock().await;
    manager.clear_all_accounts().map_err(Into::into)
}

/// 获取使用事件
#[tauri::command]
async fn get_usage_events(
    account_id: String,
    start_time: i64,
    end_time: i64,
    page_num: i32,
    page_size: i32,
    state: State<'_, AppState>
) -> Result<UsageQueryResponse> {
    let mut manager = state.account_manager.lock().await;
    manager.get_usage_events(&account_id, start_time, end_time, page_num, page_size)
        .await
        .map_err(Into::into)
}

/// 从 Trae IDE号
#[tauri::command]
async fn read_trae_account(state: State<'_, AppState>) -> Result<Option<Account>> {
    let mut manager = state.account_manager.lock().await;
    manager.read_trae_ide_account().await.map_err(Into::into)
}

/// 获取当前系统机器码
#[tauri::command]
async fn get_machine_id() -> Result<String> {
    machine::get_machine_guid().map_err(Into::into)
}

/// 重置系统机器码（生成新的随机机器码）
#[tauri::command]
async fn reset_machine_id() -> Result<String> {
    machine::reset_machine_guid().map_err(Into::into)
}

/// 设置系统机器码为指定值
#[tauri::command]
async fn set_machine_id(machine_id: String) -> Result<()> {
    machine::set_machine_guid(&machine_id).map_err(Into::into)
}

/// 绑定账号机器码（保存当前系统机器码到账号）
#[tauri::command]
async fn bind_account_machine_id(account_id: String, state: State<'_, AppState>) -> Result<String> {
    let mut manager = state.account_manager.lock().await;
    manager.bind_machine_id(&account_id).map_err(Into::into)
}

/// 获取 Trae IDE 的机器码
#[tauri::command]
async fn get_trae_machine_id() -> Result<String> {
    machine::get_trae_machine_id().map_err(Into::into)
}

/// 设置 Trae IDE 的机器码
#[tauri::command]
async fn set_trae_machine_id(machine_id: String) -> Result<()> {
    machine::set_trae_machine_id(&machine_id).map_err(Into::into)
}

/// 读取 Trae 客户端的本机真实 device-id（ahanet/tt_net_config.config）
#[tauri::command]
async fn get_trae_device_id() -> Result<String> {
    machine::get_trae_device_id().map_err(Into::into)
}

/// 清除 Trae IDE 登录状态（让 IDE 变成全新安装状态）
#[tauri::command]
async fn clear_trae_login_state() -> Result<()> {
    machine::clear_trae_login_state().map_err(Into::into)
}

/// 获取保存的 Trae IDE 路径
#[tauri::command]
async fn get_trae_path() -> Result<String> {
    machine::get_saved_trae_path().map_err(Into::into)
}

/// 设置 Trae IDE 路径
#[tauri::command]
async fn set_trae_path(path: String) -> Result<()> {
    machine::save_trae_path(&path).map_err(Into::into)
}

/// 自动扫描 Trae IDE 路径
#[tauri::command]
async fn scan_trae_path() -> Result<String> {
    machine::scan_trae_path().map_err(Into::into)
}

/// 刷新单个账号 Token
#[tauri::command]
async fn refresh_token(account_id: String, state: State<'_, AppState>) -> Result<()> {
    let mut manager = state.account_manager.lock().await;
    manager.refresh_token(&account_id).await.map_err(Into::into)
}

/// 批量刷新所有即将过期的 Token
#[tauri::command]
async fn refresh_all_tokens(state: State<'_, AppState>) -> Result<Vec<String>> {
    let mut manager = state.account_manager.lock().await;
    manager.refresh_all_tokens().await.map_err(Into::into)
}

/// 领取礼包
#[tauri::command]
async fn claim_gift(account_id: String, state: State<'_, AppState>) -> Result<()> {
    let mut manager = state.account_manager.lock().await;
    manager.claim_birthday_bonus(&account_id).await.map_err(Into::into)
}

/// 查询单个账号今日签到状态
#[tauri::command]
async fn checkin_status(
    account_id: String,
    state: State<'_, AppState>,
) -> Result<serde_json::Value> {
    let mut manager = state.account_manager.lock().await;
    let result = manager
        .checkin_status(&account_id)
        .await
        .map_err(ApiError::from)?;
    serde_json::to_value(result).map_err(|e| ApiError { message: e.to_string() }.into())
}

/// 重置所有账号的签到虚拟设备档案（v5 → v4 重新生成）
#[tauri::command]
async fn reset_checkin_devices(state: State<'_, AppState>) -> Result<serde_json::Value> {
    let mut manager = state.account_manager.lock().await;
    let count = manager.reset_checkin_devices().map_err(ApiError::from)?;
    Ok(serde_json::json!({ "count": count }))
}

/// 重置单个账号的签到虚拟设备档案（被风控时单独换指纹）
#[tauri::command]
async fn reset_checkin_device(
    account_id: String,
    state: State<'_, AppState>,
) -> Result<serde_json::Value> {
    let mut manager = state.account_manager.lock().await;
    let profile = manager
        .reset_checkin_device(&account_id)
        .map_err(ApiError::from)?;
    serde_json::to_value(profile).map_err(|e| ApiError { message: e.to_string() }.into())
}

/// 获取签到全局配置
#[tauri::command]
async fn get_checkin_config(state: State<'_, AppState>) -> Result<CheckinConfig> {
    let manager = state.account_manager.lock().await;
    Ok(manager.get_checkin_config())
}

/// 更新签到全局配置
#[tauri::command]
async fn update_checkin_config(config: CheckinConfig, state: State<'_, AppState>) -> Result<()> {
    let mut manager = state.account_manager.lock().await;
    manager
        .update_checkin_config(config.clone())
        .map_err(|e| ApiError { message: e.to_string() })?;
    // 同步日志 watchdog 开关与检查间隔到全局原子（立即生效，无需重启）
    sync_log_watchdog_config(config.log_watchdog_enabled, config.log_watchdog_interval);
    Ok(())
}

/// 获取「切换账号当作新设备」开关状态
#[tauri::command]
async fn get_switch_as_new_device(state: State<'_, AppState>) -> Result<bool> {
    let manager = state.account_manager.lock().await;
    Ok(manager.get_switch_as_new_device())
}

/// 设置「切换账号当作新设备」开关状态（即时持久化生效）
#[tauri::command]
async fn set_switch_as_new_device(enabled: bool, state: State<'_, AppState>) -> Result<()> {
    let mut manager = state.account_manager.lock().await;
    manager.set_switch_as_new_device(enabled).map_err(Into::into)
}

/// 重新生成单个账号的 device-id（保持其他字段不变）
#[tauri::command]
async fn regenerate_device_id(account_id: String, state: State<'_, AppState>) -> Result<CheckinDeviceProfile> {
    let mut manager = state.account_manager.lock().await;
    manager.regenerate_device_id(&account_id).map_err(Into::into)
}

/// 更换单个账号的虚拟设备型号（从型号池重新随机分配，不改变其他字段）
#[tauri::command]
async fn swap_device_brand(account_id: String, state: State<'_, AppState>) -> Result<CheckinDeviceProfile> {
    let mut manager = state.account_manager.lock().await;
    manager.swap_device_brand(&account_id).map_err(Into::into)
}

/// 批量查询所有账号的今日签到状态
#[tauri::command]
async fn checkin_status_all(
    state: State<'_, AppState>,
) -> Result<serde_json::Value> {
    let mut manager = state.account_manager.lock().await;
    let results = manager
        .checkin_status_all()
        .await
        .map_err(ApiError::from)?;
    let mapped: Vec<serde_json::Value> = results
        .into_iter()
        .map(|(id, name, status)| match status {
            Some(s) => serde_json::json!({
                "account_id": id,
                "account_name": name,
                "code": s.code,
                "message": s.message,
                "checked_in": s.checked_in,
                "credits": s.credits,
                "enable": s.enable,
            }),
            None => serde_json::json!({
                "account_id": id,
                "account_name": name,
                "code": -1,
                "message": "无 Token 或查询失败",
                "checked_in": false,
                "credits": 0,
                "enable": false,
            }),
        })
        .collect();
    Ok(serde_json::Value::Array(mapped))
}

/// 单个账号签到
#[tauri::command]
async fn checkin(account_id: String, state: State<'_, AppState>) -> Result<serde_json::Value> {
    let mut manager = state.account_manager.lock().await;
    let result = manager.checkin(&account_id).await.map_err(ApiError::from)?;
    serde_json::to_value(result).map_err(|e| ApiError { message: e.to_string() }.into())
}

/// 批量签到所有账号
#[tauri::command]
async fn checkin_all(state: State<'_, AppState>) -> Result<serde_json::Value> {
    let mut manager = state.account_manager.lock().await;
    let results = manager.checkin_all().await.map_err(ApiError::from)?;
    let mapped: Vec<serde_json::Value> = results
        .into_iter()
        .map(|(id, name, result, already_checked)| {
            serde_json::json!({
                "account_id": id,
                "account_name": name,
                "code": result.code,
                "message": result.message,
                "skipped": already_checked, // true=已签到跳过 false=实际执行了 claim
            })
        })
        .collect();
    Ok(serde_json::Value::Array(mapped))
}

/// 查看账号的签到请求头配置（固定值 / 账号专属虚拟设备 / 凭证 / 每次请求变化）
#[tauri::command]
async fn get_checkin_headers(
    account_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<CheckinHeaderEntry>> {
    let mut manager = state.account_manager.lock().await;
    manager
        .get_checkin_header_preview(&account_id)
        .map_err(ApiError::from)
}

/// 浏览器登录
#[tauri::command]
async fn start_browser_login(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<()> {
    let manager = state.account_manager.clone();
    login::start_login_flow(app, manager, None).await.map_err(|e| ApiError { message: e })?;
    Ok(())
}

/// 浏览器登录并更新指定账号的 Token（登录后校验同一用户并更新）
#[tauri::command]
async fn start_browser_login_for_update(
    account_id: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<()> {
    let manager = state.account_manager.clone();
    login::start_login_flow(app, manager, Some(account_id))
        .await
        .map_err(|e| ApiError { message: e })?;
    Ok(())
}

/// 通过读取当前 Trae 客户端登录态自动更新指定账号的 Token
#[tauri::command]
async fn update_account_token_from_client(
    account_id: String,
    state: State<'_, AppState>,
) -> Result<UsageSummary> {
    let mut manager = state.account_manager.lock().await;
    manager
        .update_account_token_from_client(&account_id)
        .await
        .map_err(Into::into)
}

/// 读取当前目标客户端已登录账号的标识（user_id + email），用于"从客户端读取更新 Token"的前端预检/展示
#[tauri::command]
async fn current_client_login(state: State<'_, AppState>) -> Result<Option<CurrentClientLogin>> {
    let manager = state.account_manager.lock().await;
    manager.current_client_login().await.map_err(Into::into)
}

/// 探测登录窗口当前 Token 与账号（手动刷新调用）
#[tauri::command]
async fn probe_login_webview(app: AppHandle) -> Result<Option<login::LoginWebviewProbe>> {
    login::probe_login_webview_inner(&app).await.map_err(Into::into)
}

/// 应用登录窗口当前 Token 到指定账号（手动确认调用，更新场景）
#[tauri::command]
async fn apply_login_webview_token(
    account_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<UsageSummary> {
    let manager = state.account_manager.clone();
    login::apply_login_webview_token_inner(&app, manager, &account_id)
        .await
        .map_err(Into::into)
}

/// 应用登录窗口当前 Token 新增账号（手动确认调用，新增场景）
#[tauri::command]
async fn apply_login_webview_new_account(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String> {
    login::apply_login_webview_new_account_inner(&app, state.account_manager.clone())
        .await
        .map_err(Into::into)
}

/// 关闭登录窗口（用户取消或关闭弹窗时调用）
#[tauri::command]
async fn close_login_webview(app: AppHandle) -> Result<()> {
    login::close_login_webview_internal(&app);
    Ok(())
}

/// 主窗口 resize 时由前端调用，重设登录子 webview 的位置和尺寸。
/// 无登录子 webview 时静默忽略。
#[tauri::command]
async fn resize_login_child_webview(app: AppHandle) -> Result<()> {
    if let Some(main_window) = app.get_window("main") {
        let _ = login::place_login_child_webview(&main_window);
    }
    Ok(())
}

/// 获取支持的 Trae 应用列表（含安装状态与当前选择）
#[tauri::command]
async fn get_trae_apps() -> Result<Vec<trae_app::TraeAppInfo>> {
    Ok(trae_app::list_app_infos())
}

/// 切换当前管理的目标应用（TraeCode CN / TraeWork CN / 国际版）
#[tauri::command]
async fn set_current_trae_app(app_key: String) -> Result<()> {
    let variant = trae_app::find_variant(&app_key).map_err(ApiError::from)?;
    trae_app::set_current(variant).map_err(ApiError::from)?;
    // 切换后自动扫描并保存该客户端的安装路径，避免沿用上一个客户端的旧路径
    // （Windows 不支持自动扫描，保留用户手动设置的路径）
    #[cfg(target_os = "macos")]
    {
        match machine::scan_trae_path() {
            Ok(path) => {
                let _ = machine::save_trae_path(&path);
            }
            Err(_) => {
                let _ = machine::clear_saved_trae_path();
            }
        }
    }
    Ok(())
}

/// 同步当前账号状态：读取当前目标 Trae 客户端已登录账号，更新 current_account_id。
/// 切换目标客户端后调用，确保显示的当前账号与客户端一致。
#[tauri::command]
async fn sync_current_account(state: State<'_, AppState>) -> Result<Option<AccountBrief>> {
    let mut manager = state.account_manager.lock().await;
    manager.sync_current_account().map_err(Into::into)
}

/// 初始化日志文件，将 stdout/stderr 重定向到文件，使所有 println!/eprintln! 输出落盘。
///
/// 日志路径（遵循 macOS / Windows 惯例）：
/// - macOS: ~/Library/Logs/traejumper/trae-jumper.log（Console.app 默认可见）
/// - Windows: %LOCALAPPDATA%\traejumper\logs\trae-jumper.log
///
/// 超过 5MB 时自动轮转（旧日志重命名为 .old）。
fn init_logging() {
    // macOS 惯例: ~/Library/Logs/{app}/（Console.app 默认可见）
    // Windows 惯例: %LOCALAPPDATA%\{app}\logs\
    let log_dir: PathBuf = {
        let base = directories::BaseDirs::new()
            .map(|b| b.home_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        #[cfg(target_os = "macos")]
        { base.join("Library/Logs/traejumper") }
        #[cfg(target_os = "windows")]
        { base.join("AppData/Local/traejumper/logs") }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        { base.join(".local/share/traejumper/logs") }
    };
    let _ = fs::create_dir_all(&log_dir);
    let log_path = log_dir.join("trae-jumper.log");

    // 超过 5MB 时轮转
    if let Ok(meta) = fs::metadata(&log_path) {
        if meta.len() > 5 * 1024 * 1024 {
            let _ = fs::rename(&log_path, log_dir.join("trae-jumper.log.old"));
        }
    }

    let file = match fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        Ok(f) => f,
        Err(_) => return,
    };

    // 将 stdout (fd 1) 和 stderr (fd 2) 重定向到日志文件
    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;
        let fd = file.as_raw_fd();
        unsafe {
            libc::dup2(fd, 1);
            libc::dup2(fd, 2);
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::System::Console::{SetStdHandle, STD_ERROR_HANDLE, STD_OUTPUT_HANDLE};
        let handle = file.as_raw_handle();
        unsafe {
            SetStdHandle(STD_OUTPUT_HANDLE, handle);
            SetStdHandle(STD_ERROR_HANDLE, handle);
        }
    }

    // 注意：file 必须 leak，否则 drop 后 fd 关闭，后续 println 会失败
    std::mem::forget(file);

    println!(
        "\n========== Trae Jumper 启动 {} (PID {}) ==========",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        std::process::id()
    );

    // 运行中监控：日志文件被外部删除/替换时自动重建并重新重定向，
    // 避免进程持有已删除 inode 的 fd 导致日志"消失但无任何报错"。
    start_log_watchdog(log_path);
}

/// 日志 watchdog 开关（设置页可配，默认开启）
static LOG_WATCHDOG_ENABLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
/// 日志 watchdog 检查间隔（秒，最小 2，设置页可配，默认 5）
static LOG_WATCHDOG_INTERVAL: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(5);

/// 同步日志 watchdog 配置（应用启动加载配置后与设置更新时调用）
pub fn sync_log_watchdog_config(enabled: bool, interval: u32) {
    use std::sync::atomic::Ordering;
    LOG_WATCHDOG_ENABLED.store(enabled, Ordering::Relaxed);
    LOG_WATCHDOG_INTERVAL.store(interval.max(2), Ordering::Relaxed);
}

/// 监控日志文件是否被外部删除/替换；是则自动重建文件并重新 dup2 重定向 stdout/stderr。
///
/// 背景：init_logging 仅在启动时打开一次日志 fd 并 forget，此后 println 一直写同一个
/// fd。若用户/清理工具删除了日志文件甚至整个目录，进程持有的 fd 仍指向已删除的
/// inode，写入静默成功但磁盘上不可见，且不会自动重建。本线程按配置间隔（默认 5 秒，
/// 设置页可调整/关闭）检查路径的 (dev, ino)，发现文件丢失或被替换（如轮转）即重建
/// 目录/文件并重新重定向。
#[cfg(unix)]
fn start_log_watchdog(log_path: PathBuf) {
    use std::os::unix::fs::MetadataExt;
    use std::os::unix::io::AsRawFd;
    use std::sync::atomic::Ordering;

    std::thread::spawn(move || {
        // 当前日志文件的 (dev, ino)，None 表示路径不存在
        let mut cur = fs::metadata(&log_path).ok().map(|m| (m.dev(), m.ino()));
        loop {
            let interval = LOG_WATCHDOG_INTERVAL.load(Ordering::Relaxed).max(2) as u64;
            std::thread::sleep(std::time::Duration::from_secs(interval));
            // 开关关闭时跳过检查（线程保持存活，避免频繁创建/销毁）
            if !LOG_WATCHDOG_ENABLED.load(Ordering::Relaxed) {
                continue;
            }
            let now = fs::metadata(&log_path).ok().map(|m| (m.dev(), m.ino()));
            // 文件未变：继续
            if cur.is_some() && cur == now {
                continue;
            }
            // 文件被删或替换 → 重建目录与文件
            if let Some(dir) = log_path.parent() {
                let _ = fs::create_dir_all(dir);
            }
            if let Ok(f) = fs::OpenOptions::new().create(true).append(true).open(&log_path) {
                let fd = f.as_raw_fd();
                unsafe {
                    libc::dup2(fd, 1);
                    libc::dup2(fd, 2);
                }
                std::mem::forget(f); // 与 init_logging 一致，保持 fd 不关闭
                cur = fs::metadata(&log_path).ok().map(|m| (m.dev(), m.ino()));
                println!(
                    "\n[{}] [INFO log] 检测到日志文件被外部删除/替换，已自动重建并重新重定向 (PID {})",
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                    std::process::id()
                );
            } else {
                // 重建失败（如权限异常），置为 None 以便下轮重试
                cur = None;
            }
        }
    });
}

#[cfg(not(unix))]
fn start_log_watchdog(_log_path: PathBuf) {
    // Windows 暂不实现运行中日志重建（写入仍走启动时打开的 fd）
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // ---- 初始化日志（在所有其他逻辑之前，确保后续 println! 都落盘）----
    init_logging();

    // ---- 单实例锁检测 ----
    if let Some(existing_pid) = try_acquire_lock() {
        println!("[INFO] 检测到已有 Trae Jumper 实例运行中 (PID: {}), 正在唤起...", existing_pid);
        // macOS: 激活已有实例的窗口
        #[cfg(target_os = "macos")]
        {
            let _ = Command::new("osascript")
                .args(["-e", "tell application \"Trae Jumper\" to activate"])
                .output();
        }
        // Windows: 通过 PowerShell 激活
        #[cfg(target_os = "windows")]
        {
            let _ = Command::new("powershell")
                .args(["-Command", "Add-Type '[DllImport(\"user32.dll\")]public static extern bool SetForegroundWindow(IntPtr hWnd);'; $proc = Get-Process -Name 'Trae Jumper' -ErrorAction SilentlyContinue; if ($proc) { [SetForegroundWindow]::Invoke($proc.MainWindowHandle) }"])
                .output();
        }
        println!("[INFO] 已唤起已有实例, 当前实例退出");
        return;
    }

    let account_manager = AccountManager::new().expect("无法初始化账号管理器");

    // 启动时同步日志 watchdog 配置（开关默认开启，间隔默认 5 秒）
    let cfg = account_manager.get_checkin_config();
    sync_log_watchdog_config(cfg.log_watchdog_enabled, cfg.log_watchdog_interval);

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(AppState {
            account_manager: Arc::new(Mutex::new(account_manager)),
        })
        .invoke_handler(tauri::generate_handler![
            add_account_by_token,
            remove_account,
            get_accounts,
            get_account,
            switch_account,
            renew_token_and_write_client,
            get_account_usage,
            get_account_credits,
            update_account_token,
            export_accounts,
            import_accounts,
            clear_all_accounts,
            get_usage_events,
            read_trae_account,
            get_machine_id,
            reset_machine_id,
            set_machine_id,
            bind_account_machine_id,
            get_trae_machine_id,
            set_trae_machine_id,
            get_trae_device_id,
            clear_trae_login_state,
            get_trae_path,
            set_trae_path,
            scan_trae_path,
            claim_gift,
            checkin_status,
            checkin_status_all,
            checkin,
            checkin_all,
            get_checkin_headers,
            reset_checkin_devices,
            reset_checkin_device,
            get_checkin_config,
            update_checkin_config,
            get_switch_as_new_device,
            set_switch_as_new_device,
            regenerate_device_id,
            swap_device_brand,
            refresh_token,
            refresh_all_tokens,
            start_browser_login,
            start_browser_login_for_update,
            update_account_token_from_client,
            current_client_login,
            probe_login_webview,
            apply_login_webview_token,
            apply_login_webview_new_account,
            close_login_webview,
            resize_login_child_webview,
            get_trae_apps,
            set_current_trae_app,
            sync_current_account,
        ])
        // 关闭时隐藏到系统托盘，不退出
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // 如果正在登录流程中，允许正常关闭登录窗口
                if window.label() != "main" {
                    return;
                }
                let _ = window.hide();
                api.prevent_close();
            }
        })
        // 创建系统托盘图标
        .setup(|app| {
            setup_system_tray(app)?;

            // 后端定时续签任务：不依赖前端 setInterval（电脑睡眠时前端定时器会暂停，
            // 醒来后也不会立即触发），后端用 tokio interval 每分钟检查一次，
            // 确保客户端活跃账号的 token 在过期前被续签并写回客户端。
            let account_manager = app.state::<AppState>().account_manager.clone();
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                // 等待网络就绪：每 5 秒尝试连接 api.trae.cn:443，最多等 3 分钟。
                // macOS 重启后 DNS/网络栈初始化需要时间（实测 ~2 分钟），
                // 这段时间内所有 API 请求都会因 DNS 解析失败瞬间返回 error sending request。
                // 从睡眠唤醒场景下一般几秒内即可连通，不会误等。
                {
                    let max_wait_secs = 180u64;
                    let check_interval_secs = 5u64;
                    let mut waited = 0u64;
                    loop {
                        match tokio::net::TcpStream::connect("api.trae.cn:443").await {
                            Ok(_) => {
                                if waited > 0 {
                                    eprintln!("[INFO] 网络就绪（等待 {}s 后），开始定时任务", waited);
                                }
                                break;
                            }
                            Err(e) => {
                                if waited >= max_wait_secs {
                                    eprintln!(
                                        "[WARN] 网络就绪等待超时（{}s），强制启动定时任务。最后错误: {}",
                                        waited, e
                                    );
                                    break;
                                }
                                tokio::time::sleep(tokio::time::Duration::from_secs(check_interval_secs)).await;
                                waited += check_interval_secs;
                            }
                        }
                    }
                }

                // 启动时先执行一次（覆盖从睡眠唤醒的场景）
                {
                    let mut manager = account_manager.lock().await;
                    match manager.refresh_all_tokens().await {
                        Ok(refreshed) if !refreshed.is_empty() => {
                            // 有账号续签成功 → 通知前端刷新账号列表（修复"列表显示过期但实际已续期"）
                            let _ = app_handle.emit("token-refreshed", refreshed);
                        }
                        Ok(_) => {}
                        Err(e) => eprintln!("[WARN] 启动续签失败: {}", e),
                    }
                }
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
                loop {
                    interval.tick().await;
                    let mut manager = account_manager.lock().await;
                    match manager.refresh_all_tokens().await {
                        Ok(refreshed) if !refreshed.is_empty() => {
                            let _ = app_handle.emit("token-refreshed", refreshed);
                        }
                        Ok(_) => {}
                        Err(e) => eprintln!("[WARN] 定时续签失败: {}", e),
                    }
                    // 自动签到：内部会判断时间与去重，不满足条件时立即返回
                    if let Err(e) = manager.auto_checkin().await {
                        eprintln!("[WARN] 自动签到失败: {}", e);
                    }
                }
            });

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    // 运行主事件循环
    app.run(|app_handle, event| {
        match event {
            tauri::RunEvent::ExitRequested { .. } => {
                // 退出时释放单实例锁
                release_lock();
            }
            // macOS: 点击程序坞图标时重新显示主窗口（后台隐藏后唤起）
            #[cfg(target_os = "macos")]
            tauri::RunEvent::Reopen {
                has_visible_windows,
                ..
            } => {
                if !has_visible_windows {
                    if let Some(window) = app_handle.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.unminimize();
                        let _ = window.set_focus();
                    }
                }
            }
            _ => {}
        }
    });
}

// ============ 单实例锁 ============

/// 获取锁文件路径
fn lock_file_path() -> Option<PathBuf> {
    let proj_dirs = directories::ProjectDirs::from("com", "marscey", "traejumper")?;
    let data_dir = proj_dirs.data_dir();
    let _ = fs::create_dir_all(data_dir);
    Some(data_dir.join("app.lock"))
}

/// 尝试获取单实例锁
/// 返回 Some(已有实例PID) 表示已有实例在运行，返回 None 表示当前实例获取了锁
fn try_acquire_lock() -> Option<u32> {
    let lock_path = lock_file_path()?;

    // 检查是否存在锁文件
    if lock_path.exists() {
        // 读取已有 PID
        if let Ok(content) = fs::read_to_string(&lock_path) {
            if let Ok(existing_pid) = content.trim().parse::<u32>() {
                // 检查进程是否还活着
                if is_process_alive(existing_pid) {
                    return Some(existing_pid);
                }
                // 进程已不存在，清理陈旧锁
                let _ = fs::remove_file(&lock_path);
            }
        }
        // 读取或解析失败，清理陈旧锁
        let _ = fs::remove_file(&lock_path);
    }

    // 写入当前 PID
    let mut file = match fs::File::create(&lock_path) {
        Ok(f) => f,
        Err(_) => return None,  // 无法创建锁文件，允许继续运行
    };
    let _ = write!(file, "{}", process::id());
    None
}

/// 释放单实例锁
fn release_lock() {
    if let Some(lock_path) = lock_file_path() {
        let _ = fs::remove_file(&lock_path);
    }
}

/// 检查指定 PID 的进程是否存活
fn is_process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        // 通过 kill -0 检测进程是否存在（不实际发送信号）
        let output = Command::new("kill")
            .args(["-0", &pid.to_string()])
            .output();
        match output {
            Ok(o) => o.status.success(),
            Err(_) => false,
        }
    }
    #[cfg(windows)]
    {
        // Windows: 通过 tasklist 检测
        let output = Command::new("tasklist")
            .args(["/FI", &format!("PID eq {}", pid)])
            .output();
        match output {
            Ok(o) => {
                let stdout = String::from_utf8_lossy(&o.stdout);
                stdout.contains(&pid.to_string())
            }
            Err(_) => false,
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = pid;
        false
    }
}

// ============ 系统托盘 ============

/// 创建系统托盘图标与菜单
fn setup_system_tray(app: &tauri::App) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let show = MenuItem::with_id(app, "show", "显示窗口", true, None::<&str>)
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
    let sep = PredefinedMenuItem::separator(app)
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
    let menu = Menu::with_items(app, &[&show, &sep, &quit])
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    // 托盘图标独立使用带圆角的版本（32x32，圆角 7px，透明外框）
    // 与应用图标区分：应用图标方形满幅让 macOS 自动套圆角
    let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray-31.png"))
        .expect("无法加载托盘图标");

    TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        // macOS 最佳实践：左键单击托盘图标直接显示窗口，右键才弹出菜单
        .menu_on_left_click(false)
        .tooltip("Trae Jumper")
        // 左键单击显示窗口
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        // 右键菜单事件
        .on_menu_event(|app, event| {
            match event.id.as_ref() {
                "show" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                "quit" => {
                    app.exit(0);
                }
                _ => {}
            }
        })
        .build(app)?;

    Ok(())
}
