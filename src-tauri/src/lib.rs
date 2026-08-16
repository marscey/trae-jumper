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
    Manager,
    State,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{TrayIconBuilder, TrayIconEvent, MouseButton, MouseButtonState},
};

use account::{AccountBrief, AccountManager, Account};
use api::{UsageSummary, UsageQueryResponse};

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
    manager.add_account_by_token(token, cookies, None).await.map_err(Into::into)
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
#[tauri::command]
async fn switch_account(account_id: String, state: State<'_, AppState>) -> Result<()> {
    let mut manager = state.account_manager.lock().await;
    manager.switch_account(&account_id).map_err(Into::into)
}

/// 获取账号使用量
#[tauri::command]
async fn get_account_usage(account_id: String, state: State<'_, AppState>) -> Result<UsageSummary> {
    let mut manager = state.account_manager.lock().await;
    manager.get_account_usage(&account_id).await.map_err(Into::into)
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

/// 浏览器登录
#[tauri::command]
async fn start_browser_login(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<()> {
    let manager = state.account_manager.clone();
    login::start_login_flow(app, manager).await.map_err(|e| ApiError { message: e })?;
    Ok(())
}

/// 获取支持的 Trae 应用列表（含安装状态与当前选择）
#[tauri::command]
async fn get_trae_apps() -> Result<Vec<trae_app::TraeAppInfo>> {
    Ok(trae_app::list_app_infos())
}

/// 切换当前管理的目标应用（Trae CN / TRAE SOLO CN / 国际版）
#[tauri::command]
async fn set_current_trae_app(app_key: String) -> Result<()> {
    let variant = trae_app::find_variant(&app_key).map_err(ApiError::from)?;
    trae_app::set_current(variant).map_err(ApiError::from)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
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

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            account_manager: Arc::new(Mutex::new(account_manager)),
        })
        .invoke_handler(tauri::generate_handler![
            add_account_by_token,
            remove_account,
            get_accounts,
            get_account,
            switch_account,
            get_account_usage,
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
            clear_trae_login_state,
            get_trae_path,
            set_trae_path,
            scan_trae_path,
            claim_gift,
            refresh_token,
            refresh_all_tokens,
            start_browser_login,
            get_trae_apps,
            set_current_trae_app,
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
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    // 运行主事件循环
    app.run(|_app_handle, event| {
        if let tauri::RunEvent::ExitRequested { .. } = event {
            // 退出时释放单实例锁
            release_lock();
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

    // 使用应用图标作为托盘图标，优先取自配置，回退内嵌 32x32 PNG
    let icon = app.default_window_icon().cloned().unwrap_or_else(|| {
        tauri::image::Image::from_bytes(include_bytes!("../icons/32x32.png"))
            .expect("无法加载托盘图标")
    });

    TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
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
