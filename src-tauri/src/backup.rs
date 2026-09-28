//! 备份与恢复：所有备份统一存放在游戏存档目录 profile_N 下的 DDSL_save 子目录中
//! （DDSL_save 与官方 backup 文件夹同级），profile_N 顶层只保留游戏存档文件。
use std::fs;
use std::path::Path;

use chrono::Local;

use crate::{paths, profiles, week};

/// 备份存放的子目录名（位于各 profile_N 下，与官方 backup 同级）。
const SAVE_DIR: &str = "DDSL_save";

/// 全局备份串行锁：所有备份/恢复的复制操作排队执行，避免并发 IO。
static BACKUP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[derive(serde::Serialize, Clone)]
pub struct BackupEntry {
    pub name: String,
    pub timestamp: String,
    pub week: Option<u32>,
    pub kind: String, // auto / manual
}

#[derive(serde::Serialize)]
pub struct RestoreResult {
    pub cache_deleted: bool,
}

/// 复制目录下的顶层文件（跳过子目录）。
/// 备份时：官方 profile_N 中的 backup 文件夹（玩家进入游戏时的官方备份）对我们无用，
/// 因此只保存存档文件本身，不包含任何子目录。
/// 恢复时：备份目录同样只有顶层文件，直接平铺回 profile_N 顶层即可。
fn copy_files_only(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for e in fs::read_dir(src)? {
        let e = e?;
        let p = e.path();
        if p.is_file() {
            fs::copy(&p, dst.join(e.file_name()))?;
        }
    }
    Ok(())
}

/// 清空目录内容（保留目录本身），但保留 backup 与 DDSL_save 两个子目录（以及其中的内容）。
/// 恢复只替换游戏存档文件，不触碰官方备份与我们的备份。
fn clear_dir_keep_savedirs(dir: &Path) -> std::io::Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for e in fs::read_dir(dir)? {
        let e = e?;
        let name = e.file_name().to_string_lossy().into_owned();
        let p = e.path();
        if p.is_dir() {
            if name == "backup" || name == SAVE_DIR {
                continue;
            }
            fs::remove_dir_all(&p)?;
        } else {
            fs::remove_file(&p)?;
        }
    }
    Ok(())
}

/// 执行一次备份（kind: auto / manual），返回条目信息。
pub fn backup_profile(remote: &Path, profile: &str, kind: &str) -> Result<BackupEntry, String> {
    let _guard = BACKUP_LOCK.lock().map_err(|_| "备份锁占用".to_string())?;
    let src = remote.join(profile);
    if !src.is_dir() {
        return Err(format!("档案 {profile} 不存在"));
    }
    // total_weeks 即游戏内当前周数（第0周=教学关）
    let week = week::read_total_weeks(&src.join("persist.campaign_log.json"));
    let ts = Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
    let week_tag = match week {
        Some(w) => format!("week{w:02}"),
        None => "week_unknown".to_string(),
    };
    let dir_name = format!("{ts}_{week_tag}_{kind}");
    let dst = src.join(SAVE_DIR).join(&dir_name); // 备份统一放在 profile_N\DDSL_save 下
    // 只备份存档文件本身（不包含子目录，如官方 backup 文件夹、其他备份）
    copy_files_only(&src, &dst).map_err(|e| format!("备份失败: {e}"))?;
    Ok(BackupEntry {
        name: dir_name,
        timestamp: ts,
        week,
        kind: kind.to_string(),
    })
}

/// 从备份恢复：直接用备份覆盖当前档案（保留我们自己的备份文件夹），随后清理 remotecache.vdf。
/// 注意：恢复不可撤销，请确认已自行备份需要保留的进度。
pub fn restore_profile(
    remote: &Path,
    profile: &str,
    backup_name: &str,
) -> Result<RestoreResult, String> {
    if game_running() {
        return Err("检测到游戏正在运行，请先退出游戏再恢复存档".into());
    }
    let src = remote.join(profile).join(SAVE_DIR).join(backup_name);
    if !src.is_dir() {
        return Err(format!("备份 {backup_name} 不存在"));
    }

    // 恢复期间串行化，避免与其他备份并发
    let _guard = BACKUP_LOCK.lock().map_err(|_| "备份锁占用".to_string())?;
    let target = remote.join(profile);
    clear_dir_keep_savedirs(&target).map_err(|e| format!("清理目标档案失败: {e}"))?;
    copy_files_only(&src, &target).map_err(|e| format!("恢复失败: {e}"))?;

    let cache_deleted = match paths::remotecache_path(remote) {
        Some(p) => fs::remove_file(p).is_ok(),
        None => false,
    };
    Ok(RestoreResult { cache_deleted })
}

