//! OAuth Native 登录模块
//!
//! 逆向自 TRAE CN 3.4.2 Electron 客户端 `main.js`，完整复现 OAuth 2.0
//! Authorization Code Flow + PKCE + EC P-256 设备密钥对机制。
//!
//! 两条流程：
//! 1. **AuthCode 登录流程**：用户通过 WebView 打开 OAuth authorize URL → 登录 →
//!    拦截 auth_code → 调 ExchangeToken → 拿到 14d access + 180d refresh
//! 2. **Refresh 续签流程**：access_token 即将过期时 → 用 refresh_token + 私钥签名
//!    调 ExchangeToken → 拿到新的 14d access_token

use base64::Engine;
use chrono::{DateTime, Utc};
use p256::ecdsa::signature::Signer;
use p256::ecdsa::{DerSignature, SigningKey, VerifyingKey};
use p256::pkcs8::{
    DecodePrivateKey, EncodePrivateKey, EncodePublicKey,
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::Duration;

use crate::trae_app::{self, TraeAppVariant};

// ============================================================================
// OAuth 常量
// ============================================================================

/// Client ID：TRAE CN / TraeWork CN / 国际版（on8o9krqynydwx5）
const CLIENT_ID_TRAE: &str = "ono9krqynydwx5";
/// Client ID：SOLO / Lite 模式
const CLIENT_ID_SOLO: &str = "en1oxy7wnw8j9n";

/// ExchangeToken 端点 path（所有变体共用）
const EXCHANGE_TOKEN_PATH: &str = "/trae/api/v3/oauth/ExchangeToken";

// ============================================================================
// 数据结构
// ============================================================================

/// OAuth 凭证集合（Account 中 oauth_* 字段的完整载荷）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OauthCredentials {
    /// OAuth access_token（约 14d 有效）
    pub access_token: String,
    /// OAuth refresh_token（约 180d 有效，绑定 EC 公钥）
    pub refresh_token: String,
    /// access_token 过期时间（RFC3339）
    pub access_expired_at: Option<String>,
    /// refresh_token 过期时间（RFC3339）
    pub refresh_expired_at: Option<String>,
    /// JWT token_release_at
    pub token_release_at: Option<String>,
    /// EC P-256 公钥（SPKI PEM）
    pub public_key_pem: String,
    /// EC P-256 私钥（PKCS#8 PEM，用于 Refresh Token 签名）
    pub private_key_pem: String,
    /// 设备 ID（ExchangeToken 请求体）
    pub device_id: String,
    /// 机器 ID（ExchangeToken 请求体）
    pub machine_id: String,
}

/// ExchangeToken 返回的核心字段（实测：服务端全部 PascalCase）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExchangeTokenResult {
    /// 服务端字段 "Token"（不是 AccessToken）
    #[serde(rename = "Token", default)]
    pub access_token: String,
    /// 服务端字段 "RefreshToken"
    #[serde(rename = "RefreshToken", default)]
    pub refresh_token: String,
    /// access_token 过期时间（ms epoch）——实测服务端字段是 TokenExpireAt
    #[serde(rename = "TokenExpireAt", alias = "ExpireAt", alias = "ExpiredAt", default)]
    pub expired_at_ms: Option<i64>,
    /// refresh_token 过期时间（ms epoch）
    #[serde(rename = "RefreshExpireAt", alias = "RefreshExpiredAt", default)]
    pub refresh_expired_at_ms: Option<i64>,
    #[serde(rename = "TokenExpireDuration", default)]
    pub token_expire_duration_ms: Option<i64>,
    /// 用户信息 JWT（包含用户名/邮箱/头像等完整资料）
    #[serde(rename = "UserJwt", default)]
    pub user_jwt: Option<String>,
    #[serde(rename = "TokenReleaseAt", default)]
    pub token_release_at_ms: Option<i64>,
    #[serde(rename = "UserID", alias = "UserId", default)]
    pub user_id: Option<String>,
    #[serde(rename = "Host", default)]
    pub host: Option<String>,
    #[serde(rename = "UserRegion", default)]
    pub user_region: Option<serde_json::Value>,
    #[serde(rename = "Account", default)]
    pub account: Option<serde_json::Value>,
    #[serde(rename = "BoundDeviceID", default)]
    pub bound_device_id: Option<String>,
    #[serde(rename = "DeviceBindStatus", default)]
    pub device_bind_status: Option<String>,
    /// 完整 Result JSON（用于兜底提取未显式映射的字段）
    #[serde(rename = "raw", default)]
    pub raw: Option<serde_json::Value>,
}

