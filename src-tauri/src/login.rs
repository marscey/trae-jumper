use std::sync::Arc;
use tauri::{
    webview::WebviewBuilder, AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, WebviewUrl,
};
use tokio::sync::Mutex;

use crate::account::AccountManager;

/// 启动浏览器登录流程。
///
/// - `update_account_id == None`：新增账号（登录后调用 add_account_by_token）
/// - `update_account_id == Some(id)`：更新指定账号的 Token（登录后调用 update_account_token，
///   login-success 事件仍会附带邮箱，供前端提示）
pub async fn start_login_flow(
    app: AppHandle,
    state: Arc<Mutex<AccountManager>>,
    update_account_id: Option<String>,
) -> Result<(), String> {
    // 如果已有登录子 webview，聚焦它。
    // add / update 共用同一 label（二者互斥，不会同时打开）。
    if let Some(wv) = app.get_webview("trae-login-child") {
        let _ = wv.set_focus();
        return Ok(());
    }

    // 新增和更新场景统一走手动确认流程：
    // 用户点击"确认登录"后，前端调 probe_login_webview 读取 token + cookies。
    // 自动轮询在已登录场景下不可靠（cookie store 可能不共享），手动确认更稳妥。
    start_login_flow_manual(app).await
}

/// 从登录子 webview 直接读取完整 cookies（含 HttpOnly）。
///
/// 直接从子 webview 自身的 cookie store 读取（无痕会话，登录后写入的认证 cookies
/// 均在该 store 内），利用底层 WKHTTPCookieStore.getAllCookies 绕过 JS document.cookie
/// 无法读取 HttpOnly 的限制。这些 cookies 用于后续 Token 自动续签（GetUserToken 接口
/// 需要完整的认证 cookies）。
fn read_child_webview_cookies(app: &AppHandle) -> Option<String> {
    let wv = app.get_webview("trae-login-child")?;
    let cookies = match wv.cookies() {
        Ok(c) => c,
        Err(e) => {
            println!("[DEBUG cookies] 子 webview cookies() 失败: {}", e);
            return None;
        }
    };
    println!("[DEBUG cookies] 子 webview cookies() 成功，共 {} 个 cookie", cookies.len());

    let trae_domains = ["trae.cn", "trae.ai", "bytecdn.com", "volcengine.com", "feishu.cn", "lanxin.com"];
    let injected_cookies = ["trae_jumper_token", "trae_jumper_probe"];
    let cookie_str: String = cookies
        .iter()
        .filter(|c| {
            if injected_cookies.contains(&c.name()) {
                return false;
            }
            c.domain()
                .map(|d| trae_domains.iter().any(|td| d.contains(td)))
                .unwrap_or(false)
        })
        .map(|c| format!("{}={}", c.name(), c.value()))
        .collect::<Vec<_>>()
        .join("; ");

    if cookie_str.is_empty() {
        println!("[DEBUG cookies] 子 webview 没有 trae 相关的 cookies");
        None
    } else {
        println!(
            "[DEBUG cookies] 子 webview 读取到 {} 个 trae 相关 cookies",
            cookie_str.split("; ").count()
        );
        Some(cookie_str)
    }
}

/// 探测登录窗口当前 Token 与账号（手动刷新调用 Rust command 时使用）
#[derive(serde::Serialize)]
pub struct LoginWebviewProbe {
    pub token: String,
    pub user_id: String,
    pub email: Option<String>,
}

/// 更新场景的精简登录流程：在主窗口内创建子 webview + 注入 probe 脚本，不自动检测、不自动关窗。
/// 用户在前端点「刷新」时由 `probe_login_webview_inner` 读取，点「确认」时由
/// `apply_login_webview_token_inner` 写入。
async fn start_login_flow_manual(app: AppHandle) -> Result<(), String> {
    // 精简 init_script：只注册全局 probe 函数，不轮询、不 hook fetch/XHR
    let init_script = r#"
        (function() {
            window.__traeJumperProbe = function() {
                try {
                    var t = localStorage.getItem("Cloud-IDE-Token");
                    if (!t || t.length < 50) {
                        document.cookie = "trae_jumper_probe=; path=/; max-age=0";
                        return;
                    }
                    // JWT 仅含 A-Za-z0-9_- 和 .，均为 cookie 合法字符，无需编码
                    document.cookie = "trae_jumper_probe=" + t + "; path=/; max-age=60";
                } catch(e) {}
            };
        })();
    "#
    .to_string();

    create_login_child_webview(&app, &init_script)
}

