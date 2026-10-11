//! GATT layout and advertising names. The transport itself (CoreBluetooth,
//! Android BLE, bleak, btleplug…) stays in the host app.

/// Every UUID the watch exposes, as 128-bit strings (16-bit SIG UUIDs are
/// given in their short form, which every BLE stack accepts).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct GattProfile {
    /// Vendor service holding the command characteristics.
    pub primary_service: String,
    /// Extended vendor service; some builds put the remote channel here.
    pub extend_service: String,
    /// Apollo OTA (DFU) service.
    pub ota_service: String,
    /// Nordic-style OTA service UUID that also appears in the apps.
    pub ota_service_nordic: String,
    /// Standard Heart Rate service.
    pub heart_rate_service: String,
    /// Phone → watch commands (write).
    pub command_write: String,
    /// Watch → phone replies (notify).
    pub command_notify: String,
    /// Phone → watch replies to remote events (write).
    pub remote_write: String,
    /// Watch → phone remote events (notify).
    pub remote_notify: String,
    /// Watch → phone transparent channel (notify).
    pub transparent_notify: String,
    /// OTA control point (write without response + notify).
    pub ota_control: String,
    /// OTA data (write without response).
    pub ota_data: String,
    /// Standard Heart Rate Measurement (notify).
    pub heart_rate_measurement: String,
    /// Advertised name prefixes that identify a Norm Two.
    pub device_name_prefixes: Vec<String>,
}

fn vendor(short: &str) -> String {
    format!("0000{short}-0000-1000-8000-00805F9B34FB")
}

/// The watch's GATT profile.
#[uniffi::export]
pub fn gatt_profile() -> GattProfile {
    GattProfile {
        primary_service: vendor("6006"),
        extend_service: vendor("7006"),
        ota_service: vendor("1530"),
        ota_service_nordic: "00001530-1212-EFDE-1523-785FEABCD123".into(),
        heart_rate_service: "180D".into(),
        command_write: vendor("8001"),
        command_notify: vendor("8002"),
        remote_write: vendor("8003"),
        remote_notify: vendor("8004"),
        transparent_notify: vendor("8005"),
        ota_control: vendor("1531"),
        ota_data: vendor("1532"),
        heart_rate_measurement: "2A37".into(),
        device_name_prefixes: DEVICE_NAME_PREFIXES.iter().map(|s| s.to_string()).collect(),
    }
}

const DEVICE_NAME_PREFIXES: [&str; 5] =
    ["Norm2#", "Norm2_", "Norm2", "Festina", "Allview Hybrid T"];

/// True when an advertised name belongs to a Norm Two (or a rebadged P03B).
#[uniffi::export]
pub fn is_norm_device_name(name: Option<String>) -> bool {
    name.is_some_and(|n| DEVICE_NAME_PREFIXES.iter().any(|p| n.starts_with(p)))
}

/// Parses a standard Heart Rate Measurement (`2A37`) notification into bpm.
#[uniffi::export]
pub fn parse_heart_rate_measurement(data: Vec<u8>) -> Option<u16> {
    if data.len() < 2 {
        return None;
    }
    if data[0] & 0x01 == 0 {
        Some(u16::from(data[1]))
    } else if data.len() >= 3 {
        Some(u16::from(data[1]) | (u16::from(data[2]) << 8))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_heart_rate() {
        assert!(is_norm_device_name(Some("Norm2#00000".into())));
        assert!(!is_norm_device_name(Some("Pixel Watch".into())));
        assert!(!is_norm_device_name(None));
        assert_eq!(parse_heart_rate_measurement(vec![0x00, 72]), Some(72));
        assert_eq!(
            parse_heart_rate_measurement(vec![0x01, 0x2C, 0x01]),
            Some(300)
        );
        assert_eq!(parse_heart_rate_measurement(vec![0x01, 0x2C]), None);
    }
}