impl ExchangeTokenResult {
    /// access_token 过期时间 RFC3339 格式
    pub fn expired_at(&self) -> Option<String> {
        self.expired_at_ms.and_then(|ms| {
            chrono::DateTime::from_timestamp_millis(ms)
                .map(|dt| dt.format("%Y-%m-%dT%H:%M:%SZ").to_string())
        })
    }
    /// refresh_token 过期时间 RFC3339 格式
    pub fn refresh_expired_at(&self) -> Option<String> {
        self.refresh_expired_at_ms.and_then(|ms| {
            chrono::DateTime::from_timestamp_millis(ms)
                .map(|dt| dt.format("%Y-%m-%dT%H:%M:%SZ").to_string())
        })
    }
}

/// ExchangeToken 统一响应包装
#[derive(Debug, Deserialize)]
struct ExchangeTokenResponse {
    #[serde(rename = "Result")]
    result: Option<ExchangeTokenResult>,
}

// ============================================================================
// 密钥对生成
// ============================================================================

/// 生成 EC P-256 密钥对，返回 PEM 编码的私钥（PKCS#8）和公钥（SPKI）
pub fn generate_key_pair() -> anyhow::Result<(String, String)> {
    let signing_key = SigningKey::random(&mut rand::thread_rng());
    let verifying_key = signing_key.verifying_key();
    let private_pem = signing_key
        .to_pkcs8_pem(p256::pkcs8::LineEnding::LF)
        .map_err(|e| anyhow::anyhow!("私钥 PEM 编码失败: {}", e))?
        .to_string();
    let public_pem = verifying_key
        .to_public_key_pem(p256::pkcs8::LineEnding::LF)
        .map_err(|e| anyhow::anyhow!("公钥 PEM 编码失败: {}", e))?;
    Ok((private_pem, public_pem))
}

// ============================================================================
// PKCE
// ============================================================================

/// 生成 PKCE code_verifier（32 字节随机 → base64url，43 字符）和 code_challenge
pub fn generate_pkce() -> (String, String) {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    let verifier = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);

    let digest = Sha256::digest(verifier.as_bytes());
    let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest);

    (verifier, challenge)
}

// ============================================================================
// DeviceProof 签名（Refresh Token Flow 使用）
// ============================================================================

/// ExchangeToken Refresh Flow 的设备签名
#[derive(Debug, Clone, Serialize)]
struct DeviceProof {
    #[serde(rename = "Signature")]
    signature: String,
    #[serde(rename = "Timestamp")]
    timestamp: i64,
    #[serde(rename = "Nonce")]
    nonce: String,
}

/// 用 EC 私钥对 Refresh Token Exchange 请求签名
///
/// 签名原文（6 段用 \n 连接，与 Electron 源码 NPe() 完全一致）：
///
/// ```
/// POST
/// /trae/api/v3/oauth/ExchangeToken
/// {client_id}
/// {refresh_token}
/// {unix_timestamp}
/// {nonce_hex}
/// ```
fn build_device_proof(
    client_id: &str,
    refresh_token: &str,
    private_key_pem: &str,
) -> anyhow::Result<DeviceProof> {
    let private_key = SigningKey::from_pkcs8_pem(private_key_pem)
        .map_err(|e| anyhow::anyhow!("解析 EC 私钥失败: {}", e))?;

    let timestamp = Utc::now().timestamp();
    let mut nonce_bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = hex::encode(nonce_bytes);

    let sign_str = format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        "POST", EXCHANGE_TOKEN_PATH, client_id, refresh_token, timestamp, nonce
    );

    let signature: DerSignature = private_key
        .try_sign(sign_str.as_bytes())
        .map_err(|e| anyhow::anyhow!("ECDSA 签名失败: {}", e))?;

    // p256 的 DerSignature 实现了 AsRef<[u8]>
    let sig_b64 = base64::engine::general_purpose::STANDARD.encode(signature.as_ref());

    Ok(DeviceProof {
        signature: sig_b64,
        timestamp,
        nonce,
    })
}

// ============================================================================
// ExchangeToken — Auth Code Flow（首次登录）
// ============================================================================

/// ExchangeToken Auth Code Flow 请求体
#[derive(Debug, Serialize)]
struct ExchangeAuthCodeBody {
    #[serde(rename = "ClientID")]
    client_id: String,
    #[serde(rename = "AuthCode")]
    auth_code: String,
    #[serde(rename = "CodeVerifier")]
    code_verifier: String,
    #[serde(rename = "DeviceInfo")]
    device_info: DeviceInfo,
    #[serde(rename = "IDEVersion")]
    ide_version: String,
}

