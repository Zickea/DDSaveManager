//! 存档路径定位：优先读取 Steam 注册表"当前登录账号"，其次按存档最新修改时间选账号，
//! 非 Steam 版兜底 Documents/Darkest。
//! 修复点：userdata 下可能有多个 Steam 账号目录，不能取枚举到的第一个，
//! 必须指向当前活跃账号，否则备份/恢复/监控会作用到错误的存档。
use std::path::{Path, PathBuf};

/// 可能的 Steam userdata 根目录列表（x86 与非 x86 安装路径）。
fn steam_userdata_roots() -> Vec<PathBuf> {
    let mut v = Vec::new();
    for var in ["ProgramFiles(x86)", "ProgramFiles"] {
        if let Ok(pf) = std::env::var(var) {
            let p = PathBuf::from(pf).join("Steam").join("userdata");
            if !v.contains(&p) {
                v.push(p);
            }
        }
    }
    v
}

/// 读取 Steam 注册表中"当前/最近登录用户"的 ID（HKCU\Software\Valve\Steam\ActiveProcess\ActiveUser）。
#[cfg(windows)]
fn steam_active_user_id() -> Option<u32> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu.open_subkey(r"Software\Valve\Steam\ActiveProcess").ok()?;
    key.get_value::<u32, _>("ActiveUser").ok()
}
#[cfg(not(windows))]
fn steam_active_user_id() -> Option<u32> {
    None
}

/// 收集 userdata 下所有包含 262060/remote 的候选目录。
fn collect_remote_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for root in steam_userdata_roots() {
        if let Ok(entries) = std::fs::read_dir(&root) {
            for e in entries.flatten() {
                if !e.path().is_dir() {
                    continue;
                }
                let remote = e.path().join("262060").join("remote");
                if remote.is_dir() {
                    out.push(remote);
                }
            }
        }
    }
    out
}

/// 候选目录的新鲜度：内部存档文件的最后修改时间（越大越新，用于判断最近活跃的账号）。
fn freshness(dir: &Path) -> std::time::SystemTime {
    let mut latest = std::time::UNIX_EPOCH;
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            if let Ok(m) = e.metadata().and_then(|m| m.modified()) {
                if m > latest {
                    latest = m;
                }
            }
        }
    }
    latest
}

/// 定位暗黑地牢1的存档目录（remote 或 Documents/Darkest）。
pub fn locate_remote_dir() -> Option<PathBuf> {
    // 1) 注册表当前登录账号优先（多账号机器上保证指向正在使用的账号）
    if let Some(id) = steam_active_user_id() {
        for root in steam_userdata_roots() {
            let remote = root.join(id.to_string()).join("262060").join("remote");
            if remote.is_dir() {
                return Some(remote);
            }
        }
    }
    // 2) 兜底：所有候选按最新修改时间排序，选最近活跃的账号
    let mut candidates = collect_remote_candidates();
    if !candidates.is_empty() {
        candidates.sort_by(|a, b| freshness(b).cmp(&freshness(a)));
        return candidates.into_iter().next();
    }
    // 3) 非 Steam 版兜底：%USERPROFILE%\Documents\Darkest（结构与 remote 相同）
    if let Ok(docs) = std::env::var("USERPROFILE") {
        let d = PathBuf::from(docs).join("Documents").join("Darkest");
        if d.is_dir() {
            return Some(d);
        }
    }
    None
}

/// Steam 云同步缓存文件 remotecache.vdf，位于 remote 的上一级（262060 根目录）。
pub fn remotecache_path(remote: &Path) -> Option<PathBuf> {
    remote
        .parent()
        .map(|p| p.join("remotecache.vdf"))
        .filter(|p| p.exists())
}