/// 解析备份文件夹名（元数据编码在名字里）。
fn parse_backup_name(name: &str) -> Option<BackupEntry> {
    // 格式：2026-09-28_19-00-00_week25_auto
    let parts: Vec<&str> = name.split('_').collect();
    if parts.len() < 4 {
        return None;
    }
    let timestamp = format!("{}_{}", parts[0], parts[1]);
    let week = parts[2]
        .strip_prefix("week")
        .and_then(|s| s.parse::<u32>().ok());
    let kind = parts[3].to_string();
    Some(BackupEntry {
        name: name.to_string(),
        timestamp,
        week,
        kind,
    })
}

/// 列出某档案的所有备份（位于 profile_N\DDSL_save 下），按名称字典序降序（即时间降序，格式统一）。
pub fn list_backups(remote: &Path, profile: &str) -> Vec<BackupEntry> {
    let mut out = Vec::new();
    let dir = remote.join(profile).join(SAVE_DIR);
    if let Ok(rd) = fs::read_dir(&dir) {
        for e in rd.flatten() {
            if e.path().is_dir() {
                if let Some(entry) = parse_backup_name(&e.file_name().to_string_lossy()) {
                    out.push(entry);
                }
            }
        }
    }
    out.sort_by(|a, b| b.name.cmp(&a.name));
    out
}

/// 判断某周是否已存在自动档（用于去重，保证每周只产生一个自动档）。
pub fn has_auto_for_week(remote: &Path, profile: &str, week: u32) -> bool {
    list_backups(remote, profile)
        .iter()
        .any(|b| b.kind == "auto" && b.week == Some(week))
}

/// 删除单个备份。
pub fn delete_backup(remote: &Path, profile: &str, backup_name: &str) -> Result<(), String> {
    let p = remote.join(profile).join(SAVE_DIR).join(backup_name);
    if !p.is_dir() {
        return Err(format!("备份 {backup_name} 不存在"));
    }
    fs::remove_dir_all(&p).map_err(|e| format!("删除失败: {e}"))
}

/// 校验档案名：必须是 remote 下真实存在的存档位（profile_0~8），防路径注入。
fn validate_profile(remote: &Path, profile: &str) -> Result<(), String> {
    if profiles::list_profiles(remote).iter().any(|p| p == profile) {
        Ok(())
    } else {
        Err(format!("无效档案名: {profile}"))
    }
}

/// 删除某档案的全部备份（保留游戏存档本身与官方 backup 目录）。
pub fn clear_profile_backups(remote: &Path, profile: &str) -> Result<(), String> {
    validate_profile(remote, profile)?;
    let dir = remote.join(profile).join(SAVE_DIR);
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|e| format!("删除备份失败: {e}"))?;
    }
    Ok(())
}

/// 删除整个档案：存档位目录（游戏进度 + 备份 + 官方 backup 目录）一并删除。
/// 游戏运行中禁止执行，防止游戏写盘冲突。
pub fn delete_profile(remote: &Path, profile: &str) -> Result<(), String> {
    if game_running() {
        return Err("检测到游戏正在运行，请先退出游戏再删除档案".into());
    }
    validate_profile(remote, profile)?;
    let dir = remote.join(profile);
    fs::remove_dir_all(&dir).map_err(|e| format!("删除档案失败: {e}"))
}

/// 进程检测结果缓存：5 秒内复用，避免频繁启动 tasklist 子进程（黑窗闪现与卡顿的根源）。
static GAME_CACHE: std::sync::Mutex<Option<(std::time::Instant, bool)>> = std::sync::Mutex::new(None);

fn detect_game() -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        if let Ok(out) = std::process::Command::new("tasklist")
            .args(["/FI", "IMAGENAME eq Darkest.exe", "/NH"])
            // CREATE_NO_WINDOW：不创建控制台窗口（避免每次查询弹出黑色命令窗）
            .creation_flags(0x0800_0000)
            .output()
        {
            let s = String::from_utf8_lossy(&out.stdout);
            return s.contains("Darkest.exe");
        }
    }
    #[cfg(not(windows))]
    {
        // 非 Windows 平台不做进程检测
    }
    false
}

/// 检测游戏进程是否运行（Darkest.exe），结果缓存 5 秒。
pub fn game_running() -> bool {
    let now = std::time::Instant::now();
    if let Ok(mut cache) = GAME_CACHE.lock() {
        if let Some((t, v)) = *cache {
            if now.duration_since(t) < std::time::Duration::from_secs(5) {
                return v;
            }
        }
        let v = detect_game();
        *cache = Some((now, v));
        return v;
    }
    detect_game()
}
