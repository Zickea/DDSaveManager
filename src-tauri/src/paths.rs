//! 存档路径定位：优先读取 Steam 注册表"当前登录账号"，其次按存档最新修改时间选账号，
//! 非 Steam 版兜底 Documents/Darkest。
//! 修复点：userdata 下可能有多个 Steam 账号目录，不能取枚举到的第一个，
//! 必须指向当前活跃账号，否则备份/恢复/监控会作用到错误的存档。
use std::path::{Path, PathBuf};

/// 从注册表读取 Steam 安装根目录（HKCU\Software\Valve\Steam\SteamPath）。
/// SteamPath 通常直接是 Steam 根目录（如 "C:/Program Files (x86)/Steam"），
/// 个别版本可能存的是 steam.exe 完整路径，统一转成根目录。
/// 这是覆盖"Steam 自定义安装位置"的关键：不依赖 Program Files 硬编码路径。
#[cfg(windows)]
fn steam_install_dir_from_registry() -> Option<PathBuf> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu.open_subkey(r"Software\Valve\Steam").ok()?;
    let raw: String = key.get_value("SteamPath").ok()?;
    let p = PathBuf::from(&raw);
    if p.join("steam.exe").is_file() {
        Some(p) // 本身就是 Steam 根目录
    } else if p
        .file_name()
        .map(|n| n.eq_ignore_ascii_case("steam.exe"))
        .unwrap_or(false)
    {
        p.parent().map(|d| d.to_path_buf()) // 存的是 steam.exe 完整路径，取父目录
    } else {
        Some(p)
    }
}

#[cfg(not(windows))]
fn steam_install_dir_from_registry() -> Option<PathBuf> {
    None
}

/// 可能的 Steam userdata 根目录列表：
/// 1) 注册表 SteamPath（覆盖自定义安装位置，优先）
/// 2) ProgramFiles(x86) / ProgramFiles 硬编码兜底（标准安装）
fn steam_userdata_roots() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(dir) = steam_install_dir_from_registry() {
        let p = dir.join("userdata");
        if !v.contains(&p) {
            v.push(p);
        }
    }
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

/// 非 Steam 版存档目录候选：Documents\Darkest，以及 OneDrive 重定向后的 Documents。
fn darkest_documents_candidates() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Ok(profile) = std::env::var("USERPROFILE") {
        let base = PathBuf::from(profile);
        v.push(base.join("Documents").join("Darkest"));
        v.push(base.join("OneDrive").join("Documents").join("Darkest"));
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
/// 每一步尝试都写日志：定位失败时用户可打开日志文件，直接看到卡在哪一步。
pub fn locate_remote_dir() -> Option<PathBuf> {
    crate::dlog!("[paths] 开始定位存档目录");
    // 1) 注册表当前登录账号优先（多账号机器上保证指向正在使用的账号）
    match steam_active_user_id() {
        Some(id) => {
            crate::dlog!("[paths] 注册表 ActiveUser = {id}");
            for root in steam_userdata_roots() {
                let remote = root.join(id.to_string()).join("262060").join("remote");
                if remote.is_dir() {
                    crate::dlog!("[paths] 命中 ActiveUser 路径: {}", remote.display());
                    return Some(remote);
                }
            }
            crate::dlog!("[paths] ActiveUser 账号下未找到 262060/remote，继续兜底");
        }
        None => {
            crate::dlog!("[paths] 注册表无 ActiveUser（Steam 可能尚未运行），跳过步骤 1");
        }
    }
    // 2) 兜底：所有候选按最新修改时间排序，选最近活跃的账号
    let mut candidates = collect_remote_candidates();
    if !candidates.is_empty() {
        candidates.sort_by(|a, b| freshness(b).cmp(&freshness(a)));
        crate::dlog!(
            "[paths] 步骤 2 兜底命中 {} 个候选，选中: {}",
            candidates.len(),
            candidates[0].display()
        );
        return candidates.into_iter().next();
    }
    crate::dlog!("[paths] 步骤 2 无任何候选目录");
    // 3) 非 Steam 版兜底：Documents\Darkest（含 OneDrive 重定向）
    for d in darkest_documents_candidates() {
        if d.is_dir() {
            crate::dlog!("[paths] 步骤 3 兜底命中: {}", d.display());
            return Some(d);
        }
    }
    crate::dlog!("[paths] 三步定位全部失败，返回 None");
    None
}

/// Steam 云同步缓存文件 remotecache.vdf，位于 remote 的上一级（262060 根目录）。
pub fn remotecache_path(remote: &Path) -> Option<PathBuf> {
    remote
        .parent()
        .map(|p| p.join("remotecache.vdf"))
        .filter(|p| p.exists())
}
