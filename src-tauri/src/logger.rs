//! 日志落盘：dev 模式下 eprintln 可见；正式版（windows_subsystem 无控制台）时
//! stderr 被静默丢弃，因此同时写入 %APPDATA%\com.ddsl.savemanager\dd-save-manager.log。
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

static LOG_FILE: Mutex<Option<std::fs::File>> = Mutex::new(None);

/// 初始化日志文件（追加模式）。失败时静默（stderr 输出不受影响）。
pub fn init() {
    let path = std::env::var_os("APPDATA").map(|base| {
        PathBuf::from(base).join("com.ddsl.savemanager").join("dd-save-manager.log")
    });
    let Some(path) = path else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(file) = OpenOptions::new().create(true).append(true).open(path) {
        if let Ok(mut guard) = LOG_FILE.lock() {
            *guard = Some(file);
        }
    }
}

/// 记录一条日志（带时间戳）：始终写 stderr（dev 可见），落盘失败不影响运行。
pub fn log(msg: &str) {
    let line = format!("[{}] {}", chrono::Local::now().format("%H:%M:%S"), msg);
    eprintln!("{line}");
    if let Ok(mut guard) = LOG_FILE.lock() {
        if let Some(f) = guard.as_mut() {
            let _ = writeln!(f, "{line}");
        }
    }
}
