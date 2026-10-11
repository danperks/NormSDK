//! Classification of every frame the watch sends, including events it starts
//! itself (find phone, media keys, phone-GPS workouts, live heart rate).

use crate::bytes::{le_u32, le_u64};
use crate::frame::{Action, Channel, FrameFormat, Packet};
use crate::model::{BatteryStatus, GpsConfirmations, GpsFix, ReminderKind};

/// Media keys from the watch's music page (6F `D0 71 [n]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum MusicControl {
    Play,
    Pause,
    Next,
    Previous,
    VolumeUp,
    VolumeDown,
    Other { raw: u8 },
}

/// The legacy `0xD9` device-control byte map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum DeviceControl {
    FindPhone,
    TakePhoto,
    StopFindPhone,
    MusicPlay,
    MusicPause,
    MusicNext,
    MusicPrevious,
    VolumeUp,
    VolumeDown,
    Other { raw: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum GpsDirection {
    /// The watch asking the phone (action `0x70`).
    WatchRequest,
    /// A phone reply echoed back (any other action).
    PhoneResponse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum GpsEventKind {
    Prepare,
    Start,
    Pause,
    Resume,
    Progress,
    Stop,
    LocationRequest,
    PhoneStatus,
    PhoneDistance,
    PhoneLocation,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum GpsAvailability {
    Available,
    Unavailable,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct GpsTlv {
    pub tag: u8,
    pub value: Vec<u8>,
}

/// A phone-GPS workout frame (`0xE9`, remote channel).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PhoneGpsEvent {
    pub direction: GpsDirection,
    pub kind: GpsEventKind,
    /// Tag 0 of a watch request: 0 start, 1 pause, 2 resume, 3 stop,
    /// 4 progress, 5 prepare, 6 location request.
    pub request_type: Option<u8>,
    pub gps_availability: Option<GpsAvailability>,
    pub start_timestamp: Option<i64>,
    pub pause_timestamp: Option<i64>,
    pub resume_timestamp: Option<i64>,
    pub stop_timestamp: Option<i64>,
    pub distance_m: Option<i64>,
    pub speed_kmh: Option<i64>,
    pub confirmations: Option<GpsConfirmations>,
    pub location: Option<GpsFix>,
    pub unknown_tlvs: Vec<GpsTlv>,
    pub raw: Vec<u8>,
}

/// What a received frame means.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum WatchMessage {
    /// Generic ack `01 81 [command, status]`.
    Ack {
        command: u8,
        status: u8,
    },
    /// Battery reply (`0x08`), new-mode encoding.
    Battery {
        status: BatteryStatus,
    },
    /// Patched battery stream (`E4 71 [level, state]`); state 2 = on charger.
    BatteryStream {
        level: u8,
        on_charger: bool,
    },
    /// Find phone (`D2 71 [00 start | 01 stop]`). Reply to a start with
    /// `FindPhoneStartAck` on the remote channel.
    FindPhone {
        ringing: bool,
    },
    MusicControl {
        action: MusicControl,
    },
    DeviceControl {
        action: DeviceControl,
    },
    PhoneGps {
        event: PhoneGpsEvent,
    },
    /// Patched live workout heart rate (`E2 71 [bpm (cal_milli_le32)]`).
    LiveHeartRate {
        bpm: u8,
        calories_kcal: Option<u32>,
    },
    /// Real-time heart rate (`0x5D`).
    RealTimeHeartRate {
        bpm: u8,
    },
    /// A reminder sent by the watch after `GetReminders6e` (6E `0x11`).
    Reminder6e {
        kind: ReminderKind,
        hour: u8,
        minute: u8,
        weekdays: u8,
    },
    /// Anything else, usually a reply to a request.
    Reply {
        command: u8,
    },
}

/// Classifies a frame without any request context.
#[uniffi::export]
pub fn classify_packet(packet: Packet) -> WatchMessage {
    classify(&packet)
}

pub(crate) fn classify(packet: &Packet) -> WatchMessage {
    let d = packet.payload.as_slice();
    if let Some((command, status)) = packet.ack() {
        return WatchMessage::Ack { command, status };
    }
    if packet.format == FrameFormat::Frame6e {
        if packet.command == 0x11 && d.len() >= 4 {
            return WatchMessage::Reminder6e {
                kind: ReminderKind::from_byte_6e(d[0]),
                hour: d[1],
                minute: d[2],
                weekdays: d[3],
            };
        }
        return WatchMessage::Reply {
            command: packet.command,
        };
    }
    match (packet.command, d) {
        (0x08, [b, ..]) => WatchMessage::Battery {
            status: BatteryStatus {
                level: b & 0x7F,
                charging: b & 0x80 != 0,
            },
        },
        (0xE4, [level, state, ..]) => WatchMessage::BatteryStream {
            level: *level,
            on_charger: *state == 2,
        },
        (0xD2, [sub, ..]) if packet.action == Action::Set && *sub <= 1 => {
            WatchMessage::FindPhone { ringing: *sub == 0 }
        }
        (0xD0, [sub, ..]) if packet.action == Action::Set => WatchMessage::MusicControl {
            action: match sub {
                1 => MusicControl::Play,
                2 => MusicControl::Pause,
                3 => MusicControl::Next,
                4 => MusicControl::Previous,
                5 => MusicControl::VolumeUp,
                6 => MusicControl::VolumeDown,
                raw => MusicControl::Other { raw: *raw },
            },
        },
        (0xD9, [b, ..]) => WatchMessage::DeviceControl {
            action: match b {
                1 => DeviceControl::FindPhone,
                2 => DeviceControl::TakePhoto,
                3 => DeviceControl::StopFindPhone,
                4 => DeviceControl::MusicPlay,
                5 => DeviceControl::MusicPause,
                6 => DeviceControl::MusicNext,
                7 => DeviceControl::MusicPrevious,
                8 => DeviceControl::VolumeUp,
                9 => DeviceControl::VolumeDown,
                raw => DeviceControl::Other { raw: *raw },
            },
        },
        (0xE9, _) => WatchMessage::PhoneGps {
            event: parse_phone_gps(d, packet.action),
        },
        (0xE2, [bpm, ..]) if packet.channel == Channel::Remote || packet.action == Action::Set => {
            WatchMessage::LiveHeartRate {
                bpm: *bpm,
                calories_kcal: (d.len() >= 5).then(|| le_u32(d, 1, 4) / 1000),
            }
        }
        (0x5D, [bpm, ..]) => WatchMessage::RealTimeHeartRate { bpm: *bpm },
        (command, _) => WatchMessage::Reply { command },
    }
}

fn tlvs(payload: &[u8]) -> Vec<GpsTlv> {
    let mut out = Vec::new();
    let mut offset = 0;
    while offset + 2 <= payload.len() {
        let tag = payload[offset];
        let len = usize::from(payload[offset + 1]);
        offset += 2;
        if offset + len > payload.len() {
            out.push(GpsTlv {
                tag,
                value: payload[offset..].to_vec(),
            });
            return out;
        }
        out.push(GpsTlv {
            tag,
            value: payload[offset..offset + len].to_vec(),
        });
        offset += len;
    }
    if offset < payload.len() {
        out.push(GpsTlv {
            tag: 0xFF,
            value: payload[offset..].to_vec(),
        });
    }
    out
}

fn parse_phone_gps(payload: &[u8], action: Action) -> PhoneGpsEvent {
    let tlvs = tlvs(payload);
    let first = |tag: u8| {
        tlvs.iter()
            .find(|t| t.tag == tag)
            .map(|t| t.value.as_slice())
    };
    let int = |tag: u8| {
        first(tag)
            .filter(|v| !v.is_empty())
            .map(|v| le_u64(v, 0, v.len()) as i64)
    };
    let unknown_tlvs: Vec<GpsTlv> = tlvs.iter().filter(|t| t.tag > 4).cloned().collect();
    let tag0 = first(0);
    let request_type = tag0.and_then(|v| v.first().copied());

    let mut event = PhoneGpsEvent {
        direction: GpsDirection::WatchRequest,
        kind: GpsEventKind::Unknown,
        request_type: None,
        gps_availability: None,
        start_timestamp: None,
        pause_timestamp: None,
        resume_timestamp: None,
        stop_timestamp: None,
        distance_m: None,
        speed_kmh: None,
        confirmations: None,
        location: None,
        unknown_tlvs,
        raw: payload.to_vec(),
    };

    if action == Action::Check {
        event.request_type = request_type;
        event.kind = match request_type {
            Some(0) => GpsEventKind::Start,
            Some(1) => GpsEventKind::Pause,
            Some(2) => GpsEventKind::Resume,
            Some(3) => GpsEventKind::Stop,
            Some(4) => GpsEventKind::Progress,
            Some(5) => GpsEventKind::Prepare,
            Some(6) => GpsEventKind::LocationRequest,
            _ => GpsEventKind::Unknown,
        };
        event.start_timestamp = int(1);
        event.pause_timestamp = int(2);
        event.resume_timestamp = int(3);
        event.stop_timestamp = int(4);
        return event;
    }

    event.direction = GpsDirection::PhoneResponse;
    event.gps_availability = tag0.and_then(|v| match v.first() {
        Some(0) => Some(GpsAvailability::Available),
        Some(1) => Some(GpsAvailability::Unavailable),
        Some(_) => Some(GpsAvailability::Unknown),
        None => None,
    });
    event.distance_m = int(1);
    event.speed_kmh = int(2);
    event.confirmations = first(3).filter(|v| v.len() >= 4).map(|v| GpsConfirmations {
        start: v[0] == 0,
        pause: v[1] == 0,
        resume: v[2] == 0,
        stop: v[3] == 0,
    });
    event.location = first(4).filter(|v| v.len() >= 20).map(|v| GpsFix {
        timestamp: le_u64(v, 0, 4) as i64,
        latitude_e7: le_u64(v, 4, 4) as i64,
        longitude_e7: le_u64(v, 8, 4) as i64,
        altitude_cm: le_u64(v, 12, 4) as i64,
        accuracy_cm: le_u64(v, 16, 4) as i64,
    });
    event.kind = if event.location.is_some() {
        GpsEventKind::PhoneLocation
    } else if event.distance_m.is_some()
        || event.speed_kmh.is_some()
        || event.confirmations.is_some()
    {
        GpsEventKind::PhoneDistance
    } else if event.gps_availability.is_some() {
        GpsEventKind::PhoneStatus
    } else {
        GpsEventKind::Unknown
    };
    event
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::Reassembler;
    use crate::request::Request;

    fn packet(command: u8, action: Action, payload: &[u8], channel: Channel) -> Packet {
        Packet {
            format: FrameFormat::Frame6f,
            command,
            action,
            payload: payload.to_vec(),
            channel,
        }
    }

    #[test]
    fn watch_initiated_events() {
        assert_eq!(
            classify(&packet(0xD2, Action::Set, &[0x00], Channel::Primary)),
            WatchMessage::FindPhone { ringing: true }
        );
        assert_eq!(
            classify(&packet(0xD0, Action::Set, &[0x03], Channel::Primary)),
            WatchMessage::MusicControl {
                action: MusicControl::Next
            }
        );
        assert_eq!(
            classify(&packet(
                0xE2,
                Action::Set,
                &[90, 0x10, 0x27, 0, 0],
                Channel::Remote
            )),
            WatchMessage::LiveHeartRate {
                bpm: 90,
                calories_kcal: Some(10)
            }
        );
        assert_eq!(
            classify(&packet(0xE4, Action::Set, &[55, 2], Channel::Primary)),
            WatchMessage::BatteryStream {
                level: 55,
                on_charger: true
            }
        );
    }

    #[test]
    fn phone_gps_round_trip() {
        let request = packet(
            0xE9,
            Action::Check,
            &[0x00, 0x01, 0x00, 0x01, 0x04, 0x10, 0x20, 0x30, 0x40],
            Channel::Remote,
        );
        let WatchMessage::PhoneGps { event } = classify(&request) else {
            panic!()
        };
        assert_eq!(event.kind, GpsEventKind::Start);
        assert_eq!(event.start_timestamp, Some(0x40302010));

        let frame = Request::PhoneGpsLocation {
            fix: GpsFix {
                timestamp: 1,
                latitude_e7: 515_074_000,
                longitude_e7: 2,
                altitude_cm: 3,
                accuracy_cm: 4,
            },
        }
        .frames()
        .remove(0);
        let echoed = Reassembler::default()
            .feed(Channel::Remote, &frame.data)
            .remove(0);
        let WatchMessage::PhoneGps { event } = classify(&echoed) else {
            panic!()
        };
        assert_eq!(event.kind, GpsEventKind::PhoneLocation);
        assert_eq!(event.location.unwrap().latitude_e7, 515_074_000);
    }
}
