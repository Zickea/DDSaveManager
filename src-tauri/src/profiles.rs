//! 档案扫描：列出 remote 目录下 profile_0 ~ profile_8（官方 9 个存档位）。
use std::path::Path;

use crate::{dson, week};

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

/// 档案附加信息（从 dson 存档提取，供侧栏卡片展示）。
#[derive(serde::Serialize, Clone, Default)]
pub struct ProfileMeta {
    /// 庄园名（persist.game.json 的 estatename）
    pub estate_name: Option<String>,
    /// 游戏模式（persist.game.json 的 game_mode：bloodmoon / new_game_plus / ...）
    pub game_mode: Option<String>,
    /// 当前英雄数（persist.roster.json 中 hero_file_data 出现次数）
    pub hero_count: Option<u32>,
    /// 离队（死亡/解雇）英雄数（persist.roster.json 的 dismissed_hero_count）
    pub dead_count: Option<u32>,
}

/// 读取某档案的附加信息（各文件缺失时相应字段为 None，不阻塞）。
pub fn profile_meta(remote: &Path, profile: &str) -> ProfileMeta {
    let dir = remote.join(profile);
    let game = dir.join("persist.game.json");
    let roster = dir.join("persist.roster.json");
    ProfileMeta {
        estate_name: dson::file_string(&game, b"estatename"),
        game_mode: dson::file_string(&game, b"game_mode"),
        hero_count: std::fs::read(&roster).ok().map(|d| dson::count_subslice(&d, b"hero_file_data") as u32),
        dead_count: dson::file_u32(&roster, b"dismissed_hero_count"),
    }
}
