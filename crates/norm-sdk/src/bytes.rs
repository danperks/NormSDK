//! Little-endian helpers shared by the encoders and parsers.
//!
//! The readers mirror the original apps' `bytesToInt`: they read up to `count`
//! bytes and stop early at the end of the buffer instead of failing, which is
//! how short replies from older firmware are tolerated.

pub(crate) fn le_u64(data: &[u8], offset: usize, count: usize) -> u64 {
    let mut out = 0u64;
    let end = data.len().min(offset.saturating_add(count));
    for (i, byte) in data.get(offset..end).unwrap_or(&[]).iter().enumerate() {
        out |= u64::from(*byte) << (i * 8);
    }
    out
}

pub(crate) fn le_u32(data: &[u8], offset: usize, count: usize) -> u32 {
    le_u64(data, offset, count) as u32
}

pub(crate) fn le_u16(data: &[u8], offset: usize) -> u16 {
    le_u64(data, offset, 2) as u16
}

pub(crate) fn put_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

pub(crate) fn put_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

/// Truncates a string to at most `max` UTF-8 bytes without splitting a
/// character, so the watch never receives half a multi-byte sequence.
pub(crate) fn utf8_prefix(text: &str, max: usize) -> &[u8] {
    if text.len() <= max {
        return text.as_bytes();
    }
    let mut end = max;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    &text.as_bytes()[..end]
}

/// Same as [`utf8_prefix`], but limits by characters first (the original apps
/// used `String.prefix(n)` before converting to UTF-8).
pub(crate) fn utf8_char_prefix(text: &str, max_chars: usize) -> Vec<u8> {
    text.chars()
        .take(max_chars)
        .collect::<String>()
        .into_bytes()
}

/// A fixed-width, NUL-padded field that always keeps at least one NUL.
pub(crate) fn fixed_field(text: &str, width: usize) -> Vec<u8> {
    let mut out = utf8_prefix(text, width.saturating_sub(1)).to_vec();
    out.resize(width, 0);
    out
}

pub(crate) fn hex(data: &[u8]) -> String {
    data.iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_reads_stop_at_the_end() {
        assert_eq!(le_u64(&[0x01, 0x02], 0, 4), 0x0201);
        assert_eq!(le_u64(&[0x01, 0x02], 5, 4), 0);
    }

    #[test]
    fn utf8_prefix_keeps_whole_characters() {
        assert_eq!(utf8_prefix("héllo", 2), b"h");
        assert_eq!(fixed_field("abc", 3), b"ab\0");
    }
}
