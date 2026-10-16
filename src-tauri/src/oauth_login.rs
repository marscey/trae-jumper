//! OAuth Native 登录流程入口
//!
//! 完整链路：
//!   1. 生成 EC P-256 密钥对 + PKCE + 找空闲端口
//!   2. 启动 warp HTTP server 监听 /authorize callback
//!   3. WebView 加载 OAuth authorize URL
//!   4. 用户完成登录 → OAuth server 回调 auth_code
//!   5. ExchangeToken(AuthCode Flow) → 拿到 access/refresh token
//!   6. 持久化到 Account.oauth_* 字段，置 oauth_enabled=true

use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Instant;

use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl};
use uuid::Uuid;
use tokio::sync::{oneshot, Mutex};

use crate::account::{Account, AccountLoginSource};
use crate::trae_app::{self, TraeAppVariant};

/// OAuth callback 收到的完整 payload
#[derive(Clone, Debug)]
struct CallbackPayload {
    auth_code: String,
    host: String,
    /// callback query 里的 userInfo 参数（percent-decode 后的原始 JSON 字符串，含用户名/邮箱等）
    user_info_raw: String,
}

// ============================================================================
// 全局 server handle + cancel sender（用于前端 cancel 时停掉 HTTP server）
// ============================================================================
static SERVER_HANDLE: StdMutex<Option<tokio::task::JoinHandle<()>>> = StdMutex::new(None);
static CANCEL_SENDER: StdMutex<Option<oneshot::Sender<()>>> = StdMutex::new(None);
/// 当前进行中 OAuth 流程的 authorize_url（"切换账号"时用无痕模式重建 webview 复用此 URL + 同一 PKCE）
static CURRENT_AUTHORIZE_URL: StdMutex<Option<String>> = StdMutex::new(None);

pub fn abort_current_oauth_server() {
    if let Ok(mut lock) = SERVER_HANDLE.lock() {
        if let Some(h) = lock.take() {
            h.abort();
            println!("[OAuth-DEBUG] 已 abort warp server");
        }
    }
    if let Ok(mut lock) = CANCEL_SENDER.lock() {
        if let Some(tx) = lock.take() {
            let _ = tx.send(());
            println!("[OAuth-DEBUG] 已发送 cancel 信号");
        }
    }
    if let Ok(mut lock) = CURRENT_AUTHORIZE_URL.lock() {
        *lock = None;
    }
}

/// "切换账号"：销毁当前持久会话 webview，用无痕（全新 cookie 会话）模式重建，
/// 让用户可以登录另一个账号。复用同一 authorize_url / 端口 / code_verifier。
pub fn switch_oauth_account(app: &AppHandle) -> Result<(), String> {
    let url = CURRENT_AUTHORIZE_URL
        .lock()
        .map_err(|e| format!("状态锁失败: {}", e))?
        .clone()
        .ok_or_else(|| "没有进行中的 OAuth 登录流程".to_string())?;
    println!("[OAuth] 用户点击切换账号，以无痕模式重建授权 webview");
    create_oauth_login_webview_inner(app, &url, true)
}