/// ExchangeToken Refresh Flow 请求体
#[derive(Debug, Serialize)]
struct ExchangeRefreshBody {
    #[serde(rename = "ClientID")]
    client_id: String,
    #[serde(rename = "ClientSecret")]
    client_secret: String,
    #[serde(rename = "RefreshToken")]
    refresh_token: String,
    #[serde(rename = "DeviceInfo")]
    device_info: DeviceInfo,
    #[serde(rename = "DeviceProof")]
    device_proof: DeviceProof,
    #[serde(rename = "IDEVersion")]
    ide_version: String,
}

/// ExchangeToken DeviceInfo（逆向自 Electron device info builder）
#[derive(Debug, Serialize, Clone)]
struct DeviceInfo {
    #[serde(rename = "DeviceID")]
    device_id: String,
    #[serde(rename = "MachineID")]
    machine_id: String,
    #[serde(rename = "PlatformCode")]
    platform_code: String,
    #[serde(rename = "DeviceType")]
    device_type: String,
    #[serde(rename = "DeviceName")]
    device_name: String,
    #[serde(rename = "DeviceModel")]
    device_model: String,
    #[serde(rename = "ClientVersion")]
    client_version: String,
    #[serde(rename = "DevicePublicKey")]
    device_public_key: String,
    #[serde(rename = "DeviceBrand")]
    device_brand: String,
    #[serde(rename = "DeviceCPU")]
    device_cpu: String,
    #[serde(rename = "OSInfo")]
    os_info: String,
    #[serde(rename = "OSVersion")]
    os_version: String,
}

impl DeviceInfo {
    /// 构造标准 DeviceInfo（模拟 macOS / PC）
    fn new(device_id: &str, machine_id: &str, public_key_pem: &str, is_cn: bool) -> Self {
        let (os_name, os_version, device_brand, device_model, device_cpu) = detect_system();
        let platform_code = if is_cn { "IDE_PC" } else { "IDE_PC" }; // 国际版也是 IDE_PC
        Self {
            device_id: device_id.to_string(),
            machine_id: machine_id.to_string(),
            platform_code: platform_code.to_string(),
            device_type: "PC".to_string(),
            device_name: hostname(),
            device_model,
            client_version: "3.4.2".to_string(),
            device_public_key: public_key_pem.to_string(),
            device_brand,
            device_cpu,
            os_info: os_name,
            os_version,
        }
    }
}

/// 获取当前机器 hostname（跨平台）
fn hostname() -> String {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        Command::new("hostname")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| "Local".to_string())
    }
    #[cfg(target_os = "windows")]
    {
        std::env::var("COMPUTERNAME").unwrap_or_else(|_| "Local".to_string())
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        "Local".to_string()
    }
}

/// 探测系统信息（简化版，不依赖额外 crate）
fn detect_system() -> (String, String, String, String, String) {
    #[cfg(target_os = "macos")]
    {
        let os_version = Command::new("sw_vers")
            .args(["-productVersion"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| "14.0".to_string());
        ("macOS".to_string(), os_version, "Apple Inc.".to_string(), "Mac".to_string(), "Apple".to_string())
    }
    #[cfg(target_os = "windows")]
    {
        (
            "Windows".to_string(),
            "11".to_string(),
            "Generic".to_string(),
            "PC".to_string(),
            "Intel".to_string(),
        )
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        ("Linux".to_string(), "unknown".to_string(), "Generic".to_string(), "PC".to_string(), "Unknown".to_string())
    }
}

// Need Command import for hostname on mac
#[cfg(target_os = "macos")]
use std::process::Command;

// ============================================================================
// Client ID / Host 选择
// ============================================================================

fn client_id_for(variant: &TraeAppVariant) -> &'static str {
    // TRAE CN / TraeWork CN / 国际版都用 ono9krqynydwx5（Electron $v() fallback）
    match variant.key {
        "trae-cn" | "trae-work" | "trae" => CLIENT_ID_TRAE,
        _ => CLIENT_ID_TRAE,
    }
}

// ============================================================================
// 公开的 ExchangeToken 调用
// ============================================================================

