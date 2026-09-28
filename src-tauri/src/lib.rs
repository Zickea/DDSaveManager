//! 暗黑地牢1 存档管家：Tauri 2 应用主入口。
mod backup;
mod paths;
mod profiles;
mod watcher;
mod week;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use tauri::{AppHandle, Manager};

pub struct AppState {
    pub remote_dir: Option<PathBuf>,
    pub watchers: Mutex<HashMap<String, notify::RecommendedWatcher>>,
}

impl AppState {
    fn new() -> Self {
        Self {
            remote_dir: paths::locate_remote_dir(),
            watchers: Mutex::new(HashMap::new()),
        }
    }
}

#[derive(serde::Serialize)]
struct ProfileInfo {
    name: String,
    week: Option<u32>,
}

#[derive(serde::Serialize)]
struct StatusInfo {
    remote_dir: Option<String>,
    game_running: bool,
    watching: Vec<String>,
}

#[tauri::command]
fn get_status(state: tauri::State<AppState>) -> StatusInfo {
    let watching = state
        .watchers
        .lock()
        .map(|w| w.keys().cloned().collect())
        .unwrap_or_default();
    StatusInfo {
        remote_dir: state.remote_dir.as_ref().map(|p| p.to_string_lossy().into_owned()),
        game_running: backup::game_running(),
        watching,
    }
}

#[tauri::command]
fn get_profiles(state: tauri::State<AppState>) -> Vec<ProfileInfo> {
    let Some(remote) = &state.remote_dir else {
        return Vec::new();
    };
    profiles::list_profiles(remote)
        .into_iter()
        .map(|name| ProfileInfo {
            week: profiles::current_week(remote, &name),
            name,
        })
        .collect()
}

#[tauri::command]
async fn manual_backup(
    state: tauri::State<'_, AppState>,
    profile: String,
) -> Result<backup::BackupEntry, String> {
    let remote = state
        .remote_dir
        .clone()
        .ok_or_else(|| "未找到暗黑地牢存档目录，请确认游戏已安装并运行过一次".to_string())?;
    // 大目录复制放后台线程，避免阻塞主线程导致界面卡死
    tauri::async_runtime::spawn_blocking(move || backup::backup_profile(&remote, &profile, "manual"))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
fn list_backups(state: tauri::State<AppState>, profile: String) -> Vec<backup::BackupEntry> {
    let Some(remote) = &state.remote_dir else {
        return Vec::new();
    };
    backup::list_backups(remote, &profile)
}

#[tauri::command]
async fn restore_backup(
    state: tauri::State<'_, AppState>,
    profile: String,
    backup_name: String,
) -> Result<backup::RestoreResult, String> {
    let remote = state
        .remote_dir
        .clone()
        .ok_or_else(|| "未找到暗黑地牢存档目录".to_string())?;
    // 大目录复制放后台线程，避免阻塞主线程导致界面卡死
    tauri::async_runtime::spawn_blocking(move || {
        backup::restore_profile(&remote, &profile, &backup_name)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn delete_backup(
    state: tauri::State<AppState>,
    profile: String,
    backup_name: String,
) -> Result<(), String> {
    let Some(remote) = &state.remote_dir else {
        return Err("未找到暗黑地牢存档目录".into());
    };
    backup::delete_backup(remote, &profile, &backup_name)
}

#[tauri::command]
fn clear_profile_backups(
    state: tauri::State<AppState>,
    profile: String,
) -> Result<(), String> {
    let Some(remote) = &state.remote_dir else {
        return Err("未找到暗黑地牢存档目录".into());
    };
    backup::clear_profile_backups(remote, &profile)
}

#[tauri::command]
fn delete_profile(
    state: tauri::State<AppState>,
    profile: String,
) -> Result<(), String> {
    let Some(remote) = &state.remote_dir else {
        return Err("未找到暗黑地牢存档目录".into());
    };
    backup::delete_profile(remote, &profile)
}

/// 为当前所有档案启动监控（幂等：已监控的跳过）。
fn start_all_watchers(app: AppHandle, state: &AppState) -> Result<Vec<String>, String> {
    let Some(remote) = &state.remote_dir else {
        return Err("未找到暗黑地牢存档目录".into());
    };
    let profiles = profiles::list_profiles(remote);
    let mut watching = Vec::new();
    let mut wmap = state.watchers.lock().map_err(|_| "状态锁占用")?;
    for profile in &profiles {
        if wmap.contains_key(profile) {
            continue;
        }
        match watcher::start_watcher(app.clone(), remote, &profile) {
            Ok(w) => {
                wmap.insert(profile.clone(), w);
                watching.push(profile.clone());
            }
            Err(e) => {
                eprintln!("watcher {profile} 启动失败: {e}");
            }
        }
    }
    Ok(watching)
}

#[tauri::command]
fn start_watchers(
    app: AppHandle,
    state: tauri::State<AppState>,
) -> Result<Vec<String>, String> {
    start_all_watchers(app, &state)
}

#[tauri::command]
fn stop_watchers(state: tauri::State<AppState>) -> Result<(), String> {
    let mut wmap = state.watchers.lock().map_err(|_| "状态锁占用")?;
    wmap.clear();
    Ok(())
}

/// 在文件资源管理器中打开存档根目录（不接收前端传入路径，只打开自身定位的 remote 目录，防注入）。
#[tauri::command]
fn open_remote_dir(state: tauri::State<AppState>) -> Result<(), String> {
    let Some(remote) = &state.remote_dir else {
        return Err("未找到暗黑地牢存档目录".into());
    };
    open_in_explorer(remote)
}

#[cfg(windows)]
fn open_in_explorer(path: &std::path::Path) -> Result<(), String> {
    std::process::Command::new("explorer")
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("打开目录失败: {e}"))
}

#[cfg(not(windows))]
fn open_in_explorer(_path: &std::path::Path) -> Result<(), String> {
    Err("仅支持 Windows".into())
}

pub fn run() {
    tauri::Builder::default()
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            get_status,
            get_profiles,
            manual_backup,
            list_backups,
            restore_backup,
            delete_backup,
            clear_profile_backups,
            delete_profile,
            start_watchers,
            stop_watchers,
            open_remote_dir
        ])
        .setup(|app| {
            // 应用启动即自动开启监控（用户无需手动点按钮）
            let handle = app.handle().clone();
            let state = app.state::<AppState>();
            let _ = start_all_watchers(handle, &state);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
