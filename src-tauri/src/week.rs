//! 周数读取：从 persist.campaign_log.json（二进制 dson）中解析 total_weeks。
//! 注意语义：total_weeks 是存档里的"下一周编号"——回城（副本结束）时它已经递增；
//! 玩家视角的"当前周 / 进副本那一周" = total_weeks - 1（第 0 周 = 教学关/老路）。
//! 因此本函数统一返回 total_weeks - 1，侧栏显示、备份命名 weekXX、按周去重全部一致。
use std::path::Path;

use crate::dson;

/// 从 campaign_log 存档文件读取玩家视角的当前周数（total_weeks - 1）。
pub fn read_total_weeks(campaign_log: &Path) -> Option<u32> {
    let raw = dson::file_u32(campaign_log, b"total_weeks")?;
    Some(raw.saturating_sub(1))
}
