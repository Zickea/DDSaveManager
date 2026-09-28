//! 周数读取：从 persist.campaign_log.json（二进制 dson）中解析 total_weeks。
//! 键名以明文存储，键后为 \x00 分隔/对齐填充 + 4 字节小端整数。
//! 已用三个真实存档验证：profile_0=24、profile_1=8、profile_2=2，
//! 且 total_weeks 即为游戏内当前周数（无 ±1 偏移；第 0 周=教学关/老路）。

/// 在字节流中查找子串，返回起始下标。
fn find_subslice(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// 从 campaign_log 存档文件读取 total_weeks（即游戏内当前周数）。
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
    if rest.len() >= 4 {
        Some(u32::from_le_bytes([rest[0], rest[1], rest[2], rest[3]]))
    } else {
        None
    }
}