/// 在主窗口内创建登录子 webview（label=`trae-login-child`）。
///
/// 方案 B：子 webview 渲染在 React 全屏弹窗中部，底部按钮栏由 React 渲染，视觉一体无需挪窗。
/// 主窗口允许自由调整大小，子 webview 尺寸通过 `resize_login_child_webview` command
/// 由前端 resize 事件驱动同步，避免子 webview 与 React 布局错位。
/// add（自动检测）/ update（手动确认）两个场景共用同一 label，二者互斥不会同时打开。
fn create_login_child_webview(app: &AppHandle, init_script: &str) -> Result<(), String> {
    let login_url = crate::trae_app::current().login_url;

    // 获取主窗口（用 Window 类型，add_child 是 Window 的方法，不是 WebviewWindow）
    let main_window = app
        .get_window("main")
        .ok_or_else(|| "找不到主窗口".to_string())?;

    // 若已有子 webview，先销毁
    if let Some(old) = app.get_webview("trae-login-child") {
        let _ = old.close();
    }

    // 读主窗口当前尺寸（物理像素），转为逻辑像素
    place_login_child_webview(&main_window)?;

    let builder = WebviewBuilder::new(
        "trae-login-child",
        WebviewUrl::External(login_url.parse().map_err(|e| format!("登录地址无效: {}", e))?),
    )
    .initialization_script(init_script)
    // 隔离会话：每次登录窗口都是独立的无痕 webview，
    // 登录新账号不会注销上一个浏览器登录账号的服务端会话
    // （共享会话时切换账号会顶掉旧账号的 cookies+token，导致其"过期"）。
    // 代价：每次添加/更新都需要完整登录，不能复用上次登录态。
    .incognito(true);

    // 子 webview 位置 (0, 56)，高度扣除标题栏与按钮栏
    let scale = main_window.scale_factor().unwrap_or(1.0);
    let inner = main_window.inner_size().unwrap_or(tauri::PhysicalSize {
        width: 1000,
        height: 700,
    });
    let w = inner.width as f64 / scale;
    let h = inner.height as f64 / scale;
    let header_h = 56.0_f64;
    let footer_h = 56.0_f64;

    main_window
        .add_child(
            builder,
            LogicalPosition::new(0.0, header_h),
            LogicalSize::new(w, h - header_h - footer_h),
        )
        .map_err(|e| e.to_string())?;

    // Tauri v2 在 macOS 上通过 add_child 添加子 webview 后，
    // 主窗口会被意外锁定（无法调整大小、无法全屏）。
    // 显式重新设置 resizable 和 maximizable 属性来恢复窗口可调整性。
    let _ = main_window.set_resizable(true);
    let _ = main_window.set_maximizable(true);

    Ok(())
}

/// 根据主窗口当前尺寸定位/重设登录子 webview 的大小。
/// header=56px（React 全屏 modal 标题栏），footer=56px（底部按钮栏）。
pub fn place_login_child_webview(main_window: &tauri::Window) -> Result<(), String> {
    let scale = main_window.scale_factor().unwrap_or(1.0);
    let inner = main_window.inner_size().unwrap_or(tauri::PhysicalSize {
        width: 1000,
        height: 700,
    });
    let w = inner.width as f64 / scale;
    let h = inner.height as f64 / scale;
    let header_h = 56.0_f64;
    let footer_h = 56.0_f64;

    if let Some(wv) = main_window.app_handle().get_webview("trae-login-child") {
        let _ = wv.set_position(LogicalPosition::new(0.0, header_h));
        let _ = wv.set_size(LogicalSize::new(w, h - header_h - footer_h));
    }
    Ok(())
}

/// 关闭登录子 webview（供 `close_login_webview` command 和 apply 成功后复用）
pub fn close_login_webview_internal(app: &AppHandle) {
    // 销毁子 webview
    if let Some(wv) = app.get_webview("trae-login-child") {
        let _ = wv.close();
    }
    // 确保关闭后窗口仍可调整大小和全屏
    if let Some(main_window) = app.get_window("main") {
        let _ = main_window.set_resizable(true);
        let _ = main_window.set_maximizable(true);
    }
}