/// 启动 OAuth Native 登录流程（新建账号）
pub async fn start_oauth_login_flow(
    app: AppHandle,
    state: Arc<Mutex<crate::account::AccountManager>>,
) -> Result<Account, String> {
    let t0 = Instant::now();
    let variant = trae_app::current().clone();
    println!(
        "[OAuth-DEBUG] [{}ms] start_oauth_login_flow 开始 variant={}",
        t0.elapsed().as_millis(),
        variant.display_name
    );

    // 关闭已有子 webview
    if let Some(wv) = app.get_webview("trae-login-child") {
        let _ = wv.close();
    }

    // === Step 1: 生成密钥对 + PKCE + 端口 ===
    let t1 = Instant::now();
    let (private_pem, public_pem) = crate::oauth::generate_key_pair()
        .map_err(|e| format!("密钥对生成失败: {}", e))?;
    println!(
        "[OAuth-DEBUG] [{}ms] generate_key_pair（耗时 {}ms）",
        t0.elapsed().as_millis(),
        t1.elapsed().as_millis()
    );

    let t2 = Instant::now();
    let port = crate::oauth::find_free_port()
        .ok_or_else(|| "无法分配空闲端口".to_string())?;
    println!(
        "[OAuth-DEBUG] [{}ms] find_free_port port={}（耗时 {}ms）",
        t0.elapsed().as_millis(),
        port,
        t2.elapsed().as_millis()
    );

    let t3 = Instant::now();
    // device_id 复用 Trae 客户端已有的 device_id
    // 这样同一台机器 = 同一个 OAuth 设备绑定，不会额外占用账号的设备绑定额度
    let device_id = crate::machine::get_trae_device_id()
        .unwrap_or_else(|_| {
            // fallback：从 machine_guid 派生（一台机器固定值）
            let guid = crate::machine::get_machine_guid()
                .unwrap_or_else(|_| Uuid::new_v4().to_string());
            let mut hasher = Sha256::new();
            hasher.update(format!("traejumper-oauth-device:{}", guid).as_bytes());
            let hash = hasher.finalize();
            // 取前 32 hex 字符（UUID simple 格式长度）
            hex::encode(&hash[..16])
        });
    let machine_id = crate::machine::get_machine_guid()
        .unwrap_or_else(|_| uuid::Uuid::new_v4().to_string());
    let (authorize_url, code_verifier) = crate::oauth::build_authorize_url(
        &variant, port, &device_id, &machine_id,
    );
    println!(
        "[OAuth-DEBUG] [{}ms] authorize_url 准备好（耗时 {}ms）",
        t0.elapsed().as_millis(),
        t3.elapsed().as_millis()
    );
    println!(
        "[OAuth] port={} device_id={} url={}...",
        port,
        device_id,
        &authorize_url[..authorize_url.len().min(150)]
    );

    // === Step 2: 启动 warp HTTP server ===
    let t4 = Instant::now();
    let (code_tx, code_rx) = oneshot::channel::<CallbackPayload>();
    let (cancel_tx, cancel_rx) = oneshot::channel::<()>();
    let code_tx = Arc::new(StdMutex::new(Some(code_tx)));
    let server_handle = spawn_callback_server(port, code_tx.clone()).await?;
    *SERVER_HANDLE.lock().unwrap() = Some(server_handle);
    *CANCEL_SENDER.lock().unwrap() = Some(cancel_tx);
    println!(
        "[OAuth-DEBUG] [{}ms] warp server 启动完成（耗时 {}ms）",
        t0.elapsed().as_millis(),
        t4.elapsed().as_millis()
    );

    // === Step 3: 创建 WebView ===
    // 记录 authorize_url，供"切换账号"以无痕模式重建 webview
    if let Ok(mut lock) = CURRENT_AUTHORIZE_URL.lock() {
        *lock = Some(authorize_url.clone());
    }
    let t5 = Instant::now();
    // 默认持久会话（非无痕）：二次打开可复用已登录态，跳过登录页
    create_oauth_login_webview_inner(&app, &authorize_url, false)?;
    println!(
        "[OAuth-DEBUG] [{}ms] create_oauth_login_webview 完成（耗时 {}ms）",
        t0.elapsed().as_millis(),
        t5.elapsed().as_millis()
    );
    // 通知前端进入"等待用户授权"阶段（隐藏 opening loading，露出底部操作按钮）
    let _ = app.emit("oauth-phase", "waiting");

    // === Step 4: 等待 OAuth callback（或 cancel 或 timeout）===
    println!(
        "[OAuth-DEBUG] [{}ms] 开始等待 OAuth callback（最多 600s）",
        t0.elapsed().as_millis()
    );
    let result: std::result::Result<CallbackPayload, String> = tokio::select! {
        r = async {
            match tokio::time::timeout(std::time::Duration::from_secs(600), code_rx).await {
                Ok(Ok(payload)) => Ok(payload),
                Ok(Err(e)) => Err(format!("OAuth callback closed: {}", e)),
                Err(_) => Err("OAuth 登录超时".to_string()),
            }
        } => r,
        _ = cancel_rx => Err("OAuth 登录已取消".to_string()),
    };

    // 收到有效 callback → 通知前端进入"换取凭证"阶段，但先**不关** webview。
    // webview 此时已经渲染了 SUCCESS_HTML（"✅ 登录成功"），ExchangeToken (~1.7s)
    // 跑完再关 webview，这样用户全程只看到 SUCCESS_HTML，不会闪一下 modal 空壳。
    // 失败路径则提前关 webview，让 React 露出错误态。
    if result.is_ok() {
        let _ = app.emit("oauth-phase", "exchanging");
    }

    // 无论成功失败，都先 abort server
    if let Ok(mut lock) = SERVER_HANDLE.lock() {
        if let Some(h) = lock.take() {
            h.abort();
        }
    }
    if let Ok(mut lock) = CANCEL_SENDER.lock() {
        lock.take();
    }
    if let Ok(mut lock) = CURRENT_AUTHORIZE_URL.lock() {
        *lock = None;
    }

    // 失败路径先关 webview，让 React 露出错误态（取消/超时/ExchangeToken 失败都从这里冒泡）
    let callback_payload = match result {
        Ok(p) => p,
        Err(e) => {
            close_login_webview_safe(&app);
            return Err(e);
        }
    };
    let auth_code = callback_payload.auth_code;
    let callback_host = callback_payload.host.clone();
    // userInfo：用户资料（昵称/邮箱/头像）的唯一可靠来源，解析成 JSON
    let callback_user_info: Option<serde_json::Value> =
        serde_json::from_str(&callback_payload.user_info_raw).ok();
    println!(
        "[OAuth-DEBUG] [{}ms] 收到 auth_code (len={}) host={} userInfo_parsed={}",
        t0.elapsed().as_millis(),
        auth_code.len(),
        callback_host,
        callback_user_info.is_some()
    );

    // === Step 5: ExchangeToken（用 callback 返回的 host，更准确）===
    let t6 = Instant::now();
    // 优先用 callback host，fallback 到 variant.api_host
    let exchange_host = if callback_host.is_empty() {
        variant.api_host
    } else {
        callback_host.as_str()
    };
    let token_result = match crate::oauth::exchange_auth_code_with_host(
        exchange_host,
        &variant,
        &auth_code,
        &code_verifier,
        &device_id,
        &machine_id,
        &public_pem,
    )
    .await
    .map_err(|e| format!("ExchangeToken 失败: {}", e))
    {
        Ok(r) => r,
        Err(e) => {
            close_login_webview_safe(&app);
            return Err(e);
        }
    };
    println!(
        "[OAuth-DEBUG] [{}ms] ExchangeToken 完成（耗时 {}ms）",
        t0.elapsed().as_millis(),
        t6.elapsed().as_millis()
    );

    // === Step 6: 创建 Account ===
    let t7 = Instant::now();

    // 从多个来源兜底提取用户信息：
    // 1. UserJwt（实测 Result 里专门有这个 JWT 存完整用户资料）
    // 2. Token JWT payload（data.id / data.tenant_id）
    // 3. ExchangeToken Result raw
    let token_jwt = crate::oauth::decode_jwt_payload(&token_result.access_token).ok();
    let user_jwt = token_result
        .user_jwt
        .as_ref()
        .and_then(|j| crate::oauth::decode_jwt_payload(j).ok());

    /// 从 JSON 对象里按候选 key 名提取字符串（PascalCase / camelCase / snake_case 全试）
    fn pick_str(obj: &serde_json::Value, candidates: &[&str]) -> Option<String> {
        let o = obj.as_object()?;
        for k in candidates {
            if let Some(v) = o.get(*k).and_then(|v| v.as_str()) {
                if !v.is_empty() {
                    return Some(v.to_string());
                }
            }
        }
        None
    }

    /// 在 userInfo 里深层查找：递归遍历整棵 JSON 树（含字符串化的嵌套 JSON），
    /// 在任意层级的 object 里按候选 key 找第一个非空字符串值。
    fn deep_pick(root: Option<&serde_json::Value>, candidates: &[&str]) -> Option<String> {
        use std::collections::HashSet;
        fn walk(node: &serde_json::Value, candidates: &[&str], seen: &mut HashSet<String>) -> Option<String> {
            match node {
                serde_json::Value::Object(map) => {
                    // 1) 当前 object 里直接找候选 key
                    for k in candidates {
                        if let Some(v) = map.get(*k).and_then(|v| v.as_str()) {
                            if !v.is_empty() {
                                return Some(v.to_string());
                            }
                        }
                    }
                    // 2) 递归所有子节点
                    for (_, v) in map {
                        if let Some(found) = walk(v, candidates, seen) {
                            return Some(found);
                        }
                    }
                    None
                }
                serde_json::Value::Array(arr) => {
                    for v in arr {
                        if let Some(found) = walk(v, candidates, seen) {
                            return Some(found);
                        }
                    }
                    None
                }
                serde_json::Value::String(s) => {
                    // 3) 字符串值可能本身是字符串化的 JSON（如 AuditInfo），尝试 parse 后递归
                    if s.starts_with('{') || s.starts_with('[') {
                        if seen.len() < 64 && !seen.contains(s) {
                            seen.insert(s.clone());
                            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(s) {
                                return walk(&parsed, candidates, seen);
                            }
                        }
                    }
                    None
                }
                _ => None,
            }
        }
        let root = root?;
        let mut seen = HashSet::new();
        walk(root, candidates, &mut seen)
    }

    // 用户 ID：Token JWT.data.id 最可靠（已验证 = 4065351123886410），不用递归避免误匹配设备 id
    let user_id = token_jwt
        .as_ref()
        .and_then(|p| p.get("data"))
        .and_then(|d| pick_str(d, &["id", "user_id", "userId"]))
        .or_else(|| deep_pick(callback_user_info.as_ref(), &["UserID", "UserId", "user_id"]))
        .or_else(|| token_result.user_id.clone())
        .unwrap_or_default();

    // tenant_id：Token JWT.data.tenant_id
    let tenant_id = token_jwt
        .as_ref()
        .and_then(|p| p.get("data"))
        .and_then(|d| pick_str(d, &["tenant_id", "tenantId", "TenantID"]))
        .unwrap_or_default();

    // 昵称候选 key（实测 Trae userInfo 用 ScreenName 存昵称 = "🏄🏻冲浪猫"）
    // 宽泛的 Name/name 仅在顶层用；深层递归用具体名避免误匹配设备名
    const NAME_KEYS_TOP: &[&str] = &[
        "ScreenName", "screenName", "screen_name",
        "Name", "UserName", "Username", "username", "name",
        "NickName", "nickName", "nick_name", "Nickname", "nickname",
        "DisplayName", "displayName", "display_name",
    ];
    const NAME_KEYS_DEEP: &[&str] = &[
        "ScreenName", "screenName", "screen_name",
        "NickName", "nickName", "nick_name", "Nickname", "nickname",
        "UserName", "Username", "username",
        "DisplayName", "displayName", "display_name",
        "Name", "name",
    ];
    // email 候选
    const EMAIL_KEYS: &[&str] = &["Email", "email", "Mail", "mail", "EmailAddress", "email_address"];
    // avatar 候选
    const AVATAR_KEYS: &[&str] = &[
        "AvatarUrl", "avatarUrl", "avatar_url", "Avatar", "avatar",
        "AvatarUri", "Portrait", "HeadImg", "headImg", "head_img", "Icon",
    ];

    // 优先级：callback userInfo（实测唯一含昵称的来源）→ UserJwt → Result raw
    let user_jwt_data = user_jwt.as_ref().and_then(|j| j.get("data"));
    let username = callback_user_info
        .as_ref()
        // 顶层先用完整候选
        .and_then(|ui| pick_str(ui, NAME_KEYS_TOP))
        // 顶层没找到再全树递归（用具体名）
        .or_else(|| deep_pick(callback_user_info.as_ref(), NAME_KEYS_DEEP))
        .or_else(|| user_jwt_data.and_then(|d| pick_str(d, NAME_KEYS_TOP)))
        .or_else(|| user_jwt.as_ref().and_then(|j| pick_str(j, NAME_KEYS_TOP)))
        .or_else(|| token_result.raw.as_ref().and_then(|r| pick_str(r, NAME_KEYS_TOP)))
        .unwrap_or_else(|| "Trae OAuth 用户".to_string());

    let email = deep_pick(callback_user_info.as_ref(), EMAIL_KEYS)
        .or_else(|| user_jwt_data.and_then(|d| pick_str(d, EMAIL_KEYS)))
        .or_else(|| user_jwt.as_ref().and_then(|j| pick_str(j, EMAIL_KEYS)))
        .or_else(|| token_result.raw.as_ref().and_then(|r| pick_str(r, EMAIL_KEYS)))
        .unwrap_or_default();

    let avatar_url = deep_pick(callback_user_info.as_ref(), AVATAR_KEYS)
        .or_else(|| user_jwt_data.and_then(|d| pick_str(d, AVATAR_KEYS)))
        .or_else(|| user_jwt.as_ref().and_then(|j| pick_str(j, AVATAR_KEYS)))
        .or_else(|| token_result.raw.as_ref().and_then(|r| pick_str(r, AVATAR_KEYS)))
        .unwrap_or_default();

    println!(
        "[OAuth-DEBUG] 用户信息提取结果: user_id={} tenant_id={} name={} email={} avatar_len={}",
        user_id,
        tenant_id,
        username,
        email,
        avatar_url.len()
    );
    let region = token_result
        .user_region
        .as_ref()
        .and_then(|r| r.get("region"))
        .and_then(|v| v.as_str())
        .unwrap_or("CN")
        .to_string();

    let mut manager = state.lock().await;

    // 账号已存在 → 用新 ExchangeToken 结果更新 OAuth 凭证
    // （新 refresh_token 签发会让旧的立即失效，所以必须更新，否则后台每分钟 401）
    if let Some(existing) = manager
        .store
        .accounts
        .iter_mut()
        .find(|a| a.user_id == user_id)
    {
        let old_refresh = existing
            .oauth_refresh_token
            .as_deref()
            .unwrap_or("")
            .chars()
            .take(16)
            .collect::<String>();
        existing.jwt_token = Some(token_result.access_token.clone());
        existing.token_expired_at = token_result.expired_at();
        existing.oauth_enabled = true;
        existing.oauth_refresh_token = Some(token_result.refresh_token.clone());
        existing.oauth_refresh_expired_at = token_result.refresh_expired_at();
        existing.oauth_public_key_pem = Some(public_pem);
        existing.oauth_private_key_pem = Some(private_pem);
        existing.oauth_device_id = Some(device_id);
        existing.oauth_machine_id = Some(machine_id);
        existing.oauth_host = Some(callback_host.clone());
        existing.region = region.clone();
        existing.avatar_url = avatar_url;
        // OAuth 授权登录入口，无论账号之前是什么来源，统一打标为 WebviewOauth
        existing.login_source = AccountLoginSource::WebviewOauth;
        // 昵称可能在 Trae 侧改过，也刷新一下
        existing.name = username;
        if !email.is_empty() {
            existing.email = email;
        }
        let new_refresh = token_result.refresh_token.chars().take(16).collect::<String>();
        println!(
            "[OAuth-DEBUG] 账号已存在 → 更新 OAuth 凭证 旧_refresh_prefix={} 新_refresh_prefix={}",
            old_refresh, new_refresh
        );
        let account = existing.clone();
        manager
            .save_store()
            .map_err(|e| format!("持久化失败: {}", e))?;
        drop(manager);

        let _ = app.emit("oauth-login-success", &account.email);
        println!(
            "[OAuth-DEBUG] [{}ms] 账号 OAuth 凭证刷新成功！email={} 准备关 webview",
            t0.elapsed().as_millis(),
            account.email
        );

        close_login_webview_safe(&app);
        return Ok(account);
    }

    let mut account = Account::new(
        username.clone(),
        email.clone(),
        String::new(),
        user_id.clone(),
        tenant_id,
    );

    account.jwt_token = Some(token_result.access_token.clone());
    account.token_expired_at = token_result.expired_at();
    account.oauth_enabled = true;
    account.oauth_refresh_token = Some(token_result.refresh_token.clone());
    account.oauth_refresh_expired_at = token_result.refresh_expired_at();
    account.oauth_public_key_pem = Some(public_pem);
    account.oauth_private_key_pem = Some(private_pem);
    account.oauth_device_id = Some(device_id);
    account.oauth_machine_id = Some(machine_id);
    account.oauth_host = Some(callback_host.clone());

    println!(
        "[OAuth-DEBUG] 账号创建成功 id={} name={} email={} host={}",
        account.id, account.name, account.email, account.oauth_host.as_deref().unwrap_or("")
    );
    account.avatar_url = avatar_url;
    account.region = region.clone();
    account.login_source = AccountLoginSource::WebviewOauth;

    manager.store.accounts.push(account.clone());
    match manager.save_store() {
        Ok(_) => {}
        Err(e) => {
            println!("[OAuth-DEBUG] save_store 失败 → 关 webview → Err: {}", e);
            drop(manager);
            close_login_webview_safe(&app);
            return Err(format!("持久化失败: {}", e));
        }
    }

    let _ = app.emit("oauth-login-success", &account.email);
    println!(
        "[OAuth-DEBUG] [{}ms] 账号创建成功！email={} 准备关 webview",
        t0.elapsed().as_millis(),
        account.email
    );

    // 立即尝试关 webview。wv.close() 在 Tauri 2 子窗口上理论上是同步的，
    // 但 warp handler 刚返回 SUCCESS_HTML，可能仍有内部状态未清完；
    // 同时 spawn 一个 50ms 后的二次关闭，双保险。
    close_login_webview_safe(&app);
    {
        let app_clone = app.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(80)).await;
            close_login_webview_safe(&app_clone);
        });
    }

    Ok(account)
}

