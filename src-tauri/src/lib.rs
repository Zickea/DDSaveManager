//! 暗黑地牢1 存档管家：Tauri 2 应用主入口。
mod backup;
mod dson;
mod logger;
mod paths;
mod profiles;
mod settings;
mod watcher;
mod week;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, WindowEvent};

/// 统一日志：stderr + 落盘（%APPDATA%\com.ddsl.savemanager\dd-save-manager.log）。
/// 正式版（windows_subsystem）无控制台，stderr 被静默丢弃，必须依赖文件日志。
#[macro_export]
macro_rules! dlog {
    ($($arg:tt)*) => {
        $crate::logger::log(&format!($($arg)*))
    };
}

/// 退出标志：置位后允许窗口关闭并退出进程（托盘「退出」菜单置位后 app.exit）。
static EXITING: AtomicBool = AtomicBool::new(false);

pub struct AppState {
    pub remote_dir: Option<PathBuf>,
    pub watchers: Mutex<HashMap<String, notify::RecommendedWatcher>>,
    pub settings: Mutex<settings::Settings>,
}

impl AppState {
    fn new() -> Self {
        Self {
            remote_dir: paths::locate_remote_dir(),
            watchers: Mutex::new(HashMap::new()),
            settings: Mutex::new(settings::load()),
        }
    }
}

#[derive(serde::Serialize)]
struct ProfileInfo {
    name: String,
    week: Option<u32>,
    #[serde(flatten)]
    meta: profiles::ProfileMeta,
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
            meta: profiles::profile_meta(remote, &name),
            name,
        })
        .collect()
}

#[tauri::command]
async fn manual_backup(
    state: tauri::State<'_, AppState>,
    profile: String,
    note: Option<String>,
) -> Result<backup::BackupEntry, String> {
    let remote = state
        .remote_dir
        .clone()
        .ok_or_else(|| "未找到暗黑地牢存档目录，请确认游戏已安装并运行过一次".to_string())?;
    // 副本（未结算）状态下禁止备份：备份的只是副本中途进度，且会与 Steam 云台账不一致
    if backup::is_in_raid(&remote, &profile) {
        return Err("当前档案处于副本（未结算）状态：请先回城（完成或撤退副本）后再手动备份。".into());
    }
    // 大目录复制放后台线程，避免阻塞主线程导致界面卡死
    tauri::async_runtime::spawn_blocking(move || {
        backup::backup_profile(&remote, &profile, "manual", note.as_deref())
    })
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
    // 大目录复制放后台线程，避免阻塞主线程导致界面卡死。
    // 副本残留状态（游戏未运行）允许恢复：实测恢复后完全退出 Steam 客户端再启动游戏可正常加载
    // （游戏运行中的拦截在 restore_profile 内由 game_running 负责）。
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
                crate::dlog!("watcher {profile} 启动失败: {e}");
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

/// 通过 Steam 协议拉起《暗黑地牢》（AppID 262060），Steam 未运行时也会先启动 Steam。
/// 不直接启动 Darkest.exe：避免绕过 Steam 层（DLC/成就/云同步/DRM 校验）。
#[tauri::command]
fn launch_game() -> Result<(), String> {
    if backup::game_running() {
        return Err("游戏已在运行中".into());
    }
    launch_via_steam()
}

#[cfg(windows)]
fn launch_via_steam() -> Result<(), String> {
    // explorer 会把 steam:// 协议 URI 交给系统注册的处理器（Steam 客户端）
    std::process::Command::new("explorer")
        .arg("steam://rungameid/262060")
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("启动游戏失败: {e}"))
}

#[cfg(not(windows))]
fn launch_via_steam() -> Result<(), String> {
    Err("仅支持 Windows".into())
}

#[tauri::command]
fn get_settings(state: tauri::State<AppState>) -> settings::Settings {
    state
        .settings
        .lock()
        .map(|s| s.clone())
        .unwrap_or_default()
}

#[tauri::command]
fn set_settings(
    state: tauri::State<AppState>,
    settings: settings::Settings,
) -> Result<(), String> {
    // 先应用开机自启（注册表操作），失败则不保存，避免设置与系统状态不一致
    settings::apply_auto_start(settings.auto_start)?;
    *state
        .settings
        .lock()
        .map_err(|_| "设置锁占用".to_string())? = settings.clone();
    settings::save(&settings)
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

/// 创建系统托盘：左键单击显示主窗口，右键菜单「显示主窗口 / 退出」。
/// 关闭主窗口后应用不退出（见 run() 的 on_window_event），常驻托盘继续监控。
fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let show_item = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

    let mut builder = TrayIconBuilder::with_id("main-tray")
        .tooltip("暗黑地牢 存档管家")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => show_main_window(app),
            "quit" => {
                EXITING.store(true, Ordering::SeqCst);
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });
    // 显式使用应用图标（TrayIconBuilder 不会自动取 default_window_icon）
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    let tray = builder.build(app)?;
    // 持有 TrayIcon 实例，防止 drop 后托盘从系统移除
    app.manage(tray);
    Ok(())
}

fn show_main_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

pub fn run() {
    tauri::Builder::default()
        .manage(AppState::new())
        // 单实例：重复启动时唤醒已有实例的主窗口，不创建第二个进程
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            show_main_window(app);
        }))
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
            open_remote_dir,
            launch_game,
            get_settings,
            set_settings
        ])
        .on_window_event(|window, event| {
            // 关闭主窗口 = 隐藏到托盘继续运行；仅当托盘「退出」置位时才真正关闭
            if let WindowEvent::CloseRequested { api, .. } = event {
                if !EXITING.load(Ordering::SeqCst) {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .setup(|app| {
            // 日志落盘初始化（必须在任何日志输出前调用）
            logger::init();
            // 系统托盘（关闭窗口后常驻后台）
            if let Err(e) = setup_tray(app) {
                crate::dlog!("托盘初始化失败: {e}");
            }
            // 应用启动即自动开启监控（用户无需手动点按钮）
            let handle = app.handle().clone();
            let state = app.state::<AppState>();
            let _ = start_all_watchers(handle, &state);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