/// 用 AuthCode 换取 Token（首次 OAuth 登录）
pub async fn exchange_auth_code(
    variant: &TraeAppVariant,
    auth_code: &str,
    code_verifier: &str,
    device_id: &str,
    machine_id: &str,
    public_key_pem: &str,
) -> anyhow::Result<ExchangeTokenResult> {
    exchange_auth_code_with_host(
        variant.api_host,
        variant,
        auth_code,
        code_verifier,
        device_id,
        machine_id,
        public_key_pem,
    )
    .await
}

/// 用 AuthCode 换取 Token（指定 host 版本，优先用 OAuth callback 返回的 host）
pub async fn exchange_auth_code_with_host(
    api_host: &str,
    variant: &TraeAppVariant,
    auth_code: &str,
    code_verifier: &str,
    device_id: &str,
    machine_id: &str,
    public_key_pem: &str,
) -> anyhow::Result<ExchangeTokenResult> {
    let client_id = client_id_for(variant);
    let is_cn = variant.is_cn;

    let device_info = DeviceInfo::new(device_id, machine_id, public_key_pem, is_cn);

    let body = ExchangeAuthCodeBody {
        client_id: client_id.to_string(),
        auth_code: auth_code.to_string(),
        code_verifier: code_verifier.to_string(),
        device_info,
        ide_version: "3.4.2".to_string(),
    };

    let url = format!("{}{}", api_host, EXCHANGE_TOKEN_PATH);
    let body_json = serde_json::to_string(&body).unwrap_or_default();
    println!("[OAuth] ExchangeToken(AuthCode) POST → {}", url);
    println!(
        "[OAuth-DEBUG] body_json (len={}): {}",
        body_json.len(),
        &body_json[..body_json.len().min(300)]
    );

    // 加 30s timeout + 详细日志
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| anyhow::anyhow!("reqwest client 构建失败: {}", e))?;

    println!("[OAuth-DEBUG] 发送 HTTP POST...");
    let resp = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("User-Agent", "TraeJumper/0.10.0")
        .header("Accept", "application/json")
        .body(body_json.clone())
        .send()
        .await;

    let resp = match resp {
        Ok(r) => r,
        Err(e) => {
            eprintln!(
                "[OAuth-ERROR] ExchangeToken HTTP 请求错误: {:?}",
                e
            );
            if e.is_timeout() {
                return Err(anyhow::anyhow!(
                    "ExchangeToken 请求超时（30s），请检查网络或 host 是否可达"
                ));
            }
            if e.is_connect() {
                return Err(anyhow::anyhow!(
                    "ExchangeToken 连接失败: {}（检查 host: {}）",
                    e,
                    api_host
                ));
            }
            return Err(anyhow::anyhow!("ExchangeToken HTTP 错误: {}", e));
        }
    };

    let status = resp.status();
    println!(
        "[OAuth-DEBUG] 收到 HTTP {} ({} headers)",
        status,
        resp.headers().len()
    );
    for (k, v) in resp.headers() {
        println!(
            "[OAuth-DEBUG]   header {} = {}",
            k,
            v.to_str().unwrap_or("(binary)")
        );
    }

    let raw = resp.text().await.unwrap_or_default();
    println!(
        "[OAuth-DEBUG] response body (len={}): {}",
        raw.len(),
        &raw[..raw.len().min(500)]
    );

    if !status.is_success() {
        eprintln!(
            "[OAuth-ERROR] ExchangeToken 失败 HTTP {}: {}",
            status,
            &raw[..raw.len().min(500)]
        );
        // 识别已知错误码，返回友好提示
        if raw.contains("Device limit reached") || raw.contains("20401") {
            return Err(anyhow::anyhow!(
                "OAuth 设备绑定数已达上限！请按以下步骤解绑旧设备：\n\
                 1. 在浏览器打开 https://account.trae.cn 登录你的 Trae 账号\n\
                 2. 进入「安全设置」→「设备管理」\n\
                 3. 解绑多余的 OAuth 设备（能看到多次 TraeJumper 注册的设备）\n\
                 4. 解绑完成后重新点击 OAuth 授权登录\n\
                 \n\
                 （TraeJumper 现在复用 Trae 客户端的 device_id，首次绑定成功后不会再新注册设备）"
            ));
        }
        if raw.contains("InvalidCode") || raw.contains("CodeExpired") {
            return Err(anyhow::anyhow!(
                "授权码已失效，请重新点击 OAuth 授权登录按钮（不要重复使用同一个授权码）"
            ));
        }
        return Err(anyhow::anyhow!(
            "OAuth 授权失败 (HTTP {}): {}",
            status,
            &raw[..raw.len().min(200)]
        ));
    }

    let parsed: ExchangeTokenResponse = serde_json::from_str(&raw)
        .map_err(|e| anyhow::anyhow!("ExchangeToken 响应解析失败: {} raw={}", e, &raw[..200]))?;
    let mut result = parsed
        .result
        .ok_or_else(|| anyhow::anyhow!("ExchangeToken 响应缺少 Result: {}", &raw[..200]))?;
    // 再 parse 一份完整 Result Value 存到 raw 字段（兜底提取未显式映射的字段）
    if let Ok(response_val) = serde_json::from_str::<serde_json::Value>(&raw) {
        if let Some(full_result_json) = response_val.get("Result").cloned() {
            result.raw = Some(full_result_json);
        }
    }

    // Debug dump: Result 所有顶层 key + JWT payload 自动解码
    println!(
        "[OAuth] ExchangeToken 成功 user_id={:?} access_len={} refresh_len={} host={:?}",
        result.user_id,
        result.access_token.len(),
        result.refresh_token.len(),
        result.host,
    );
    if let Some(full_raw) = &result.raw {
        if let Some(obj) = full_raw.as_object() {
            println!("[OAuth-DEBUG] Result 顶层 keys: {:?}", obj.keys().collect::<Vec<_>>());
            for (k, v) in obj {
                let vstr = format!("{:?}", v);
                let vstr = if vstr.len() > 100 { vstr[..100].to_string() + "..." } else { vstr };
                println!("  [OAuth-DEBUG] Result.{} = {}", k, vstr);
            }
        }
    }
    // 自动解码 JWT payload 拿用户信息
    if !result.access_token.is_empty() {
        if let Ok(payload) = decode_jwt_payload(&result.access_token) {
            if let Some(obj) = payload.as_object() {
                println!("[OAuth-DEBUG] Token JWT payload keys: {:?}", obj.keys().collect::<Vec<_>>());
            }
            if let Some(data) = payload.get("data").and_then(|d| d.as_object()) {
                println!("[OAuth-DEBUG] Token JWT.data keys: {:?}", data.keys().collect::<Vec<_>>());
            }
        }
    }
    // 解码 UserJwt（用户完整资料）
    if let Some(user_jwt) = &result.user_jwt {
        println!("[OAuth-DEBUG] UserJwt len={}", user_jwt.len());
        match decode_jwt_payload(user_jwt) {
            Ok(up) => {
                if let Some(obj) = up.as_object() {
                    println!("[OAuth-DEBUG] UserJwt payload keys: {:?}", obj.keys().collect::<Vec<_>>());
                    for (k, v) in obj {
                        let vstr = format!("{:?}", v);
                        let vstr = if vstr.len() > 120 { vstr[..120].to_string() + "..." } else { vstr };
                        println!("  [OAuth-DEBUG] UserJwt.{} = {}", k, vstr);
                    }
                }
                if let Some(data) = up.get("data").and_then(|d| d.as_object()) {
                    println!("[OAuth-DEBUG] UserJwt.data keys: {:?}", data.keys().collect::<Vec<_>>());
                    for (k, v) in data {
                        let vstr = format!("{:?}", v);
                        let vstr = if vstr.len() > 120 { vstr[..120].to_string() + "..." } else { vstr };
                        println!("  [OAuth-DEBUG] UserJwt.data.{} = {}", k, vstr);
                    }
                }
            }
            Err(e) => println!("[OAuth-DEBUG] UserJwt 解码失败: {}", e),
        }
    }

    Ok(result)
}