// ============================================================================
// 辅助函数
// ============================================================================

const SUCCESS_HTML: &str = r#"
<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>授权成功</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
html,body{width:100%;height:100%;background:#f5f6f8;
  font-family:-apple-system,BlinkMacSystemFont,"PingFang SC","Microsoft YaHei",sans-serif;
  display:flex;align-items:center;justify-content:center;color:#1f2329}
.card{text-align:center}
.ring{width:40px;height:40px;margin:0 auto 18px;border:3px solid #e3e8f0;
  border-top-color:#1b8fff;border-radius:50%;animation:tjs 0.8s linear infinite}
.title{font-size:15px;font-weight:600;margin-bottom:6px}
.sub{font-size:12.5px;color:#8a8f9c}
@keyframes tjs{to{transform:rotate(360deg)}}
</style></head><body>
<div class="card">
  <div class="ring"></div>
  <div class="title">授权成功</div>
  <div class="sub">正在获取访问凭证，请勿关闭…</div>
</div>
</body></html>"#;

const BASE_CB_HTML: &str = r#"
<html><body style="text-align:center;padding:60px;font-family:-apple-system;background:#fff5f5">
<h2 style="color:#e5484d">❌ 登录回调异常</h2>
<p>Trae OAuth 服务端没有返回预期的授权码。</p>
<p style="font-size:12px;color:#666;margin-top:20px">请求路径: <code>__PATH__</code></p>
<p style="font-size:12px;color:#666">收到参数: __COUNT__ 个</p>
<pre style="text-align:left;background:#222;color:#eee;padding:12px;border-radius:6px;font-size:11px;overflow:auto">__PARAMS__</pre>
<p style="margin-top:15px"><a href="javascript:history.back()">返回</a></p>
</body></html>"#;

/// 启动 warp HTTP server 等待 OAuth callback
async fn spawn_callback_server(
    port: u16,
    code_tx: Arc<StdMutex<Option<oneshot::Sender<CallbackPayload>>>>,
) -> Result<tokio::task::JoinHandle<()>, String> {
    use warp::Filter;

    let authorize_route = warp::path!("authorize")
        .and(warp::get())
        .and(warp::query::<HashMap<String, String>>())
        .and_then(
            move |params: HashMap<String, String>| {
                let tx = code_tx.clone();
                async move {
                    let all_keys: Vec<String> = params.keys().cloned().collect();
                    let code_val = params.get("code").cloned();
                    let error_val = params.get("error").cloned();
                    let auth_code_info_raw = params.get("authCodeInfo").cloned();
                    let host = params.get("host").cloned().unwrap_or_default();

                    println!(
                        "[OAuth-DEBUG] warp /authorize keys=[{}] count={} host={}",
                        all_keys.join(","),
                        params.len(),
                        host
                    );
                    for (k, v) in &params {
                        let trimmed: String = v.chars().take(120).collect();
                        println!("[OAuth-DEBUG]   param {} = '{}'", k, trimmed);
                    }

                    // 1) 标准 OAuth code（未来兼容）
                    let mut code = code_val.clone();

                    // 1b) Trae 自定义 authCodeInfo 解析
                    if code.is_none() {
                        if let Some(raw) = auth_code_info_raw {
                            let decoded = percent_decode(&raw);
                            println!(
                                "[OAuth-DEBUG] 尝试解析 authCodeInfo (len={})",
                                decoded.len()
                            );
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&decoded) {
                                if let Some(ac) = json.get("AuthCode").and_then(|v| v.as_str()) {
                                    println!(
                                        "[OAuth-DEBUG] 从 authCodeInfo 提取 AuthCode 成功 (len={})",
                                        ac.len()
                                    );
                                    code = Some(ac.to_string());
                                } else {
                                    println!(
                                        "[OAuth-DEBUG] authCodeInfo JSON 无 AuthCode, keys={:?}",
                                        json.as_object().map(|o| o.keys().collect::<Vec<_>>())
                                    );
                                }
                            } else {
                                println!(
                                    "[OAuth-DEBUG] authCodeInfo 不是有效 JSON: {}...",
                                    &decoded.chars().take(100).collect::<String>()
                                );
                            }
                        }
                    }

                    // 2) 拿到 auth_code + host + userInfo → 发送
                    if let Some(code) = code {
                        // userInfo：warp::query 已自动 percent-decode，直接用；
                        // 嵌套 JSON 里含转义引号，双重 decode 会破坏，故仅在 parse 失败时兜底再 decode
                        let user_info_raw = params.get("userInfo").cloned().unwrap_or_default();
                        let user_info_parsed = serde_json::from_str::<serde_json::Value>(&user_info_raw).is_ok();
                        let user_info_final = if user_info_parsed || user_info_raw.is_empty() {
                            user_info_raw.clone()
                        } else {
                            // 兜底：极少数情况 warp 未完全 decode，手动再 decode 一次
                            let decoded = percent_decode(&user_info_raw);
                            if serde_json::from_str::<serde_json::Value>(&decoded).is_ok() {
                                decoded
                            } else {
                                user_info_raw
                            }
                        };
                        // 完整打印 userInfo（这是用户资料的唯一来源，不能截断）
                        println!("[OAuth-DEBUG] userInfo 完整内容 (len={}): {}", user_info_final.len(), user_info_final);
                        if let Ok(ui) = serde_json::from_str::<serde_json::Value>(&user_info_final) {
                            if let Some(obj) = ui.as_object() {
                                println!("[OAuth-DEBUG] userInfo keys: {:?}", obj.keys().collect::<Vec<_>>());
                            }
                        }

                        let mut slot = tx.lock().unwrap();
                        if let Some(sender) = slot.take() {
                            let payload = CallbackPayload {
                                auth_code: code.clone(),
                                host: host.clone(),
                                user_info_raw: user_info_final.clone(),
                            };
                            let _ = sender.send(payload);
                            println!(
                                "[OAuth-DEBUG] warp: 已 send CallbackPayload (code_len={}, host={}, userInfo_len={})",
                                code.len(), host, user_info_final.len()
                            );
                        }
                        return Ok::<_, warp::Rejection>(warp::reply::html(SUCCESS_HTML.to_string()));
                    }

                    // 2) OAuth error 返回
                    if let Some(err) = error_val {
                        let desc = params.get("error_description").cloned().unwrap_or_else(|| "未知错误".to_string());
                        println!(
                            "[OAuth-DEBUG] warp: OAuth error={} desc={}",
                            err, desc
                        );
                        let body = format!(
                            "<html><body style='text-align:center;padding:60px;font-family:-apple-system;background:#fff5f5'>\
                             <h2 style='color:#e5484d'>❌ 登录未完成</h2>\
                             <p style='font-size:14px'>原因: <b>{}</b></p>\
                             <p style='font-size:12px;color:#888'>error={}</p>\
                             <p style='margin-top:20px'><a href='javascript:history.back()'>返回重新登录</a></p>\
                             <script>setTimeout(function(){{try{{window.close()}}catch(e){{}}}},3000)</script></body></html>",
                            desc, err
                        );
                        return Ok::<_, warp::Rejection>(warp::reply::html(body));
                    }

                    // 3) 没有 code 也没有 error —— 诊断页面
                    let params_dump: Vec<String> = params
                        .iter()
                        .map(|(k, v)| format!("{}={}", k, v.chars().take(80).collect::<String>()))
                        .collect();
                    let mut body = BASE_CB_HTML.to_string();
                    body = body.replace("__PATH__", "/authorize");
                    body = body.replace("__COUNT__", &params.len().to_string());
                    body = body.replace("__PARAMS__", &params_dump.join("\n"));
                    println!("[OAuth-DEBUG] warp: 既无 code 也无 error — 返回诊断页面");
                    return Ok::<_, warp::Rejection>(warp::reply::html(body));
                }
            },
        );

    // catch-all 路由：任何请求都打日志
    let catch_all = warp::any()
        .and(warp::method())
        .and(warp::path::full())
        .and_then(
            |method: warp::http::Method, full_path: warp::path::FullPath| {
                async move {
                    println!(
                        "[OAuth-DEBUG] warp [catch-all] {} path='{}'",
                        method,
                        full_path.as_str()
                    );
                    Ok::<_, warp::Rejection>(warp::reply::html(
                        "<html><body><p>OAuth callback server (trae-jumper)</p></body></html>",
                    ))
                }
            },
        );

    let final_route = authorize_route.or(catch_all);

    let handle = tokio::spawn(async move {
        println!(
            "[OAuth-DEBUG] warp server 开始监听 127.0.0.1:{}",
            port
        );
        warp::serve(final_route)
            .run(([127, 0, 0, 1], port))
            .await;
        println!("[OAuth-DEBUG] warp server 已退出");
    });

    // 端口探测确认 server 就绪
    for i in 0..20 {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        if let Ok(stream) = tokio::net::TcpStream::connect(("127.0.0.1", port)).await {
            drop(stream);
            println!(
                "[OAuth-DEBUG] warp server 已就绪（第 {} 次探测）",
                i + 1
            );
            return Ok(handle);
        }
    }
    println!("[OAuth-DEBUG] warp server 启动探测超时，继续前进");

    Ok(handle)
}

/// 构造注入到每个页面的初始化脚本：
/// 1. 给本地 oauth-loading.html 传 authorize_url（window.__OAUTH_TARGET__）
/// 2. 在 trae 授权页 document-start 注入顶部加载进度条，页面 load 后移除
fn build_oauth_init_script(authorize_url: &str) -> String {
    // 用 serde_json 序列化为合法 JS 字符串字面量（含引号转义），避免注入/转义问题
    let target_lit = serde_json::to_string(authorize_url).unwrap_or_else(|_| "\"\"".to_string());

    const JS_BODY: &str = r#"
(function () {
    window.__OAUTH_TARGET__ = __TARGET__;
    try {
        var host = location.hostname || "";
        // 仅在 trae 授权相关域名显示顶部进度条（本地 loading 页与 127.0.0.1 callback 不需要）
        if (host.indexOf("trae") === -1) return;

        var bar = document.createElement("div");
        bar.id = "__tj_oauth_bar";
        bar.style.cssText = [
            "position:fixed", "top:0", "left:0", "height:3px", "width:30%",
            "z-index:2147483647", "background:linear-gradient(90deg,#1b8fff,#56b0ff)",
            "border-radius:0 2px 2px 0", "opacity:0.9", "pointer-events:none",
            "box-shadow:0 0 8px rgba(27,143,255,.6)",
            "transition:width .25s ease,opacity .3s ease",
            "animation:__tj_slide 1.1s ease-in-out infinite"
        ].join(";");
        var style = document.createElement("style");
        style.textContent = "@keyframes __tj_slide{0%{left:-30%}50%{left:35%;width:45%}100%{left:100%;width:30%}}";

        function mount() {
            var root = document.documentElement || document.body;
            if (!root) return false;
            if (!document.getElementById("__tj_oauth_bar")) {
                (document.head || root).appendChild(style);
                root.appendChild(bar);
            }
            return true;
        }
        function remove() {
            var b = document.getElementById("__tj_oauth_bar");
            if (b) b.remove();
        }
        if (!mount()) {
            document.addEventListener("DOMContentLoaded", mount, { once: true });
        }
        window.addEventListener("load", function () { setTimeout(remove, 300); });
        // 15s 安全兜底：异常情况下进度条也必须消失
        setTimeout(remove, 15000);
    } catch (e) { /* 注入失败不影响授权主流程 */ }
})();
"#;
    JS_BODY.replace("__TARGET__", &target_lit)
}

/// 创建 OAuth 登录子 webview
///
/// 先加载本地 `oauth-loading.html`（自带 spinner，由 init script 注入目标地址后
/// 自动 replace 到授权 URL）。WKWebView 在远程页面首字节到达前会保持渲染本地页，
/// 从而覆盖 trae.cn 连接/TTFB 期间（实测可达十几秒）的白屏。
///
/// `incognito=false`（默认）：持久 cookie 会话，二次打开可跳过登录页；
/// `incognito=true`（切换账号）：全新临时会话，可登录另一个账号。
fn create_oauth_login_webview_inner(
    app: &AppHandle,
    authorize_url: &str,
    incognito: bool,
) -> Result<(), String> {
    let t0 = Instant::now();
    let main_window = app
        .get_window("main")
        .ok_or_else(|| "找不到主窗口".to_string())?;

    if let Some(old) = app.get_webview("trae-login-child") {
        let _ = old.close();
    }

    let scale = main_window.scale_factor().unwrap_or(1.0);
    let inner = main_window.inner_size().unwrap_or(tauri::PhysicalSize {
        width: 1000,
        height: 700,
    });
    let w = inner.width as f64 / scale;
    let h = inner.height as f64 / scale;
    println!(
        "[OAuth-DEBUG] add_child 位置=(0,56) 尺寸={}x{} incognito={}",
        w, h, incognito
    );

    // 初始页：Vite public/ 下的本地 loading 页（dev 走 vite，prod 走 tauri asset 协议）
    let mut builder = tauri::webview::WebviewBuilder::new(
        "trae-login-child",
        WebviewUrl::App("oauth-loading.html".into()),
    )
    .initialization_script(&build_oauth_init_script(authorize_url));

    if incognito {
        // 切换账号：隔离的临时会话，不复用已登录 cookie
        builder = builder.incognito(true);
    }

    main_window
        .add_child(
            builder,
            tauri::LogicalPosition::new(0.0, 56.0),
            tauri::LogicalSize::new(w, h - 112.0),
        )
        .map_err(|e| e.to_string())?;

    let _ = main_window.set_resizable(true);
    let _ = main_window.set_maximizable(true);

    println!(
        "[OAuth-DEBUG] create_oauth_login_webview 完成 incognito={}（耗时 {}ms）",
        incognito,
        t0.elapsed().as_millis()
    );
    Ok(())
}

/// 关闭登录子 webview（不 abort server——server 由 cancel_rx/abort_current_oauth_server 负责）
pub fn close_login_webview_safe(app: &AppHandle) {
    if let Some(wv) = app.get_webview("trae-login-child") {
        println!("[OAuth-DEBUG] close_login_webview_safe → wv.close()");
        let _ = wv.close();
    } else {
        println!("[OAuth-DEBUG] close_login_webview_safe → webview already gone");
    }
    if let Some(main_window) = app.get_window("main") {
        let _ = main_window.set_resizable(true);
        let _ = main_window.set_maximizable(true);
    }
}

/// 简易 URL percent-decode（处理 %XX 编码的 JSON 字符串）
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h1), Some(h2)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                out.push((h1 << 4) | h2);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}
