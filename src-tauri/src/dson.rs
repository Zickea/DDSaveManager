//! dson 二进制存档的最小提取工具（暗黑地牢自定义序列化格式）。
//! 事实依据（多次真实存档实测校准，2026-09-29 用六个字段样本验证）：
//! - 键名为 ASCII 明文存储，可全文检索定位
//! - 字段布局：键 + 对齐填充 \x00 + 值；**值（含字符串长度字段）起始位置 4 字节对齐**，
//!   填充数 = 键结束位置补到 4 边界所需 \x00 数；若键结束已在 4 边界则仍填 4 个 \x00。
//!   实测：total_weeks 键结束@3963(mod3)→pad1；game_mode@1896(mod0)→pad4；
//!   nextGuid@512(mod0)→pad4；dismissed_hero_count@540(mod0)→pad4（值=0 时再跟 4 个 \x00）
//! - u32 值：填充后 4 字节小端整数
//! - 字符串值：填充后 4 字节小端长度 + UTF-8 内容（可能带尾 \x00）
use std::path::Path;

/// 在字节流中查找子串，返回起始下标。
pub fn find_subslice(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// 统计字节流中子串出现次数。
pub fn count_subslice(hay: &[u8], needle: &[u8]) -> usize {
    if needle.is_empty() {
        return 0;
    }
    let mut n = 0;
    let mut from = 0;
    while let Some(i) = hay[from..].windows(needle.len()).position(|w| w == needle) {
        n += 1;
        from += i + needle.len();
    }
    n
}

/// 键后对齐填充数：值起始位置 4 字节对齐，且键后至少 1 个 \x00。
fn pad_to_align(key_end: usize) -> usize {
    match key_end % 4 {
        0 => 4,
        r => 4 - r,
    }
}

/// 读取指定键后的 u32 值：键 + 对齐填充 + 4 字节小端整数。
pub fn read_u32_after_key(data: &[u8], key: &[u8]) -> Option<u32> {
    let idx = find_subslice(data, key)?;
    let val_start = idx + key.len() + pad_to_align(idx + key.len());
    let bytes = data.get(val_start..val_start + 4)?;
    Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

/// 读取指定键后的字符串值：键 + 对齐填充 + 4 字节小端长度 + UTF-8 内容。
/// 内容末尾的 \x00 与空白会被去除。
pub fn read_string_after_key(data: &[u8], key: &[u8]) -> Option<String> {
    let idx = find_subslice(data, key)?;
    let len_start = idx + key.len() + pad_to_align(idx + key.len());
    let len_bytes = data.get(len_start..len_start + 4)?;
    let len = u32::from_le_bytes([len_bytes[0], len_bytes[1], len_bytes[2], len_bytes[3]]) as usize;
    let content = data.get(len_start + 4..len_start + 4 + len)?;
    let mut end = content.len();
    while end > 0 && (content[end - 1] == 0 || content[end - 1] == b' ') {
        end -= 1;
    }
    Some(String::from_utf8_lossy(&content[..end]).into_owned())
}

/// 读取单个存档文件中指定键后的 u32（文件缺失/键缺失返回 None）。
pub fn file_u32(path: &Path, key: &[u8]) -> Option<u32> {
    let data = std::fs::read(path).ok()?;
    read_u32_after_key(&data, key)
}

/// 读取单个存档文件中指定键后的字符串。
pub fn file_string(path: &Path, key: &[u8]) -> Option<String> {
    let data = std::fs::read(path).ok()?;
    read_string_after_key(&data, key)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 键结束 @12（mod4=0）→ pad4 → 值=23（模拟 nextGuid/game_mode 布局）
    #[test]
    fn u32_key_end_aligned() {
        let data = b"abcdnextGuid\x00\x00\x00\x00\x17\x00\x00\x00yyy";
        // nextGuid@4, key_end=12 (mod4=0) → pad=4 → val@16 = 0x17
        assert_eq!(read_u32_after_key(data, b"nextGuid"), Some(23));
    }

    /// 键结束 @9（mod4=1）→ pad3 → 值=2
    #[test]
    fn u32_key_end_misaligned() {
        let data = b"xnextGuid\x00\x00\x00\x02\x00\x00\x00yyy";
        // nextGuid@1, key_end=9 (mod4=1) → pad=3 → val@12 = 2
        assert_eq!(read_u32_after_key(data, b"nextGuid"), Some(2));
    }

    /// 值=0：pad4 后值仍为 0（共 8 个 \x00，模拟 dismissed_hero_count=0）
    #[test]
    fn u32_zero_value() {
        let data = b"dismissed_hero_count\x00\x00\x00\x00\x00\x00\x00\x00heroes";
        // key_end=20 (mod4=0) → pad=4 → val@24 = 0
        assert_eq!(read_u32_after_key(data, b"dismissed_hero_count"), Some(0));
    }

    /// 字符串：estatename 布局（键结束 mod3 → pad1 → 长度7 → 极暗+00）
    #[test]
    fn string_with_utf8() {
        let mut data = b"xestatename\x00\x07\x00\x00\x00".to_vec();
        // estatename@1, key_end=11 (mod4=3) → pad=1 → len@12 = 7
        data.extend_from_slice("\u{6781}\u{6697}".as_bytes()); // 极暗 UTF-8 = 6 字节
        data.push(0);
        data.extend_from_slice(b"next");
        assert_eq!(
            read_string_after_key(&data, b"estatename").as_deref(),
            Some("\u{6781}\u{6697}")
        );
    }

    /// 字符串：game_mode 布局（键结束 mod0 → pad4 → 长度10 → bloodmoon）
    #[test]
    fn string_key_end_aligned() {
        let mut data = b"xyzgame_mode\x00\x00\x00\x00\x0a\x00\x00\x00".to_vec();
        // game_mode@3, key_end=12 (mod4=0) → pad=4 → len@16 = 10
        data.extend_from_slice(b"bloodmoon\x00");
        data.extend_from_slice(b"next");
        assert_eq!(
            read_string_after_key(&data, b"game_mode").as_deref(),
            Some("bloodmoon")
        );
    }

    #[test]
    fn count_occurrences() {
        let data = b"hero_file_data xx hero_file_data hero_file_data";
        assert_eq!(count_subslice(data, b"hero_file_data"), 3);
    }
}
