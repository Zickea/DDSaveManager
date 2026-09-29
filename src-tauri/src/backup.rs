//! 备份与恢复：所有备份统一存放在游戏存档目录 profile_N 下的 DDSL_save 子目录中
//! （DDSL_save 与官方 backup 文件夹同级），profile_N 顶层只保留游戏存档文件。
use std::fs;
use std::path::Path;

use chrono::Local;

use crate::{dlog, paths, profiles, week};

/// 备份存放的子目录名（位于各 profile_N 下，与官方 backup 同级）。
const SAVE_DIR: &str = "DDSL_save";

/// 副本会话信号文件：进副本时创建、副本结束（回城）时删除。
/// 存在 = 档案处于副本（未结算）状态；在副本状态下备份/恢复城镇档，
/// 会导致本地文件与 Steam 云台账（remotecache.vdf）记录的文件大小不一致，
/// 游戏启动时 Steamworks StorageManager 校验失败（didn't read whole file?）无法加载。
pub const RAID_SIGNAL: &str = "persist.raid.json";

/// 全局备份串行锁：所有备份/恢复的复制操作排队执行，避免并发 IO。
static BACKUP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[derive(serde::Serialize, Clone)]
pub struct BackupEntry {
    pub name: String,
    pub timestamp: String,
    pub week: Option<u32>,
    pub kind: String, // auto / manual
    /// 手动备份备注（来自备份目录内 note.txt；auto 档为 None）
    pub note: Option<String>,
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
/// note 仅手动备份时使用：写入备份目录内 note.txt（不编码进目录名，
/// 保证目录名格式稳定、解析器不变，且备注可含任意字符）。
pub fn backup_profile(
    remote: &Path,
    profile: &str,
    kind: &str,
    note: Option<&str>,
) -> Result<BackupEntry, String> {
    let _guard = BACKUP_LOCK.lock().map_err(|_| "备份锁占用".to_string())?;
    let src = remote.join(profile);
    if !src.is_dir() {
        return Err(format!("档案 {profile} 不存在"));
    }
    // read_total_weeks 已返回玩家视角周数（total_weeks - 1，第0周=教学关）
    let week = week::read_total_weeks(&src.join("persist.campaign_log.json"));
    let ts = Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
    let week_tag = match week {
        Some(w) => format!("week{w:02}"),
        None => "week_unknown".to_string(),
    };
    let dir_name = format!("{ts}_{week_tag}_{kind}");
    let dst = src.join(SAVE_DIR).join(&dir_name); // 备份统一放在 profile_N\DDSL_save 下
    // 同秒重复备份（如连续两次手动备份）会导致目录名冲突：加序号区分，避免覆盖/混入上次内容
    let dst = {
        let mut candidate = dst;
        let mut n = 1;
        while candidate.exists() {
            candidate = src.join(SAVE_DIR).join(format!("{dir_name}_{n}"));
            n += 1;
        }
        candidate
    };
    // 只备份存档文件本身（不包含子目录，如官方 backup 文件夹、其他备份）
    copy_files_only(&src, &dst).map_err(|e| {
        dlog!("[backup] {profile} 复制失败（{kind}）: {e}");
        format!("备份失败: {e}")
    })?;
    // 备注落盘（失败不影响备份结果）
    if let Some(text) = note.filter(|t| !t.trim().is_empty()) {
        let _ = fs::write(dst.join("note.txt"), text.trim());
    }
    Ok(BackupEntry {
        name: dst
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or(dir_name),
        timestamp: ts,
        week,
        kind: kind.to_string(),
        note: note.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()),
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
    // 格式：2026-09-28_19-00-00_week25_auto 或 ..._week_unknown_auto（周数不可读）
    let parts: Vec<&str> = name.split('_').collect();
    if parts.len() < 4 {
        return None;
    }
    let timestamp = format!("{}_{}", parts[0], parts[1]);
    // weekNN 正常形态；week_unknown 会拆成 ["week", "unknown", kind] 三段
    let (week, kind) = if parts[2] == "week" {
        (
            None,
            parts.get(4).map(|s| s.to_string()).unwrap_or_default(),
        )
    } else {
        (
            parts[2].strip_prefix("week").and_then(|s| s.parse::<u32>().ok()),
            parts.get(3).map(|s| s.to_string()).unwrap_or_default(),
        )
    };
    Some(BackupEntry {
        name: name.to_string(),
        timestamp,
        week,
        kind,
        note: None,
    })
}

/// 列出某档案的所有备份（位于 profile_N\DDSL_save 下），按名称字典序降序（即时间降序，格式统一）。
/// 每个条目读取目录内 note.txt 作为备注（缺失则为 None）。
pub fn list_backups(remote: &Path, profile: &str) -> Vec<BackupEntry> {
    let mut out = Vec::new();
    let dir = remote.join(profile).join(SAVE_DIR);
    if let Ok(rd) = fs::read_dir(&dir) {
        for e in rd.flatten() {
            if e.path().is_dir() {
                if let Some(mut entry) = parse_backup_name(&e.file_name().to_string_lossy()) {
                    entry.note = fs::read_to_string(e.path().join("note.txt"))
                        .ok()
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty());
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

/// 自动备份保留策略：保留最近 keep_weeks 个不同周的 auto 档，删除更早的 auto 档。
/// - keep_weeks == 0：不清理（保留全部）
/// - 手动档始终保留；week 未知（week_unknown）的 auto 档保守保留，不参与清理
/// - 删除失败只记录不中断（清理是优化项，不阻塞主流程）
pub fn prune_auto_backups(remote: &Path, profile: &str, keep_weeks: u32) {
    if keep_weeks == 0 {
        return;
    }
    let autos: Vec<BackupEntry> = list_backups(remote, profile)
        .into_iter()
        .filter(|b| b.kind == "auto")
        .collect();
    // list_backups 已按名称（时间）降序：首个遇到的某周即为该周最新档
    let mut keep: std::collections::HashSet<u32> = std::collections::HashSet::new();
    for b in &autos {
        if let Some(w) = b.week {
            if keep.len() < keep_weeks as usize {
                keep.insert(w);
            }
        }
    }
    for b in autos {
        if let Some(w) = b.week {
            if !keep.contains(&w) {
                if let Err(e) = delete_backup(remote, profile, &b.name) {
                    dlog!("[backup] {profile} 清理旧自动档 {0} 失败: {e}", b.name);
                }
            }
        }
    }
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

/// 检测档案是否处于副本（未结算）状态：副本信号文件 persist.raid.json 存在即为副本中。
/// 副本状态（含强退残留）下禁止手动备份与恢复城镇档——见 RAID_SIGNAL 注释的坏档原因。
pub fn is_in_raid(remote: &Path, profile: &str) -> bool {
    remote.join(profile).join(RAID_SIGNAL).exists()
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造一个测试专用的 remote 目录骨架（tag 保证并行测试互不污染）。
    fn temp_remote(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("ddsl_test_{tag}_{}", std::process::id()));
        let profile = dir.join("profile_0");
        std::fs::create_dir_all(profile.join(SAVE_DIR)).unwrap();
        // 构造"游戏存档"占位文件，使 backup_profile 有内容可复制
        std::fs::write(profile.join("persist.game.json"), b"x").unwrap();
        dir
    }

    fn mk_backup(remote: &Path, name: &str) {
        let dir = remote.join("profile_0").join(SAVE_DIR).join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("persist.game.json"), b"x").unwrap();
    }

    #[test]
    fn parse_backup_name_normal() {
        let e = parse_backup_name("2026-09-28_19-00-00_week25_auto").unwrap();
        assert_eq!(e.timestamp, "2026-09-28_19-00-00");
        assert_eq!(e.week, Some(25));
        assert_eq!(e.kind, "auto");
    }

    #[test]
    fn parse_backup_name_unknown_week() {
        let e = parse_backup_name("2026-09-28_19-00-00_week_unknown_manual").unwrap();
        assert_eq!(e.week, None);
        assert_eq!(e.kind, "manual");
        // week_unknown 的 auto 档也要正确识别为 auto
        let e = parse_backup_name("2026-09-28_19-00-00_week_unknown_auto").unwrap();
        assert_eq!(e.kind, "auto");
    }

    #[test]
    fn parse_backup_name_invalid() {
        assert!(parse_backup_name("random_dir").is_none());
        assert!(parse_backup_name("2026-09-28_19-00-00_week25").is_none());
    }

    #[test]
    fn list_backups_sorted_desc() {
        let remote = temp_remote("list");
        mk_backup(&remote, "2026-09-28_19-00-00_week01_auto");
        mk_backup(&remote, "2026-09-30_10-00-00_week03_auto");
        mk_backup(&remote, "2026-09-29_08-00-00_week02_manual");
        let list = list_backups(&remote, "profile_0");
        let names: Vec<&str> = list.iter().map(|b| b.name.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "2026-09-30_10-00-00_week03_auto",
                "2026-09-29_08-00-00_week02_manual",
                "2026-09-28_19-00-00_week01_auto"
            ]
        );
    }

    #[test]
    fn has_auto_for_week_dedup() {
        let remote = temp_remote("dedup");
        mk_backup(&remote, "2026-09-28_19-00-00_week05_auto");
        assert!(has_auto_for_week(&remote, "profile_0", 5));
        assert!(!has_auto_for_week(&remote, "profile_0", 6));
        // 手动档不算 auto
        mk_backup(&remote, "2026-09-29_08-00-00_week05_manual");
        assert!(has_auto_for_week(&remote, "profile_0", 5));
    }

    #[test]
    fn prune_keeps_recent_and_manual() {
        let remote = temp_remote("prune");
        mk_backup(&remote, "2026-09-28_19-00-00_week01_auto");
        mk_backup(&remote, "2026-09-29_08-00-00_week02_auto");
        mk_backup(&remote, "2026-09-30_10-00-00_week03_auto");
        mk_backup(&remote, "2026-09-28_20-00-00_week01_manual");
        mk_backup(&remote, "2026-09-28_21-00-00_week_unknown_auto");

        prune_auto_backups(&remote, "profile_0", 1); // 只保留最近 1 个不同周的 auto

        let remaining: Vec<String> = list_backups(&remote, "profile_0")
            .into_iter()
            .map(|b| b.name)
            .collect();
        assert!(remaining.contains(&"2026-09-30_10-00-00_week03_auto".to_string()));
        assert!(remaining.contains(&"2026-09-28_20-00-00_week01_manual".to_string()));
        assert!(remaining.contains(&"2026-09-28_21-00-00_week_unknown_auto".to_string()));
        assert!(!remaining.contains(&"2026-09-28_19-00-00_week01_auto".to_string()));
        assert!(!remaining.contains(&"2026-09-29_08-00-00_week02_auto".to_string()));
    }

    #[test]
    fn prune_zero_keeps_all() {
        let remote = temp_remote("prune0");
        mk_backup(&remote, "2026-09-28_19-00-00_week01_auto");
        mk_backup(&remote, "2026-09-29_08-00-00_week02_auto");
        prune_auto_backups(&remote, "profile_0", 0); // 0 = 不清理
        assert_eq!(list_backups(&remote, "profile_0").len(), 2);
    }

    #[test]
    fn manual_backup_writes_and_reads_note() {
        let remote = temp_remote("note");
        // 备份（无 campaign_log → week_unknown，不影响备注链路）
        let entry = backup_profile(&remote, "profile_0", "manual", Some("打 Boss 前")).unwrap();
        assert_eq!(entry.note.as_deref(), Some("打 Boss 前"));
        assert_eq!(entry.kind, "manual");
        // note.txt 已写入备份目录
        let note_file = remote
            .join("profile_0")
            .join(SAVE_DIR)
            .join(&entry.name)
            .join("note.txt");
        assert_eq!(fs::read_to_string(note_file).unwrap(), "打 Boss 前");
        // list_backups 读回备注
        let listed = list_backups(&remote, "profile_0");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].note.as_deref(), Some("打 Boss 前"));
    }

    #[test]
    fn is_in_raid_detects_signal_file() {
        let remote = temp_remote("raid");
        // 无信号文件 = 城镇
        assert!(!is_in_raid(&remote, "profile_0"));
        // 存在 persist.raid.json = 副本中/强退残留
        std::fs::write(
            remote.join("profile_0").join(RAID_SIGNAL),
            b"{}",
        )
        .unwrap();
        assert!(is_in_raid(&remote, "profile_0"));
        // 删除后回到城镇
        std::fs::remove_file(remote.join("profile_0").join(RAID_SIGNAL)).unwrap();
        assert!(!is_in_raid(&remote, "profile_0"));
    }
}
