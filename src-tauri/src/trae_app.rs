//! Trae 系应用变体定义（Trae CN / TRAE WORK / 国际版 Trae）
//!
//! 国内版与国际版的数据目录、安装路径、进程名、登录站点、API Host 均不同，
//! storage.json 键名与加密格式完全一致（见 crypto.rs），可共用同一套逻辑。
//!
//! TRAE WORK 即原 TRAE SOLO CN（2026-06 更名，站点 work.trae.cn），
//! 同时兼容新旧两种安装名（TRAE SOLO CN.app / TraeWork.app）与数据目录。

use anyhow::{anyhow, Result};
use std::fs;
use std::path::PathBuf;
use std::sync;
use std::sync::OnceLock;

#[derive(Debug, Clone, serde::Serialize)]
pub struct TraeAppVariant {
    /// 唯一标识（持久化用）
    pub key: &'static str,
    /// 显示名称
    pub display_name: &'static str,
    /// 数据目录名候选（Application Support / APPDATA 下，按优先级取第一个存在的）
    pub data_dir_names: &'static [&'static str],
    /// macOS 安装路径候选（按优先级）
    pub bundle_paths: &'static [&'static str],
    /// pgrep/pkill -f 匹配模式候选（含空格需整串匹配命令行）
    pub process_patterns: &'static [&'static str],
    /// osascript 应用名候选（按优先级）
    pub osascript_names: &'static [&'static str],
    /// 浏览器登录站点
    pub login_url: &'static str,
    /// 默认 API Host
    pub api_host: &'static str,
    /// 是否国内版
    pub is_cn: bool,
}

pub const TRAE_CN: TraeAppVariant = TraeAppVariant {
    key: "trae-cn",
    display_name: "Trae CN（国内版）",
    data_dir_names: &["Trae CN"],
    bundle_paths: &["/Applications/Trae CN.app", "~/Applications/Trae CN.app"],
    process_patterns: &["Trae CN.app/Contents/MacOS"],
    osascript_names: &["Trae CN"],
    login_url: "https://www.trae.cn",
    api_host: "https://api.trae.cn",
    is_cn: true,
};

/// TRAE WORK（原 TRAE SOLO CN，2026-06 更名）
pub const TRAE_WORK: TraeAppVariant = TraeAppVariant {
    key: "trae-work",
    display_name: "TRAE WORK（原 TRAE SOLO CN）",
    data_dir_names: &["TRAE SOLO CN", "TraeWork", "Trae Work"],
    bundle_paths: &[
        "/Applications/TRAE SOLO CN.app",
        "/Applications/TraeWork.app",
        "/Applications/Trae Work.app",
        "~/Applications/TRAE SOLO CN.app",
        "~/Applications/TraeWork.app",
    ],
    process_patterns: &[
        "TRAE SOLO CN.app/Contents/MacOS",
        "TraeWork.app/Contents/MacOS",
        "Trae Work.app/Contents/MacOS",
    ],
    osascript_names: &["TRAE SOLO CN", "TraeWork", "Trae Work"],
    login_url: "https://www.trae.cn",
    api_host: "https://api.trae.cn",
    is_cn: true,
};

pub const TRAE_GLOBAL: TraeAppVariant = TraeAppVariant {
    key: "trae",
    display_name: "Trae（国际版）",
    data_dir_names: &["Trae"],
    bundle_paths: &["/Applications/Trae.app", "~/Applications/Trae.app"],
    process_patterns: &["Trae.app/Contents/MacOS"],
    osascript_names: &["Trae"],
    login_url: "https://www.trae.ai",
    api_host: "https://api-sg-central.trae.ai",
    is_cn: false,
};

/// 全部变体（探测顺序即默认优先级）
pub fn all_variants() -> &'static [TraeAppVariant] {
    &[TRAE_CN, TRAE_WORK, TRAE_GLOBAL]
}

