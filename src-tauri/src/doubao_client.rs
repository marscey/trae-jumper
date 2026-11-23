//! 豆包桌面客户端本地登录态读取（Chromium Cookies v10 解密）。
//!
//! 背景（2026-11-23 实测确认，见 docs/doubao-api/API-INVENTORY.md §2.8）：
//! - 签到接口 `/alice/ug/campaign/landing/checkin/`（`scene=pc_incentive_task_v3`）
//!   只对豆包**桌面客户端**登录态开放；
//! - 用网页端 Cookie 会恒定返回 `code=710010702 系统错误`
//!   （aid=582478/497858、Origin、SamanthaDoubao UA、device_id、agw-js-conv、
//!    msToken 等十余种组合均已实测排除，与参数无关）；
//! - 客户端 Cookie 比网页端多 `msToken` / `tt_scid` 等字段，且会话属于客户端登录账号。
//!
//! 因此本模块直接读取客户端 Chromium Cookies 数据库（v10 加密），解密出
//! `.doubao.com` 域 Cookie 串，供客户端专属接口使用。

use anyhow::{anyhow, Result};

/// 客户端登录态：Cookie 串 + 所属 user_id。
pub struct ClientLogin {
    pub cookies: String,
    pub user_id: String,
}

/// 读取豆包客户端当前登录账号的 Cookie。
#[cfg(target_os = "macos")]
pub fn read_client_login() -> Result<ClientLogin> {
    let base = directories::BaseDirs::new()
        .ok_or_else(|| anyhow!("无法定位用户主目录"))?
        .home_dir()
        .join("Library/Application Support/Doubao");
    if !base.exists() {
        return Err(anyhow!("未找到豆包客户端数据目录：{}", base.display()));
    }

    let dbs = collect_cookie_dbs(&base);

    // 1) 先用缓存的 Safe Storage 密钥（避免每次都弹系统钥匙串授权框）
    if let Some(key) = read_cached_key() {
        #[cfg(debug_assertions)]
        println!("[DEBUG][doubao-client] 使用缓存密钥（{} bytes）尝试 {} 个 DB", key.len(), dbs.len());
        match try_dbs(&dbs, &key) {
            Ok(login) => {
                println!("[INFO][doubao-client] 缓存密钥成功: user_id={}", login.user_id);
                return Ok(login);
            }
            Err(e) => {
                println!("[WARN][doubao-client] 缓存密钥失效: {} → 丢弃重读钥匙串", e);
                remove_cached_key();
            }
        }
    } else {
        #[cfg(debug_assertions)]
        println!("[DEBUG][doubao-client] 无缓存密钥，直接读钥匙串");
    }

    // 2) 读钥匙串（首次会弹一次系统授权，建议点始终允许）
    let key = keychain_key().map_err(|e| {
        #[cfg(debug_assertions)]
        println!("[DEBUG][doubao-client] keychain_key 失败: {}", e);
        e
    })?;
    #[cfg(debug_assertions)]
    println!("[DEBUG][doubao-client] 钥匙串密钥读成功，尝试 {} 个 DB", dbs.len());
    let login = try_dbs(&dbs, &key).map_err(|e| {
        #[cfg(debug_assertions)]
        println!("[DEBUG][doubao-client] try_dbs 全部失败: {}", e);
        e
    })?;
    write_cached_key(&key);
    println!("[INFO][doubao-client] 钥匙串密钥成功: user_id={}", login.user_id);
    Ok(login)
}