/// 解码 JWT token 的 payload 部分（base64url decode 第二段）
pub fn decode_jwt_payload(token: &str) -> anyhow::Result<serde_json::Value> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() < 2 {
        return Err(anyhow::anyhow!("JWT 格式错误：少于 2 段"));
    }
    let mut payload_b64 = parts[1].to_string();
    // base64url → base64
    payload_b64 = payload_b64.replace('-', "+").replace('_', "/");
    // 补 padding
    let pad = (4 - payload_b64.len() % 4) % 4;
    payload_b64.push_str(&"=".repeat(pad));

    let decoded = base64::engine::general_purpose::STANDARD
        .decode(&payload_b64)
        .map_err(|e| anyhow::anyhow!("base64 解码失败: {}", e))?;
    let json: serde_json::Value = serde_json::from_slice(&decoded)
        .map_err(|e| anyhow::anyhow!("JWT payload JSON 解析失败: {}", e))?;
    Ok(json)
}

/// 用 RefreshToken 续签 access_token（后台定时调用）
pub async fn exchange_refresh_token(
    variant: &TraeAppVariant,
    api_host: &str,
    refresh_token: &str,
    device_id: &str,
    machine_id: &str,
    public_key_pem: &str,
    private_key_pem: &str,
) -> anyhow::Result<ExchangeTokenResult> {
    let client_id = client_id_for(variant);
    let is_cn = variant.is_cn;

    let device_proof = build_device_proof(client_id, refresh_token, private_key_pem)?;
    let device_info = DeviceInfo::new(device_id, machine_id, public_key_pem, is_cn);

    let body = ExchangeRefreshBody {
        client_id: client_id.to_string(),
        client_secret: String::new(), // PKCE 流程不用 secret
        refresh_token: refresh_token.to_string(),
        device_info,
        device_proof,
        ide_version: "3.4.2".to_string(),
    };

    let url = format!("{}{}", api_host, EXCHANGE_TOKEN_PATH);
    println!("[OAuth] ExchangeToken(Refresh) POST → {}", url);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| anyhow::anyhow!("reqwest client 构建失败: {}", e))?;

    println!("[OAuth-DEBUG] 发送 Refresh HTTP POST...");
    let resp = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await;

    let resp = match resp {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[OAuth-ERROR] ExchangeToken(Refresh) HTTP 错误: {:?}", e);
            return Err(anyhow::anyhow!("ExchangeToken(Refresh) HTTP 错误: {}", e));
        }
    };

    let status = resp.status();
    let raw = resp.text().await.unwrap_or_default();

    if !status.is_success() {
        println!("[OAuth] Refresh 失败 HTTP {}: {}", status, &raw[..raw.len().min(500)]);
        // 识别已知错误码
        if raw.contains("TokenInvalid") || raw.contains("RefreshTokenInvalid") {
            return Err(anyhow::anyhow!("Refresh Token 已失效，需要重新登录"));
        }
        return Err(anyhow::anyhow!(
            "ExchangeToken(Refresh) 失败: HTTP {} - {}",
            status,
            &raw[..raw.len().min(200)]
        ));
    }

    let parsed: ExchangeTokenResponse = serde_json::from_str(&raw)?;
    let result = parsed
        .result
        .ok_or_else(|| anyhow::anyhow!("ExchangeToken Refresh 响应缺少 Result"))?;

    println!(
        "[OAuth] Refresh 成功 access_expired_at={:?} refresh_expired_at={:?}",
        result.expired_at(),
        result.refresh_expired_at()
    );

    Ok(result)
}