/// 探测登录窗口当前 Token 与账号。
///
/// 流程：触发 webview 内 `window.__traeJumperProbe()` → JS 读 localStorage token 写入
/// `trae_jumper_probe` cookie → Rust 短暂等待后读 cookie → 用 token 调 entitlement 接口
/// 拿权威 `user_id` + `email`。窗口未打开或未读到 token 返回 `Ok(None)`。
pub async fn probe_login_webview_inner(
    app: &AppHandle,
) -> Result<Option<LoginWebviewProbe>, anyhow::Error> {
    // 获取子 webview
    let wv = match app.get_webview("trae-login-child") {
        Some(w) => w,
        None => return Ok(None),
    };

    // 触发子 webview 内 probe 函数（fire-and-forget，eval 无返回值）
    let _ = wv.eval("window.__traeJumperProbe && window.__traeJumperProbe()");

    // 等待 JS 写入 cookie
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    // 直接从子 webview 读 cookies（不依赖主窗口）
    let cookies = match wv.cookies() {
        Ok(c) => c,
        Err(e) => {
            println!("[DEBUG probe] 子 webview cookies() 失败: {}", e);
            return Ok(None);
        }
    };
    let token = cookies
        .iter()
        .find(|c| c.name() == "trae_jumper_probe")
        .map(|c| c.value().to_string());

    let token = match token {
        Some(v) if v.len() >= 50 => v,
        _ => {
            println!("[DEBUG probe] 未找到 trae_jumper_probe cookie（可能尚未登录）");
            return Ok(None);
        }
    };

    // 用 token 调 entitlement 接口拿权威 user_id + email
    let client = crate::api::TraeApiClient::new_with_token(&token)?;
    let user_info = client.get_user_info_by_token().await?;

    println!(
        "[DEBUG probe] 探测到 token user_id={} email={:?}",
        user_info.user_id, user_info.email
    );

    Ok(Some(LoginWebviewProbe {
        token,
        user_id: user_info.user_id,
        email: user_info.email,
    }))
}

/// 应用登录窗口当前 Token 到指定账号（手动确认调用）。
///
/// 流程：probe 拿 token → 读 webview 完整 cookies（含 HttpOnly）→ 调
/// `update_account_token_with_cookies`（内部会做 user_id 一致性后置校验）→ 成功后关窗。
pub async fn apply_login_webview_token_inner(
    app: &AppHandle,
    state: Arc<Mutex<AccountManager>>,
    account_id: &str,
) -> Result<crate::api::UsageSummary, anyhow::Error> {
    let probe = probe_login_webview_inner(app)
        .await?
        .ok_or_else(|| anyhow::anyhow!("未探测到 Token，请先在登录窗口登录后点刷新"))?;

    // 从子 webview 直接读取完整 cookies（含 HttpOnly）
    let cookies = read_child_webview_cookies(app);

    let mut manager = state.lock().await;
    let summary = manager
        .update_account_token_with_cookies(account_id, probe.token, cookies)
        .await?;

    // 成功后关闭 webview
    close_login_webview_internal(app);

    Ok(summary)
}

/// 应用登录窗口当前 Token 新增账号（手动确认调用，新增场景）。
///
/// 流程：probe 拿 token → 读子 webview 完整 cookies → 调 add_account_by_token → 成功后关窗。
/// 返回新增账号的 email（供前端提示）。
pub async fn apply_login_webview_new_account_inner(
    app: &AppHandle,
    state: Arc<Mutex<AccountManager>>,
) -> Result<String, anyhow::Error> {
    let probe = probe_login_webview_inner(app)
        .await?
        .ok_or_else(|| anyhow::anyhow!("未探测到 Token，请先在登录窗口完成登录后点击确认"))?;

    // 从子 webview 直接读取完整 cookies（含 HttpOnly）
    let cookies = read_child_webview_cookies(app);

    let mut manager = state.lock().await;
    let account = manager
        .add_account_by_token(probe.token, cookies, None, crate::account::AccountLoginSource::Webview)
        .await?;

    // 成功后关闭 webview
    close_login_webview_internal(app);

    Ok(account.email)
}