/// 收集候选 Cookies 数据库：优先客户端 `Local State` 中记录的活跃 Profile。
///
/// Chromium/Doubao 的 `Local State`（`~/.../Doubao/Local State`）里有
/// `profile.last_used` 和 `profile.info_cache.<name>.active_time`，
/// 决定当前哪个 Profile 窗口是用户正在用的。
/// 之前硬编码 Default 优先，当 Default 是老账号时会误读到旧登录态。
#[cfg(target_os = "macos")]
fn collect_cookie_dbs(base: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();

    // 1) 一次性读 Local State（避免 fallback 循环里每个 Profile 重复读）
    let ls = read_local_state(base);

    // 2) 优先：Local State 里 last_used profile（活跃窗口）
    if let Some((profile_key, active_time)) = ls.as_ref().and_then(|ls| {
        ls.get("profile")
            .and_then(|p| p.get("last_used"))
            .and_then(|lu| lu.as_str())
            .and_then(|lu| {
                let at = ls
                    .get("profile")
                    .and_then(|p| p.get("info_cache"))
                    .and_then(|c| c.get(lu))
                    .and_then(|info| info.get("active_time"))
                    .and_then(|t| t.as_f64())
                    .unwrap_or(0.0);
                Some((lu.to_string(), at))
            })
    }) {
        let dir_name = if profile_key == "Default" {
            "Default".to_string()
        } else if profile_key.starts_with("Profile ") {
            profile_key.clone()
        } else {
            String::new()
        };
        if !dir_name.is_empty() {
            let p = base.join(&dir_name).join("Cookies");
            if p.exists() {
                candidates.push(p);
            }
        }
        println!(
            "[INFO][doubao-client] Local State 活跃 profile='{}' active_time={}",
            profile_key, active_time
        );
    }

    // 3) 兜底：所有 Profile 按 active_time 降序（只用刚才缓存的 ls，不再重读）
    let active_times = profile_active_times_from_ls(ls.as_ref());
    let mut fallback: Vec<(f64, std::path::PathBuf)> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(base) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() { continue; }
            let name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n, None => continue,
            };
            if name == "Default" || name.starts_with("Profile ") {
                let ck = path.join("Cookies");
                if !ck.exists() { continue; }
                let at = active_times.get(name).copied().unwrap_or(0.0);
                fallback.push((at, ck));
            }
        }
    }
    fallback.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    // 去重（Local State 找到的活跃 profile 可能也在 fallback 里）
    for (_at, p) in fallback {
        if !candidates.contains(&p) {
            candidates.push(p);
        }
    }

    println!(
        "[INFO][doubao-client] Cookies DB 候选顺序: {:?}",
        candidates.iter().map(|p| p.file_name().and_then(|n| n.to_str()).unwrap_or("")).collect::<Vec<_>>()
    );
    candidates
}

/// 读取并解析 Doubao Local State（Chromium/Doubao 全局 profile 元数据）。
/// 返回 None 表示文件不存在 / 权限不足 / JSON 损坏（完全跳过活跃 profile 检测）。
#[cfg(target_os = "macos")]
fn read_local_state(base: &std::path::Path) -> Option<serde_json::Value> {
    let ls_path = base.join("Local State");
    let text = std::fs::read_to_string(&ls_path).ok()?;
    serde_json::from_str(&text).ok()
}

/// 从已解析的 Local State 中提取所有 profile 的 active_time 映射。
#[cfg(target_os = "macos")]
fn profile_active_times_from_ls(ls: Option<&serde_json::Value>) -> std::collections::HashMap<String, f64> {
    let mut map = std::collections::HashMap::new();
    if let Some(info_cache) = ls
        .and_then(|v| v.get("profile"))
        .and_then(|p| p.get("info_cache"))
        .and_then(|c| c.as_object())
    {
        for (name, info) in info_cache.iter() {
            if let Some(at) = info
                .get("active_time")
                .and_then(|t| t.as_f64())
            {
                map.insert(name.clone(), at);
            }
        }
    }
    map
}

#[cfg(target_os = "macos")]
fn try_dbs(dbs: &[std::path::PathBuf], key: &[u8]) -> Result<ClientLogin> {
    let mut errors = Vec::new();
    for db in dbs.iter().filter(|p| p.exists()) {
        match read_from_db(db, key) {
            Ok(login) => return Ok(login),
            Err(e) => errors.push(format!("{}: {e}", db.display())),
        }
    }
    Err(anyhow!(
        "未从豆包客户端读取到有效登录态（客户端可能未登录或正在写入）。详情：{}",
        errors.join("; ")
    ))
}

/// Safe Storage 派生密钥缓存路径（0600）。避免每次签到都触发钥匙串授权弹窗。
#[cfg(target_os = "macos")]
fn key_cache_path() -> Option<std::path::PathBuf> {
    directories::ProjectDirs::from("com", "marscey", "traejumper")
        .map(|d| d.data_dir().join("doubao_safe_storage.key"))
}

