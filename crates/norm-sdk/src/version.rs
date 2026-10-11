use crate::model::FirmwareVersion;

const STOCK_APOLLO_TAGS: [&str; 2] = ["B1.0", "B0.1"];

/// Splits the 20-character version string (`A0.2R0.4T0.0H0.0B0.1`) into its
/// five 4-character segments. Returns `None` for any other shape.
#[uniffi::export]
pub fn parse_firmware_version(raw: String) -> Option<FirmwareVersion> {
    let trimmed: String = raw.chars().filter(|c| !c.is_control()).collect();
    if trimmed.len() != 20
        || !trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.')
    {
        return None;
    }
    let segments: Vec<String> = (0..5)
        .map(|i| trimmed[i * 4..i * 4 + 4].to_string())
        .collect();
    let apollo_tag = segments.get(4).cloned();
    let is_patched = apollo_tag
        .as_deref()
        .is_some_and(|t| !STOCK_APOLLO_TAGS.contains(&t));
    Some(FirmwareVersion {
        raw: trimmed,
        segments,
        apollo_tag,
        is_patched,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stock_and_patched_tags() {
        let stock = parse_firmware_version("A0.2R0.4T0.0H0.0B0.1".into()).unwrap();
        assert_eq!(stock.segments[1], "R0.4");
        assert!(!stock.is_patched);
        assert!(
            parse_firmware_version("A0.2R0.4T2.3H0.6B0.P".into())
                .unwrap()
                .is_patched
        );
        assert!(parse_firmware_version("short".into()).is_none());
    }
}
