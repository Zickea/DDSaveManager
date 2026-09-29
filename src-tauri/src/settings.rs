//! 应用设置持久化与开机自启。
//! 设置保存为 JSON（%APPDATA%\com.ddsl.savemanager\settings.json），
//! 开机自启通过写入 HKCU\...\CurrentVersion\Run 注册表实现（Windows）。
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// 应用设置（新增字段时保持 serde(default) 兼容旧配置文件）。
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
    /// 开机自启（写入注册表 Run 键）
    pub auto_start: bool,
    /// 自动备份保留策略：保留最近 N 周的 auto 档（0 = 保留全部）
    pub keep_auto_weeks: u32,
    /// 恢复前显示"回退 N 周"提示
    pub confirm_rollback: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            auto_start: false,
            keep_auto_weeks: 0,
            confirm_rollback: true,
        }
    }
}

/// 设置文件路径：%APPDATA%\com.ddsl.savemanager\settings.json
fn settings_path() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA")?;
    Some(
        PathBuf::from(base)
            .join("com.ddsl.savemanager")
            .join("settings.json"),
    )
}

/// 读取设置；文件缺失或损坏时回退默认值。
pub fn load() -> Settings {
    settings_path()
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// 保存设置（自动创建目录）。
pub fn save(s: &Settings) -> Result<(), String> {
    let Some(p) = settings_path() else {
        return Err("无法定位 APPDATA 目录".into());
    };
    if let Some(dir) = p.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("创建设置目录失败: {e}"))?;
    }
    let json = serde_json::to_string_pretty(s).map_err(|e| e.to_string())?;
    fs::write(&p, json).map_err(|e| format!("保存设置失败: {e}"))
}

/// 开机自启：启用时写入 Run 键（引号包裹 exe 路径），关闭时删除该值。
pub fn apply_auto_start(enabled: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        use winreg::enums::{HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE};
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let key = hkcu
            .open_subkey_with_flags(
                r"Software\Microsoft\Windows\CurrentVersion\Run",
                KEY_QUERY_VALUE | KEY_SET_VALUE,
            )
            .map_err(|e| format!("打开注册表 Run 键失败: {e}"))?;
        if enabled {
            let exe = std::env::current_exe().map_err(|e| format!("获取程序路径失败: {e}"))?;
            key.set_value("DDSaveManager", &format!("\"{}\"", exe.to_string_lossy()))
                .map_err(|e| format!("写入开机自启失败: {e}"))?;
        } else {
            key.delete_value("DDSaveManager")
                .map_err(|e| format!("关闭开机自启失败: {e}"))?;
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = enabled;
        Err("开机自启仅支持 Windows".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_sane() {
        let s = Settings::default();
        assert!(!s.auto_start);
        assert_eq!(s.keep_auto_weeks, 0);
        assert!(s.confirm_rollback);
    }

    #[test]
    fn settings_json_roundtrip() {
        let s = Settings {
            auto_start: true,
            keep_auto_weeks: 10,
            confirm_rollback: false,
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert!(back.auto_start);
        assert_eq!(back.keep_auto_weeks, 10);
        assert!(!back.confirm_rollback);
    }
}