#[cfg(target_os = "macos")]
fn read_cached_key() -> Option<Vec<u8>> {
    let p = key_cache_path()?;
    let bytes = std::fs::read(p).ok()?;
    if bytes.len() == 16 {
        Some(bytes)
    } else {
        None
    }
}

#[cfg(target_os = "macos")]
fn write_cached_key(key: &[u8]) {
    let Some(p) = key_cache_path() else { return };
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if std::fs::write(&p, key).is_ok() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600));
        }
    }
}

#[cfg(target_os = "macos")]
fn remove_cached_key() {
    if let Some(p) = key_cache_path() {
        let _ = std::fs::remove_file(p);
    }
}

#[cfg(not(target_os = "macos"))]
pub fn read_client_login() -> Result<ClientLogin> {
    Err(anyhow!("读取豆包客户端登录态暂仅支持 macOS"))
}

/// 读取单个 Cookies 数据库文件并解密。
#[cfg(target_os = "macos")]
fn read_from_db(db: &std::path::Path, key: &[u8]) -> Result<ClientLogin> {
    // 复制一份避免被客户端锁住
    let tmp = std::env::temp_dir().join(format!("doubao_ck_{}.db", uuid::Uuid::new_v4()));
    std::fs::copy(db, &tmp)?;

    let output = std::process::Command::new("sqlite3")
        .arg("-separator")
        .arg("\u{1f}")
        .arg(&tmp)
        .arg(
            "SELECT host_key,name,hex(encrypted_value) FROM cookies \
             WHERE host_key LIKE '%doubao.com' ORDER BY host_key,name;",
        )
        .output();
    let _ = std::fs::remove_file(&tmp);

    let output = output.map_err(|e| anyhow!("调用 sqlite3 失败: {e}"))?;
    if !output.status.success() {
        return Err(anyhow!(
            "sqlite3 读取失败: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut pairs: Vec<(String, String)> = Vec::new();
    for line in text.lines() {
        let mut it = line.splitn(3, '\u{1f}');
        let (host, name, hexv) = match (it.next(), it.next(), it.next()) {
            (Some(h), Some(n), Some(v)) => (h, n, v.trim()),
            _ => continue,
        };
        let enc = match hex::decode(hexv) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if let Some(val) = decrypt_value(key, host, &enc) {
            // Chrome v10 Cookie 解密后可能残留控制字符（\x00 / \r / \n），
            // 会导致 reqwest HeaderValue 解析失败。清洗掉所有非可见 ASCII。
            let val: String = val
                .chars()
                .filter(|c| {
                    let c = *c as u32;
                    (0x20..=0x7e).contains(&c) || c == b'\t' as u32
                })
                .collect();
            if !val.is_empty() {
                pairs.push((name.to_string(), val));
            }
        }
    }

    if pairs.is_empty() {
        return Err(anyhow!("无 doubao.com 域 Cookie"));
    }

    let get = |n: &str| pairs.iter().find(|(k, _)| k == n).map(|(_, v)| v.clone());
    let sessionid = get("sessionid").ok_or_else(|| anyhow!("缺少 sessionid（客户端未登录）"))?;
    let multi_sids = get("multi_sids");
    let user_id = resolve_user_id(&multi_sids, &sessionid).ok_or_else(|| {
        anyhow!(
            "无法从 multi_sids 解析 user_id（sessionid={}）",
            sessionid
        )
    })?;

    // 同名 Cookie 保留首个
    let mut seen = std::collections::HashSet::new();
    let mut parts = Vec::new();
    for (k, v) in &pairs {
        if seen.insert(k.clone()) {
            parts.push(format!("{k}={v}"));
        }
    }

    Ok(ClientLogin {
        cookies: parts.join("; "),
        user_id,
    })
}

/// 从钥匙串读取豆包客户端 Safe Storage 密钥，PBKDF2-HMAC-SHA1 派生 AES-128 密钥。
#[cfg(target_os = "macos")]
fn keychain_key() -> Result<Vec<u8>> {
    let output = std::process::Command::new("security")
        .args(["find-generic-password", "-s", "Doubao Safe Storage", "-w"])
        .output()
        .map_err(|e| anyhow!("调用 security 失败: {e}"))?;
    if !output.status.success() {
        return Err(anyhow!(
            "未读取到钥匙串项「Doubao Safe Storage」，请确认豆包客户端已安装并登录"
        ));
    }
    let pw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if pw.is_empty() {
        return Err(anyhow!("钥匙串密钥为空"));
    }
    let mut key = [0u8; 16];
    pbkdf2::pbkdf2_hmac::<sha1::Sha1>(pw.as_bytes(), b"saltysalt", 1003, &mut key);
    Ok(key.to_vec())
}

/// 解密 Chromium v10/v11 Cookie 值。
#[cfg(target_os = "macos")]
fn decrypt_value(key: &[u8], host: &str, enc: &[u8]) -> Option<String> {
    use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};

    if enc.len() <= 3 {
        return None;
    }
    let (prefix, ct) = enc.split_at(3);
    if prefix != b"v10" && prefix != b"v11" {
        return None;
    }
    if ct.is_empty() || ct.len() % 16 != 0 {
        return None;
    }

    let iv = [b' '; 16];
    let dec = cbc::Decryptor::<aes::Aes128>::new(key.into(), (&iv).into());
    let pt = dec.decrypt_padded_vec_mut::<Pkcs7>(ct).ok()?;

    // Chrome v24+：明文前 32 字节是 SHA256(host_key)
    let pt: &[u8] = if pt.len() > 32 {
        use sha2::{Digest, Sha256};
        let h = Sha256::digest(host.as_bytes());
        if pt[..32] == h[..] {
            &pt[32..]
        } else {
            &pt
        }
    } else {
        &pt
    };

    Some(String::from_utf8_lossy(pt).to_string())
}

/// 从 multi_sids 中匹配当前 sessionid 对应的 user_id。
///
/// multi_sids 完整格式：`uid%3Asid%7Cuid%3Asid`（所有分隔符都被 percent-encode）。
/// 正确顺序：先 decode 整个 raw → 再按 `|`（或兜底逗号） split → 每条按 `:` 拆 uid:sid。
/// 严格匹配：sessionid 未命中时返回 None。
fn resolve_user_id(multi_sids: &Option<String>, sessionid: &str) -> Option<String> {
    let raw = multi_sids.as_ref()?;
    let decoded_all = percent_decode(raw);
    let entries: Vec<&str> = if decoded_all.contains('|') {
        decoded_all.split('|').collect()
    } else {
        decoded_all.split(',').collect()
    };
    for entry in entries {
        let e = entry.trim();
        if e.is_empty() { continue; }
        if let Some((uid, sid)) = e.split_once(':') {
            let uid = uid.trim();
            if sid.trim() == sessionid && !uid.is_empty() {
                return Some(uid.to_string());
            }
        }
    }
    None
}

/// 最小 percent-decode（仅 Cookie 值里常见场景）。
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_decode_basic() {
        assert_eq!(percent_decode("123%3Aabc"), "123:abc");
        assert_eq!(percent_decode("plain"), "plain");
    }

    #[test]
    fn resolve_user_id_matches_session() {
        // 逗号分隔（旧测试数据）
        let m_comma = Some("111%3AsidA,222%3AsidB".to_string());
        assert_eq!(resolve_user_id(&m_comma, "sidB").as_deref(), Some("222"));
        // pipe 分隔（真实豆包客户端）
        let m_pipe = Some("4333589105537683%3AsidX|1831141604067483%3AsidY".to_string());
        assert_eq!(resolve_user_id(&m_pipe, "sidX").as_deref(), Some("4333589105537683"));
        assert_eq!(resolve_user_id(&m_pipe, "sidY").as_deref(), Some("1831141604067483"));
        // 无匹配 → None（严格模式，不再 first_uid 兜底）
        assert_eq!(resolve_user_id(&m_pipe, "nope"), None);
    }

    /// 依赖本机豆包客户端登录态，默认跳过。手动执行：
    /// `cargo test read_local_client_login -- --ignored --nocapture`
    #[test]
    #[ignore = "依赖本机豆包客户端登录态"]
    fn read_local_client_login() {
        let login = read_client_login().expect("读取客户端登录态失败");
        println!(
            "user_id={} cookie_len={} has_msToken={} has_tt_scid={}",
            login.user_id,
            login.cookies.len(),
            login.cookies.contains("msToken="),
            login.cookies.contains("tt_scid=")
        );
        assert!(!login.user_id.is_empty());
        assert!(login.cookies.contains("sessionid="));
    }
}
