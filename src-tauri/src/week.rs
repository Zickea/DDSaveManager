//! 周数读取：从 persist.campaign_log.json（二进制 dson）中解析 total_weeks。
//! 键名以明文存储，键后为 \x00 分隔/对齐填充 + 4 字节小端整数。
//! 注意语义：total_weeks 是存档里的"下一周编号"——回城（副本结束）时它已经递增；
//! 玩家视角的"当前周 / 进副本那一周" = total_weeks - 1（第 0 周 = 教学关/老路）。
//! 因此本函数统一返回 total_weeks - 1，侧栏显示、备份命名 weekXX、按周去重全部一致。

/// 在字节流中查找子串，返回起始下标。
fn find_subslice(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// 从 campaign_log 存档文件读取玩家视角的当前周数（total_weeks - 1）。
pub fn read_total_weeks(campaign_log: &std::path::Path) -> Option<u32> {
    let data = std::fs::read(campaign_log).ok()?;
    let key = b"total_weeks";
    let idx = find_subslice(&data, key)?;
    let mut rest = &data[idx + key.len()..];
    // 跳过键后的 \x00 分隔与对齐填充（值本身从第一个非零字节开始）
    while rest.first() == Some(&0) {
        rest = &rest[1..];
    }
    // 值为 4 字节小端整数
    let raw = if rest.len() >= 4 {
        u32::from_le_bytes([rest[0], rest[1], rest[2], rest[3]])
    } else {
        return None;
    };
    // 玩家视角周数 = 存档 total_weeks - 1（回城后已递增为下一周编号）
    Some(raw.saturating_sub(1))
}