// ============================================================================
// Authorize URL 构建
// ============================================================================

/// 构建 OAuth 授权 URL（与 Electron buildLoginUrl() 逆向一致）
///
/// 返回 (authorize_url, code_verifier)
pub fn build_authorize_url(
    variant: &TraeAppVariant,
    port: u16,
    device_id: &str,
    machine_id: &str,
) -> (String, String) {
    let client_id = client_id_for(variant);
    let login_host = variant.login_url;
    let (code_verifier, code_challenge) = generate_pkce();
    let login_trace_id = uuid::Uuid::new_v4().to_string();

    let authorize_url = format!(
        "{}/authorization?login_version=1\
         &auth_from=trae\
         &login_channel=native_ide\
         &auth_type=local\
         &client_id={}\
         &redirect=0\
         &login_trace_id={}\
         &auth_callback_url=http://127.0.0.1:{}/authorize\
         &machine_id={}\
         &device_id={}\
         &x_device_id={}\
         &x_machine_id={}\
         &x_app_version=3.4.2\
         &x_app_type=IDE\
         &x_env=\
         &x_device_brand=\
         &x_device_type=\
         &x_os_version=\
         &code_challenge={}\
         &code_challenge_method=S256",
        login_host,
        client_id,
        login_trace_id,
        port,
        machine_id,
        device_id,
        device_id,
        machine_id,
        code_challenge,
    )
    // 清理所有多余的空格（format! 里的缩进导致的）
    .split_whitespace()
    .collect::<String>();

    (authorize_url, code_verifier)
}

// ============================================================================
// 端口探测
// ============================================================================

/// 探测一个空闲端口（60000-65535）
pub fn find_free_port() -> Option<u16> {
    use std::net::TcpListener;
    for _ in 0..100 {
        let port = 60000 + rand::thread_rng().next_u32() % 5535;
        if TcpListener::bind(("127.0.0.1", port as u16)).is_ok() {
            return Some(port as u16);
        }
    }
    None
}
