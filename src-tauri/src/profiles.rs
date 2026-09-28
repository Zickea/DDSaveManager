//! 档案扫描：列出 remote 目录下 profile_0 ~ profile_8（官方 9 个存档位）。
use std::path::Path;

use crate::week;

pub fn list_profiles(remote: &Path) -> Vec<String> {
    let mut out: Vec<(u32, String)> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(remote) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if e.path().is_dir() && name.starts_with("profile_") {
                if let Some(num) = name.strip_prefix("profile_").and_then(|s| s.parse::<u32>().ok()) {
                    // 官方存档位仅 profile_0 ~ profile_8；profile_9 及更高并非存档目录
                    if num <= 8 {
                        out.push((num, name));
                    }
                }
            }
        }
    }
    out.sort_by_key(|(num, _)| *num);
    out.into_iter().map(|(_, name)| name).collect()
}

/// 读取某档案当前的周数（玩家视角：total_weeks - 1，第0周=教学关）。
pub fn current_week(remote: &Path, profile: &str) -> Option<u32> {
    week::read_total_weeks(&remote.join(profile).join("persist.campaign_log.json"))
}
