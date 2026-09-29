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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_log_with_total(tag: &str, total: u32) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("ddsl_test_week_{tag}_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("persist.campaign_log.json");
        // 真实布局：total_weeks 键结束@11（mod3）→ pad1 → 4 字节小端值
        let mut data = b"total_weeks\x00".to_vec();
        data.extend_from_slice(&total.to_le_bytes());
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(&data).unwrap();
        path
    }

    #[test]
    fn total_weeks_minus_one() {
        let p = temp_log_with_total("mid", 3);
        assert_eq!(read_total_weeks(&p), Some(2)); // 存档 3 → 玩家视角第 2 周
    }

    #[test]
    fn total_weeks_first_town() {
        let p = temp_log_with_total("first", 1);
        assert_eq!(read_total_weeks(&p), Some(0)); // 回城后第一周 = 第 0 周
    }

    #[test]
    fn missing_file_returns_none() {
        let p = std::env::temp_dir().join("ddsl_test_week_absent_campaign.json");
        assert_eq!(read_total_weeks(&p), None);
    }
}