pub fn find_variant(key: &str) -> Result<&'static TraeAppVariant> {
    // 旧标识兼容：trae-solo-cn -> trae-work
    let key = match key {
        "trae-solo-cn" => "trae-work",
        k => k,
    };
    all_variants()
        .iter()
        .find(|v| v.key == key)
        .ok_or_else(|| anyhow!("未知的 Trae 应用标识: {}", key))
}

/// 展开 ~ 的路径
fn expand_home(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(path)
}

/// 变体是否已安装（存在 .app 或数据目录）
pub fn is_variant_installed(v: &TraeAppVariant) -> bool {
    if v.bundle_paths.iter().any(|p| expand_home(p).exists()) {
        return true;
    }
    v.data_dir_names
        .iter()
        .any(|name| data_base_dir().join(name).exists())
}

/// 数据目录的父目录（Application Support / APPDATA）
#[cfg(target_os = "macos")]
fn data_base_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home)
        .join("Library")
        .join("Application Support")
}

#[cfg(target_os = "windows")]
fn data_base_dir() -> PathBuf {
    let appdata = std::env::var("APPDATA").unwrap_or_default();
    PathBuf::from(appdata)
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn data_base_dir() -> PathBuf {
    PathBuf::new()
}

/// 变体的数据目录：取第一个存在的候选目录；都不存在时返回第一个候选（供创建）
pub fn data_dir_of(v: &TraeAppVariant) -> PathBuf {
    let base = data_base_dir();
    for name in v.data_dir_names {
        let p = base.join(name);
        if p.exists() {
            return p;
        }
    }
    base.join(v.data_dir_names[0])
}

/// 配置文件目录（与 trae_path.txt 同级）
fn selection_file() -> Result<PathBuf> {
    let proj_dirs = directories::ProjectDirs::from("com", "marscey", "traejumper")
        .ok_or_else(|| anyhow!("无法获取应用数据目录"))?;
    let config_dir = proj_dirs.config_dir();
    fs::create_dir_all(config_dir)?;
    Ok(config_dir.join("trae_app.txt"))
}

static CURRENT: OnceLock<sync::RwLock<Option<&'static TraeAppVariant>>> = OnceLock::new();

fn current_cell() -> &'static sync::RwLock<Option<&'static TraeAppVariant>> {
    CURRENT.get_or_init(|| sync::RwLock::new(None))
}

/// 获取当前变体（首次调用时从配置读取或自动探测并缓存，可被 set_current 更新）
pub fn current() -> &'static TraeAppVariant {
    {
        let cell = current_cell().read().unwrap();
        if let Some(v) = *cell {
            return v;
        }
    }

    // 首次：读取配置或自动探测
    let selected = selection_file()
        .ok()
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|s| find_variant(s.trim()).ok());
    let v = match selected {
        Some(v) => v,
        None => all_variants()
            .iter()
            .find(|v| is_variant_installed(v))
            .unwrap_or(&TRAE_GLOBAL),
    };
    let _ = persist_selection(v);
    *current_cell().write().unwrap() = Some(v);
    v
}

/// 仅写配置文件（不更新缓存）
fn persist_selection(v: &TraeAppVariant) -> Result<()> {
    let path = selection_file()?;
    fs::write(&path, v.key)?;
    Ok(())
}

/// 设置当前变体并持久化
pub fn set_current(v: &'static TraeAppVariant) -> Result<()> {
    persist_selection(v)?;
    *current_cell().write().unwrap() = Some(v);
    println!("[INFO] 当前管理目标应用: {} ({})", v.display_name, v.key);
    Ok(())
}

/// 变体信息（供前端展示）
#[derive(Debug, serde::Serialize)]
pub struct TraeAppInfo {
    pub key: String,
    pub display_name: String,
    pub installed: bool,
    pub data_dir: String,
    pub is_current: bool,
}

pub fn list_app_infos() -> Vec<TraeAppInfo> {
    let cur = current();
    all_variants()
        .iter()
        .map(|v| TraeAppInfo {
            key: v.key.to_string(),
            display_name: v.display_name.to_string(),
            installed: is_variant_installed(v),
            data_dir: data_dir_of(v).to_string_lossy().to_string(),
            is_current: v.key == cur.key,
        })
        .collect()
}
