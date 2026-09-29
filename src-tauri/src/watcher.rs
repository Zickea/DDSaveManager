//! 文件监控：纯事件驱动，自动存档只发生在"回城"（副本会话文件被删除）时。
//! 设计原则：
//! - 副本会话由三个文件（persist.raid.json / map.json / loading_screen.json）同步表达，
//!   进副本时创建、副本结束时删除，内容更新不删除重建。三者是同一状态的冗余，
//!   因此只监听 persist.raid.json 一个信号文件即可 1:1 还原"副本开始/结束"，
//!   避免三件套逐个删除产生多个 Remove 事件（曾导致并发备份出重复档）。
//! - 存档动作只由 raid.json 的 Remove 事件触发（副本结束 = 回城 = 第 N 周进副本前的状态）；
//!   启动时若该文件不存在（在城镇）也会主动补一次，语义与回城完全一致。
//! - 去重不依赖状态机：backup_on_raid_end 按"当前周是否已有 auto 档"去重，
//!   防"启动补档线程"与"回城事件"在极端时序下重叠。
//! - 不设 in_raid 状态、不轮询、不重试：错过就错过，玩家可手动备份兜底。
//!   已知取舍：若游戏崩溃后重启时清理残留文件（未正常结算），Remove 会备份副本中途状态；
//!   该场景无法可靠防御（游戏恢复副本时会重新生成文件，从工具视角与正常副本无异），不防。
//! - Create 事件仅用于前端状态显示（"副本中"），不产生任何存档动作。
//! 注意：监控采用非递归监听（仅 profile_N 顶层），副本信号文件与存档文件均在顶层；
//! 备份文件夹位于 profile_N 顶层子目录，其内部写入不会产生监听事件，
//! 备份目录本身的创建/删除也不匹配信号文件过滤，因此不会造成事件风暴。
use std::path::Path;
use std::time::Duration;

use notify::{Event, EventKind, RecursiveMode, RecommendedWatcher, Watcher};
use tauri::{AppHandle, Emitter, Manager};

use crate::{backup, dlog, profiles, AppState};

fn emit(app: &AppHandle, event: &str, payload: impl serde::Serialize) {
    let value = serde_json::to_value(payload).unwrap_or(serde_json::Value::Null);
    let _ = app.emit(event, value);
}

/// 回城自动备份（同步函数，由独立线程调用）。
/// 含 5 秒存档稳定等待（防复制撞上游戏写盘）+ 按周去重。
fn backup_on_raid_end(app: AppHandle, remote: std::path::PathBuf, profile: String) {
    dlog!("[backup] {profile} 备份线程启动（5 秒稳定等待后执行）");
    std::thread::sleep(Duration::from_secs(5));
    let week = profiles::current_week(&remote, &profile);
    dlog!("[backup] {profile} 当前周数读取结果: {:?}", week);
    if let Some(w) = week {
        if backup::has_auto_for_week(&remote, &profile, w) {
            dlog!("[backup] {profile} 第{w}周已有 auto 档，去重跳过");
            return;
        }
    }
    match backup::backup_profile(&remote, &profile, "auto", None) {
        Ok(entry) => {
            dlog!("[backup] {profile} 备份成功: {}", entry.name);
            // 按设置清理旧自动档（保留最近 N 周）
            let keep = app
                .state::<AppState>()
                .settings
                .lock()
                .map(|s| s.keep_auto_weeks)
                .unwrap_or(0);
            backup::prune_auto_backups(&remote, &profile, keep);
            emit(&app, "auto-backup-done", entry);
        }
        Err(err) => {
            dlog!("[backup] {profile} 备份失败: {err}");
            emit(&app, "auto-backup-error", err);
        }
    }
}

/// 为单个档案启动监控：返回 watcher 句柄（drop 即停止）。
pub fn start_watcher(app: AppHandle, remote: &Path, profile: &str) -> notify::Result<RecommendedWatcher> {
    let profile = profile.to_string();
    let remote = remote.to_path_buf();

    // 启动探测：若信号文件不存在（当前在城镇），主动补一次回城自动备份。
    // 语义与回城 Remove 完全一致（城镇状态 = 本周进副本前状态），
    // 去重由 backup_on_raid_end 内的 has_auto_for_week 保证；
    // 若当前在副本中（信号文件存在）则不触发，等回城事件，避免存副本中途状态。
    let in_raid = remote.join(&profile).join(backup::RAID_SIGNAL).exists();
    dlog!(
        "[watcher] {profile} 启动探测: in_raid={in_raid}（{}）",
        if in_raid {
            "副本中，等待回城事件"
        } else {
            "城镇，补一次回城自动备份"
        }
    );
    if !in_raid {
        let app2 = app.clone();
        let r2 = remote.clone();
        let p2 = profile.clone();
        std::thread::spawn(move || backup_on_raid_end(app2, r2, p2));
    }

    let app_clone = app.clone();
    let r_event = remote.clone();
    let p_event = profile.clone();

    let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
        let Ok(event) = res else { return };
        let relevant = event.paths.iter().any(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n == backup::RAID_SIGNAL)
                .unwrap_or(false)
        });
        if !relevant {
            return;
        }
        match event.kind {
            EventKind::Create(_) => {
                // 仅状态显示：前端提示"副本中"，不产生存档动作
                dlog!("[watcher] {p_event} 检测到副本开始（信号文件创建）");
                emit(&app_clone, "raid-start", p_event.clone());
            }
            EventKind::Remove(_) => {
                dlog!("[watcher] {p_event} 检测到副本结束（信号文件删除）→ 触发回城自动备份");
                emit(&app_clone, "raid-end", p_event.clone());
                // 独立线程执行备份（含 5 秒稳定等待，防复制撞上游戏写盘）；去重由 backup_on_raid_end 保证
                let app2 = app_clone.clone();
                let r2 = r_event.clone();
                let p2 = p_event.clone();
                std::thread::spawn(move || backup_on_raid_end(app2, r2, p2));
            }
            _ => {}
        }
    })?;

    // 非递归监听：副本信号文件与存档文件都在顶层；备份文件夹内部写入不触发事件
    watcher.watch(&remote.join(&profile), RecursiveMode::NonRecursive)?;
    Ok(watcher)
}

