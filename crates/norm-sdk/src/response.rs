//! Typed replies and the per-request reply matcher.
//!
//! Matching rules (ported from the NormConnect command queue):
//! 1. A generic ack `01 81 [code, status]` for the awaited code completes the
//!    request with [`Response::Ack`]. Other short acks are ignored.
//! 2. Otherwise only frames with the awaited code (or an alternate) and the
//!    request's frame format are considered.
//! 3. Single-reply requests complete on the first such frame, even if it is
//!    too short to parse ([`Response::Unparsed`]), so nothing hangs.
//! 4. Record syncs collect frames until the expected count arrives.

use std::collections::HashSet;
use std::sync::Mutex;

use crate::bytes::{le_u16, le_u32, le_u64};
use crate::catalog::CommandKind;
use crate::frame::{Action, FrameFormat, Packet, HEADER_6E};
use crate::model::*;
use crate::request::{app_setting, Request};
use crate::version::parse_firmware_version;

/// A completed reply.
#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum Response {
    /// The generic ack. Status 0 = applied; 1 = rejected or no data;
    /// 2 = no handler for the command.
    Ack {
        command: u8,
        status: u8,
    },
    BindStart {
        user_id: u32,
    },
    BindEnd {
        first_bind: bool,
    },
    WatchId {
        id: String,
    },
    Version {
        text: String,
        parsed: Option<FirmwareVersion>,
    },
    Battery {
        status: BatteryStatus,
    },
    Brightness {
        level: u8,
        percent: u8,
    },
    ScreenTimeout {
        seconds: u8,
    },
    Vibration {
        settings: VibrationSettings,
    },
    VibrationStrength {
        value: u8,
    },
    Switches {
        states: SwitchStates,
        raw: Vec<u8>,
    },
    Unit {
        metric: bool,
    },
    UnitsAndFormat {
        value: UnitsAndFormat,
    },
    Goals {
        goals: Goals,
    },
    Goal6e {
        kind: Goal6eKind,
        value: u32,
    },
    SleepWindow {
        window: SleepWindow,
    },
    InactivityAlert {
        alert: InactivityAlert,
    },
    AutoHeartRate {
        value: AutoHeartRate,
    },
    DataCounts {
        counts: DataCounts,
    },
    SportRecords {
        records: Vec<SportRecord>,
    },
    SleepPoints {
        points: Vec<SleepPoint>,
    },
    HeartRateSamples {
        samples: Vec<HeartRateSample>,
    },
    WorkoutCount {
        count: u16,
    },
    Workouts {
        records: Vec<WorkoutRecord>,
    },
    WorkoutHeartRateCount {
        count: u32,
    },
    WorkoutHeartRate {
        samples: Vec<WorkoutHeartRateSample>,
    },
    Count6e {
        count: u32,
    },
    SportRecord6e {
        timestamp: i64,
        steps: u32,
        calories: u32,
    },
    SleepPoint6e {
        timestamp: i64,
        status: u8,
    },
    AlarmCounts {
        counts: AlarmCounts,
    },
    Alarms {
        alarms: Vec<Alarm>,
    },
    FreeAlarmId {
        id: u8,
    },
    Reminder {
        reminder: Reminder,
    },
    /// The watch finished sending 6E reminders (each arrives as an event).
    RemindersSent,
    WatchFaces {
        faces: Vec<WatchFace>,
    },
    PageOrder {
        pages: Vec<u8>,
        reported_count: Option<u8>,
        raw: Vec<u8>,
    },
    PageRouteTrace {
        trace: PageRouteTrace,
    },
    Capabilities {
        value: Capabilities,
    },
    /// Raw bytes from a memory or resource read; empty when rejected.
    Bytes {
        data: Vec<u8>,
    },
    /// The reply to a `Raw` request.
    Packet {
        packet: Packet,
    },
    /// A matching reply that was too short or malformed to decode.
    Unparsed {
        packet: Packet,
    },
}

/// What a matcher made of one packet.
#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum MatchOutcome {
    /// Not for this request.
    Ignored,
    /// One more record of a multi-frame reply.
    Progress {
        received: u32,
        total: u32,
    },
    Complete {
        response: Response,
    },
}

fn plausible_timestamp(value: i64, now_unix: i64) -> bool {
    value >= 946_684_800 && value <= now_unix + 365 * 24 * 60 * 60
}

#[derive(Default)]
struct Raw6eHrDetail {
    abs: i64,
    relative: i64,
    negative_relative: i64,
    average_heart_rate: i64,
}

enum Collect {
    None,
    Sport(Vec<SportRecord>),
    Sleep(Vec<SleepPoint>),
    HeartRate(Vec<HeartRateSample>),
    Workouts(Vec<WorkoutRecord>),
    WorkoutHr(Vec<Raw6eHrDetail>),
    Alarms(Vec<Alarm>),
}

/// Tracks one request's reply.
pub struct Matcher {
    request: Request,
    format: FrameFormat,
    response_command: Option<u8>,
    alternates: Vec<u8>,
    seen: HashSet<u16>,
    collect: Collect,
}

impl Matcher {
    pub fn new(request: Request) -> Matcher {
        let spec = request.kind().spec();
        let format = match &request {
            Request::Raw { frame, .. } if frame.first() == Some(&HEADER_6E) => FrameFormat::Frame6e,
            Request::Raw { .. } => FrameFormat::Frame6f,
            _ => spec.format,
        };
        let collect = match &request {
            Request::GetSportRecords { .. } => Collect::Sport(Vec::new()),
            Request::GetSleepRecords { .. } => Collect::Sleep(Vec::new()),
            Request::GetHeartRateRecords { .. } => Collect::HeartRate(Vec::new()),
            Request::GetWorkouts { .. } => Collect::Workouts(Vec::new()),
            Request::GetWorkoutHeartRate { .. } => Collect::WorkoutHr(Vec::new()),
            Request::GetAlarms { .. } => Collect::Alarms(Vec::new()),
            _ => Collect::None,
        };
        Matcher {
            response_command: request.response_command(),
            alternates: spec.alternate_response_commands,
            request,
            format,
            seen: HashSet::new(),
            collect,
        }
    }

    pub fn request(&self) -> &Request {
        &self.request
    }

    pub fn offer(&mut self, packet: &Packet, now_unix: i64) -> MatchOutcome {
        let Some(expected) = self.response_command else {
            return MatchOutcome::Ignored;
        };
        if let Some((command, status)) = packet.ack() {
            if command == expected {
                return self.on_ack(command, status);
            }
        }
        if packet.format == FrameFormat::Frame6f
            && packet.command == 0x01
            && packet.payload.len() <= 2
        {
            return MatchOutcome::Ignored;
        }
        if packet.format != self.format
            || (packet.command != expected && !self.alternates.contains(&packet.command))
        {
            return MatchOutcome::Ignored;
        }
        self.parse(packet, now_unix)
    }

    /// What a timed-out request still produced: partial syncs, or the
    /// documented empty result for capability and memory probes.
    pub fn timeout_fallback(&self) -> Option<Response> {
        match &self.request {
            Request::GetCapabilities => Some(Response::Capabilities {
                value: capabilities(0),
            }),
            Request::ReadMemory { .. } | Request::ReadResource { .. } => {
                Some(Response::Bytes { data: Vec::new() })
            }
            _ => self.partial(),
        }
    }

    fn partial(&self) -> Option<Response> {
        match &self.collect {
            Collect::None => None,
            Collect::Sport(r) => Some(Response::SportRecords { records: r.clone() }),
            Collect::Sleep(p) => Some(Response::SleepPoints { points: p.clone() }),
            Collect::HeartRate(s) => Some(Response::HeartRateSamples { samples: s.clone() }),
            Collect::Workouts(r) => Some(Response::Workouts { records: r.clone() }),
            Collect::WorkoutHr(d) => Some(Response::WorkoutHeartRate {
                samples: resolve_hr_detail(d, i64::MAX / 4),
            }),
            Collect::Alarms(a) => Some(Response::Alarms { alarms: a.clone() }),
        }
    }

    fn on_ack(&mut self, command: u8, status: u8) -> MatchOutcome {
        MatchOutcome::Complete {
            response: Response::Ack { command, status },
        }
    }

    fn total(&self) -> u32 {
        match &self.request {
            Request::GetSportRecords { count }
            | Request::GetSleepRecords { count }
            | Request::GetHeartRateRecords { count }
            | Request::GetWorkouts { count } => u32::from(*count),
            Request::GetWorkoutHeartRate { count } => *count,
            Request::GetAlarms { expected } => u32::from(*expected),
            _ => 1,
        }
    }

    /// Accepts a 1-based record index once.
    fn accept_index(&mut self, index: u16) -> bool {
        index >= 1 && u32::from(index) <= self.total() && self.seen.insert(index)
    }

    fn progress_or_done(&self, received: usize) -> MatchOutcome {
        let total = self.total();
        if received as u32 >= total {
            MatchOutcome::Complete {
                response: self.partial().unwrap(),
            }
        } else {
            MatchOutcome::Progress {
                received: received as u32,
                total,
            }
        }
    }

    fn parse(&mut self, packet: &Packet, now_unix: i64) -> MatchOutcome {
        let d = packet.payload.as_slice();
        let unparsed = || MatchOutcome::Complete {
            response: Response::Unparsed {
                packet: packet.clone(),
            },
        };
        let done = |response: Response| MatchOutcome::Complete { response };

        match self.request.kind() {
            CommandKind::GetSportRecords => {
                if d.len() < 10 {
                    return MatchOutcome::Ignored;
                }
                let index = le_u16(d, 0);
                if !self.accept_index(index) {
                    return MatchOutcome::Ignored;
                }
                let record = SportRecord {
                    index,
                    timestamp: le_u64(d, 2, 4) as i64,
                    steps: le_u32(d, 6, 4),
                    calories: if d.len() >= 14 { le_u32(d, 10, 4) } else { 0 },
                    distance_m: if d.len() >= 18 { le_u32(d, 14, 4) } else { 0 },
                    duration_min: if d.len() >= 22 { le_u32(d, 18, 4) } else { 0 },
                    static_calories: if d.len() >= 28 { le_u32(d, 24, 4) } else { 0 },
                };
                let Collect::Sport(records) = &mut self.collect else {
                    unreachable!()
                };
                records.push(record);
                let n = records.len();
                self.progress_or_done(n)
            }
            CommandKind::GetSleepRecords => {
                if d.len() < 7 {
                    return MatchOutcome::Ignored;
                }
                let index = le_u16(d, 0);
                if !self.accept_index(index) {
                    return MatchOutcome::Ignored;
                }
                let raw_status = d[6];
                let point = SleepPoint {
                    index,
                    timestamp: le_u64(d, 2, 4) as i64,
                    raw_status,
                    status: if raw_status == 18 { 17 } else { raw_status },
                };
                let Collect::Sleep(points) = &mut self.collect else {
                    unreachable!()
                };
                points.push(point);
                let n = points.len();
                self.progress_or_done(n)
            }
            CommandKind::GetHeartRateRecords => {
                if d.len() < 7 {
                    return MatchOutcome::Ignored;
                }
                let index = le_u16(d, 0);
                if !self.accept_index(index) {
                    return MatchOutcome::Ignored;
                }
                let sample = HeartRateSample {
                    index,
                    timestamp: le_u64(d, 2, 4) as i64,
                    bpm: d[6],
                };
                let Collect::HeartRate(samples) = &mut self.collect else {
                    unreachable!()
                };
                samples.push(sample);
                let n = samples.len();
                self.progress_or_done(n)
            }
            CommandKind::GetWorkouts => {
                if d.is_empty() {
                    return MatchOutcome::Ignored;
                }
                let next_index = match &self.collect {
                    Collect::Workouts(r) => r.len() as u16 + 1,
                    _ => 1,
                };
                let Some(record) = parse_workout(d, next_index, now_unix) else {
                    return MatchOutcome::Ignored;
                };
                if !self.accept_index(record.index) {
                    return MatchOutcome::Ignored;
                }
                let Collect::Workouts(records) = &mut self.collect else {
                    unreachable!()
                };
                records.push(record);
                let n = records.len();
                self.progress_or_done(n)
            }
            CommandKind::GetWorkoutHeartRate => {
                if packet.action != Action::CheckResponse {
                    return MatchOutcome::Complete {
                        response: self.partial().unwrap(),
                    };
                }
                let parsed = parse_hr_detail(d);
                let Collect::WorkoutHr(details) = &mut self.collect else {
                    unreachable!()
                };
                details.extend(parsed);
                let n = details.len();
                if n as u32 >= self.total() {
                    let Collect::WorkoutHr(details) = &self.collect else {
                        unreachable!()
                    };
                    MatchOutcome::Complete {
                        response: Response::WorkoutHeartRate {
                            samples: resolve_hr_detail(details, now_unix),
                        },
                    }
                } else {
                    MatchOutcome::Progress {
                        received: n as u32,
                        total: self.total(),
                    }
                }
            }
            CommandKind::GetAlarms => {
                let alarm = parse_alarm(d);
                let Collect::Alarms(alarms) = &mut self.collect else {
                    unreachable!()
                };
                if let Some(alarm) = alarm {
                    alarms.push(alarm);
                }
                let n = alarms.len();
                if !self.request.is_multi_response() || n as u32 >= self.total() {
                    MatchOutcome::Complete {
                        response: self.partial().unwrap(),
                    }
                } else if alarm.is_some() {
                    MatchOutcome::Progress {
                        received: n as u32,
                        total: self.total(),
                    }
                } else {
                    MatchOutcome::Ignored
                }
            }

            CommandKind::BindStart => {
                if d.is_empty() {
                    return unparsed();
                }
                done(Response::BindStart {
                    user_id: le_u32(d, 0, 4),
                })
            }
            CommandKind::BindEnd => done(Response::BindEnd {
                first_bind: d.first() == Some(&1),
            }),
            CommandKind::GetWatchId => {
                let printable =
                    !d.is_empty() && d.iter().all(|b| b.is_ascii() && *b != b'\n' && *b != b'\r');
                let id = if printable {
                    String::from_utf8_lossy(d).into_owned()
                } else if d.is_empty() {
                    "unknown".into()
                } else {
                    d.iter().map(|b| format!("{b:02X}")).collect()
                };
                done(Response::WatchId { id })
            }
            CommandKind::GetDeviceVersion | CommandKind::GetDeviceVersionNoArgument => {
                let body = if d.len() > 1 && d[0] <= 0x06 {
                    &d[1..]
                } else {
                    d
                };
                let text: String = String::from_utf8_lossy(body)
                    .chars()
                    .filter(|c| !c.is_control())
                    .collect();
                if text.is_empty() {
                    return unparsed();
                }
                let parsed = parse_firmware_version(text.clone());
                done(Response::Version { text, parsed })
            }
            CommandKind::GetDeviceVersion6e => {
                if d.is_empty() {
                    return unparsed();
                }
                let text = d.iter().map(u8::to_string).collect::<Vec<_>>().join(".");
                done(Response::Version { text, parsed: None })
            }
            CommandKind::GetBattery => {
                let Some(&b) = d.first() else {
                    return unparsed();
                };
                let new_mode = matches!(self.request, Request::GetBattery { new_mode: true });
                let status = if new_mode {
                    BatteryStatus {
                        level: b & 0x7F,
                        charging: b & 0x80 != 0,
                    }
                } else {
                    BatteryStatus {
                        level: b,
                        charging: false,
                    }
                };
                done(Response::Battery { status })
            }
            CommandKind::GetBattery6e => {
                let Some(&b) = d.first() else {
                    return unparsed();
                };
                done(Response::Battery {
                    status: BatteryStatus {
                        level: b & 0x7F,
                        charging: b & 0x80 != 0,
                    },
                })
            }
            CommandKind::GetBrightness => {
                let raw = match d.len() {
                    0 => return unparsed(),
                    1 => d[0],
                    _ => d[1],
                };
                let level = if raw > 5 {
                    (raw / 20).clamp(1, 5)
                } else {
                    raw.max(1)
                };
                done(Response::Brightness {
                    level,
                    percent: raw,
                })
            }
            CommandKind::GetScreenTimeout => match d.first() {
                Some(&s) => done(Response::ScreenTimeout { seconds: s }),
                None => unparsed(),
            },
            CommandKind::GetVibration => {
                if d.len() < 2 {
                    return unparsed();
                }
                done(Response::Vibration {
                    settings: VibrationSettings {
                        screen_speed: d[0],
                        enabled: d[1] != 0,
                    },
                })
            }
            CommandKind::GetVibrationStrength => match d.first() {
                Some(&v) => done(Response::VibrationStrength { value: v }),
                None => unparsed(),
            },
            CommandKind::GetSwitches => done(Response::Switches {
                states: SwitchStates {
                    auto_sleep: d.first().map(|b| b & 0x08 != 0),
                    raise_to_wake: (d.len() > 1).then(|| d[1] & 0x40 != 0),
                },
                raw: d.to_vec(),
            }),
            CommandKind::GetUnit => {
                let raw = if d.len() >= 2 && d[0] == 0x0C {
                    Some(d[1])
                } else {
                    d.first().copied()
                };
                match raw {
                    Some(r) => done(Response::Unit { metric: r == 0 }),
                    None => unparsed(),
                }
            }
            CommandKind::GetUnitsAndFormat6e => match d.last() {
                Some(&b) => done(Response::UnitsAndFormat {
                    value: UnitsAndFormat {
                        use_24h: b & 0x01 != 0,
                        metric: b & 0x02 != 0,
                    },
                }),
                None => unparsed(),
            },
            CommandKind::GetGoals => {
                if d.len() < 12 || d.len() % 3 != 0 {
                    return unparsed();
                }
                let slot = |i: usize| u32::from(le_u16(d, i * 3));
                done(Response::Goals {
                    goals: Goals {
                        steps: slot(0) * 100,
                        calories: slot(1),
                        distance_km: slot(2),
                        sleep_hours: slot(3),
                        sport_minutes: (d.len() >= 15).then(|| slot(4)),
                    },
                })
            }
            CommandKind::GetGoal6e => {
                let Request::GetGoal6e { kind } = self.request else {
                    unreachable!()
                };
                if d.len() < 5 || d[0] != kind.byte() {
                    return MatchOutcome::Ignored;
                }
                let raw = le_u32(d, 1, 4);
                let value = if kind == Goal6eKind::SleepMinutes {
                    raw / 60
                } else {
                    raw
                };
                done(Response::Goal6e { kind, value })
            }
            CommandKind::GetSleepWindow => {
                if d.len() < 5 {
                    return unparsed();
                }
                done(Response::SleepWindow {
                    window: SleepWindow {
                        start_hour: d[0],
                        start_minute: d[1],
                        end_hour: d[2],
                        end_minute: d[3],
                        weekdays: d[4],
                    },
                })
            }
            CommandKind::GetInactivityAlert6e => {
                if d.len() < 7 {
                    return unparsed();
                }
                done(Response::InactivityAlert {
                    alert: InactivityAlert {
                        enabled: d[0] & 0x80 != 0,
                        weekdays: d[0] & 0x7F,
                        interval_minutes: d[1],
                        start_hour: d[2],
                        start_minute: d[3],
                        end_hour: d[4],
                        end_minute: d[5],
                        step_threshold: if d.len() >= 8 { le_u16(d, 6) } else { 0 },
                    },
                })
            }
            CommandKind::GetAutoHeartRate => match d.first() {
                Some(&m) => done(Response::AutoHeartRate {
                    value: AutoHeartRate {
                        enabled: m > 0,
                        interval_minutes: m,
                    },
                }),
                None => unparsed(),
            },
            CommandKind::GetDataCounts => {
                if d.len() < 4 {
                    return done(Response::DataCounts {
                        counts: DataCounts {
                            sport: 0,
                            sleep: 0,
                            heart_rate: 0,
                        },
                    });
                }
                done(Response::DataCounts {
                    counts: DataCounts {
                        sport: le_u16(d, 0),
                        sleep: le_u16(d, 2),
                        heart_rate: if d.len() >= 6 { le_u16(d, 4) } else { 0 },
                    },
                })
            }
            CommandKind::GetWorkoutCount => done(Response::WorkoutCount {
                count: le_u64(d, 0, 2) as u16,
            }),
            CommandKind::GetWorkoutHeartRateCount => {
                if packet.action != Action::CheckResponse {
                    return done(Response::WorkoutHeartRateCount { count: 0 });
                }
                done(Response::WorkoutHeartRateCount {
                    count: le_u32(d, 0, 4),
                })
            }
            CommandKind::GetSportSleepCount6e => done(Response::Count6e {
                count: le_u32(d, 0, 3),
            }),
            CommandKind::GetSportData6e => {
                if d.len() < 14 {
                    return unparsed();
                }
                done(Response::SportRecord6e {
                    timestamp: le_u64(d, 1, 4) as i64,
                    steps: le_u32(d, 5, 4),
                    calories: le_u32(d, 9, 4),
                })
            }
            CommandKind::GetSleepData6e => {
                if d.len() < 5 {
                    return unparsed();
                }
                let mut status = d[0];
                if status == 18 {
                    status = 17;
                }
                if matches!(status, 2..=4) {
                    status = 3;
                }
                done(Response::SleepPoint6e {
                    timestamp: le_u64(d, 1, 4) as i64,
                    status,
                })
            }
            CommandKind::GetAlarmCounts => {
                if d.len() < 2 {
                    return unparsed();
                }
                done(Response::AlarmCounts {
                    counts: AlarmCounts {
                        reminders: d[0],
                        alarms: d[1],
                    },
                })
            }
            CommandKind::GetFreeAlarmId => match d.first() {
                Some(&id) => done(Response::FreeAlarmId { id }),
                None => unparsed(),
            },
            CommandKind::GetRemindersLegacy => {
                if d.len() < 6 {
                    return unparsed();
                }
                done(Response::Reminder {
                    reminder: Reminder {
                        index: d[0],
                        enabled: d[1] == 1,
                        kind: ReminderKind::from_legacy_byte(d[2]),
                        hour: d[3],
                        minute: d[4],
                        weekdays: d[5],
                    },
                })
            }
            CommandKind::GetReminders6e => done(Response::RemindersSent),
            CommandKind::GetWatchFaces => {
                if d.len() < 2 {
                    return unparsed();
                }
                let count = usize::from(d[0]);
                let current = d[1];
                let faces = d[2..]
                    .iter()
                    .take(count)
                    .enumerate()
                    .map(|(i, &id)| WatchFace {
                        index: i as u8,
                        id,
                        is_current: i as u8 == current,
                    })
                    .collect();
                done(Response::WatchFaces { faces })
            }
            CommandKind::GetPageOrder => {
                let (pages, reported_count) = match self.request {
                    Request::GetPageOrder {
                        source: PageOrderSource::AppSetting,
                    } => parse_page_order_app_setting(d),
                    _ => (parse_page_order_driver_blob(d), None),
                };
                done(Response::PageOrder {
                    pages,
                    reported_count,
                    raw: d.to_vec(),
                })
            }
            CommandKind::GetPageRouteTrace => match parse_route_trace(d) {
                Some(trace) => done(Response::PageRouteTrace { trace }),
                None => unparsed(),
            },
            CommandKind::SetPageOrder => match d.first() {
                Some(&status) => done(Response::Ack {
                    command: 0x2A,
                    status,
                }),
                None => unparsed(),
            },
            CommandKind::GetCapabilities => {
                let raw = if d.len() >= 4 { le_u32(d, 0, 4) } else { 0 };
                done(Response::Capabilities {
                    value: capabilities(raw),
                })
            }
            CommandKind::ReadMemory | CommandKind::ReadResource => {
                done(Response::Bytes { data: d.to_vec() })
            }
            CommandKind::Raw => done(Response::Packet {
                packet: packet.clone(),
            }),
            _ => unparsed(),
        }
    }
}

fn parse_workout(d: &[u8], next_index: u16, now_unix: i64) -> Option<WorkoutRecord> {
    parse_workout_fixed(d, now_unix)
        .or_else(|| parse_workout_tlv(d, 0, next_index, now_unix))
        .or_else(|| parse_workout_tlv(d, 2, next_index, now_unix))
}

fn u32_at(d: &[u8], offset: usize) -> u32 {
    if offset >= d.len() {
        0
    } else {
        le_u32(d, offset, 4)
    }
}

fn parse_workout_fixed(d: &[u8], now_unix: i64) -> Option<WorkoutRecord> {
    if d.len() < 31 {
        return None;
    }
    let index = le_u16(d, 0);
    let start = le_u64(d, 2, 4) as i64;
    if !plausible_timestamp(start, now_unix) || index == 0 {
        return None;
    }
    let duration = le_u32(d, 18, 4);
    let raw_end = le_u64(d, 26, 4) as i64;
    let end = if plausible_timestamp(raw_end, now_unix) && raw_end >= start {
        raw_end
    } else {
        start + i64::from(duration)
    };
    Some(WorkoutRecord {
        index,
        start_timestamp: start,
        end_timestamp: end,
        workout_type: u32::from(d[30]),
        steps: le_u32(d, 6, 4),
        calories: le_u32(d, 10, 4),
        distance_m: le_u32(d, 14, 4),
        duration_seconds: duration,
        average_heart_rate: le_u32(d, 22, 4),
        pace: u32_at(d, 31),
        speed: u32_at(d, 35),
        lap_count: u32_at(d, 39),
        zone_warm_up: u32_at(d, 43),
        zone_fat_burning: u32_at(d, 47),
        zone_aerobic: u32_at(d, 51),
        zone_anaerobic: u32_at(d, 55),
        zone_limit: u32_at(d, 59),
    })
}

fn parse_workout_tlv(
    d: &[u8],
    start_offset: usize,
    next_index: u16,
    now_unix: i64,
) -> Option<WorkoutRecord> {
    if start_offset >= d.len() {
        return None;
    }
    let index = if start_offset == 2 {
        le_u16(d, 0)
    } else {
        next_index
    };
    let mut cursor = start_offset;
    let (mut start, mut end) = (0i64, 0i64);
    let mut v = [0u32; 15];
    while cursor + 2 <= d.len() {
        let tag = d[cursor];
        let len = usize::from(d[cursor + 1]);
        let value_end = cursor + 2 + len;
        if len == 0 || value_end > d.len() {
            break;
        }
        let value = le_u64(d, cursor + 2, len);
        if tag == 0 {
            if start == 0 {
                start = value as i64;
            } else {
                end = value as i64;
            }
        } else if usize::from(tag) < v.len() {
            v[usize::from(tag)] = value as u32;
        }
        cursor = value_end;
    }
    if !plausible_timestamp(start, now_unix) {
        return None;
    }
    let duration = v[4];
    let end = if plausible_timestamp(end, now_unix) && end >= start {
        end
    } else {
        start + i64::from(duration)
    };
    Some(WorkoutRecord {
        index,
        start_timestamp: start,
        end_timestamp: end,
        workout_type: v[6],
        steps: v[1],
        calories: v[2],
        distance_m: v[3],
        duration_seconds: duration,
        average_heart_rate: v[5],
        pace: v[7],
        speed: v[8],
        lap_count: v[9],
        zone_warm_up: v[10],
        zone_fat_burning: v[11],
        zone_aerobic: v[12],
        zone_anaerobic: v[13],
        zone_limit: v[14],
    })
}

fn read_varint(d: &[u8], cursor: &mut usize) -> Option<i64> {
    let mut result = 0i64;
    let mut shift = 0;
    while *cursor < d.len() && shift < 32 {
        let byte = d[*cursor];
        *cursor += 1;
        result |= i64::from(byte & 0x7F) << shift;
        if byte & 0x80 == 0 {
            return Some(result);
        }
        shift += 7;
    }
    None
}

fn parse_hr_detail(d: &[u8]) -> Vec<Raw6eHrDetail> {
    let mut out = Vec::new();
    let mut current: Option<Raw6eHrDetail> = None;
    let mut cursor = 0;
    while cursor < d.len() {
        let (Some(key), Some(value)) = (read_varint(d, &mut cursor), read_varint(d, &mut cursor))
        else {
            break;
        };
        if key <= 2 {
            if let Some(c) = current.take() {
                out.push(c);
            }
            current = Some(Raw6eHrDetail::default());
        }
        let Some(c) = current.as_mut() else { continue };
        match key {
            0 => c.abs = value,
            1 => c.relative = value,
            2 => c.negative_relative = value,
            7 => c.average_heart_rate = value,
            _ => {}
        }
    }
    out.extend(current);
    out
}

fn resolve_hr_detail(details: &[Raw6eHrDetail], now_unix: i64) -> Vec<WorkoutHeartRateSample> {
    let mut anchor = 0i64;
    details
        .iter()
        .filter_map(|d| {
            let timestamp = if d.abs > 0 {
                anchor = d.abs;
                d.abs
            } else if d.negative_relative > 0 {
                anchor - d.negative_relative
            } else {
                anchor + d.relative
            };
            (plausible_timestamp(timestamp, now_unix) && (20..=230).contains(&d.average_heart_rate))
                .then_some(WorkoutHeartRateSample {
                    timestamp,
                    bpm: d.average_heart_rate as u32,
                })
        })
        .collect()
}

/// `x id` then TLVs; only type-4 (alarm) records are returned.
pub(crate) fn parse_alarm(d: &[u8]) -> Option<Alarm> {
    if d.len() < 2 {
        return None;
    }
    let id = d[1];
    let mut offset = 2;
    let (mut hour, mut minute, mut enabled) = (None, None, None);
    let (mut weekdays, mut snooze) = (0u8, 0u8);
    while offset + 1 < d.len() {
        let field = d[offset];
        let len = usize::from(d[offset + 1]);
        let start = offset + 2;
        let end = start + len;
        if end > d.len() {
            return None;
        }
        let value = &d[start..end];
        match field {
            0 if !value.is_empty() && value[0] != 0x04 => return None,
            2 if !value.is_empty() => weekdays = value[0],
            4 if value.len() >= 2 => {
                hour = Some(value[0]);
                minute = Some(value[1]);
            }
            6 if !value.is_empty() => enabled = Some(value[0] == 1),
            8 if !value.is_empty() => snooze = value[0],
            _ => {}
        }
        offset = end;
    }
    Some(Alarm {
        id,
        enabled: enabled?,
        hour: hour?,
        minute: minute?,
        weekdays,
        snooze_minutes: snooze,
    })
}

fn dedupe_pages(pages: impl IntoIterator<Item = u8>) -> Vec<u8> {
    let mut seen = HashSet::new();
    pages
        .into_iter()
        .filter(|p| (1..=9).contains(p) && seen.insert(*p))
        .collect()
}

/// AppSetting `01 count id…`. Diagnostic builds put a `0xD6` strip in the
/// ID field; IDs stop there.
pub(crate) fn parse_page_order_app_setting(d: &[u8]) -> (Vec<u8>, Option<u8>) {
    if d.len() < 2 || d[0] != app_setting::PAGE_QUEUE {
        return (Vec::new(), None);
    }
    let count = usize::from(d[1]);
    let ids: Vec<u8> = d[2..].iter().copied().take(count).collect();
    let page_bytes = ids.iter().position(|b| *b == 0xD6).unwrap_or(ids.len());
    let pages = dedupe_pages(ids[..page_bytes].iter().copied());
    (pages, (page_bytes > 0).then_some(page_bytes as u8))
}

/// Driver blobs: `00 id…` or `count 00 id…`, `FF`/`00` padding skipped.
pub(crate) fn parse_page_order_driver_blob(d: &[u8]) -> Vec<u8> {
    let body = if d.len() >= 2 && d[0] <= 10 && d[1] == 0 {
        &d[2..]
    } else if d.first() == Some(&0) {
        &d[1..]
    } else {
        d
    };
    dedupe_pages(
        body.iter()
            .copied()
            .filter(|b| *b != 0xFF && *b != 0)
            .take(10),
    )
}

fn parse_route_trace(d: &[u8]) -> Option<PageRouteTrace> {
    if d.len() < 10 || d[0] != app_setting::ROUTE_TRACE {
        return None;
    }
    let len = usize::from(d[1]);
    let trace: Vec<u8> = d[2..].iter().copied().take(len).collect();
    if trace.len() < 8 {
        return None;
    }
    let record = |t: &[u8], site: u8| PageRouteRecord {
        counter: t[0],
        site_id: site,
        event_id: u16::from(t[2]) | (u16::from(t[3]) << 8),
        column1_resource: t[4],
        column2_resource: t[5],
        create_resource: t[6],
        pointer_low_byte: t[7],
    };
    let latest = record(&trace[..8], 0xFF);
    let records = trace[8..]
        .chunks_exact(8)
        .map(|c| record(c, c[1]))
        .filter(|r| {
            !(r.counter == 0
                && r.site_id == 0
                && r.event_id == 0
                && r.column1_resource == 0
                && r.column2_resource == 0
                && r.create_resource == 0
                && r.pointer_low_byte == 0)
        })
        .collect();
    Some(PageRouteTrace {
        firmware_counter: trace[0],
        latest,
        records,
    })
}

const CAPABILITY_LABELS: [(u32, &str); 14] = [
    (0, "Calendar"),
    (1, "Memory Probe"),
    (2, "Stopwatch"),
    (3, "Watch Text"),
    (4, "Live HR Stream"),
    (5, "Flash Read"),
    (6, "Clock Above Centre"),
    (7, "Home Widget"),
    (8, "No Goal Alerts"),
    (9, "Notification Mode"),
    (10, "HR Floor 30"),
    (11, "Resource Read"),
    (13, "Battery Stream"),
    (14, "Double Press Home"),
];

/// Names the bits of a patched-firmware capability mask.
#[uniffi::export]
pub fn capabilities(raw: u32) -> Capabilities {
    let labels = CAPABILITY_LABELS
        .iter()
        .filter(|(bit, _)| raw & (1 << bit) != 0)
        .map(|(_, label)| (*label).to_string())
        .collect();
    Capabilities { raw, labels }
}

/// Thread-safe reply matcher for one request, for hosts that run their own
/// queue instead of [`crate::session::Session`].
#[derive(uniffi::Object)]
pub struct ResponseMatcher {
    inner: Mutex<Matcher>,
}

#[uniffi::export]
impl ResponseMatcher {
    #[uniffi::constructor]
    pub fn new(request: Request) -> Self {
        ResponseMatcher {
            inner: Mutex::new(Matcher::new(request)),
        }
    }

    /// Offers a received packet. `now_unix` bounds plausible workout timestamps.
    pub fn offer(&self, packet: Packet, now_unix: i64) -> MatchOutcome {
        self.inner.lock().unwrap().offer(&packet, now_unix)
    }

    /// The result to report if the request times out, if any.
    pub fn timeout_fallback(&self) -> Option<Response> {
        self.inner.lock().unwrap().timeout_fallback()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::Channel;

    const NOW: i64 = 1_791_720_000;

    fn reply(command: u8, payload: &[u8]) -> Packet {
        Packet {
            format: FrameFormat::Frame6f,
            command,
            action: Action::CheckResponse,
            payload: payload.to_vec(),
            channel: Channel::Primary,
        }
    }

    fn ack(command: u8, status: u8) -> Packet {
        Packet {
            action: Action::SetResponse,
            ..reply(0x01, &[command, status])
        }
    }

    fn complete(request: Request, packet: Packet) -> Response {
        match Matcher::new(request).offer(&packet, NOW) {
            MatchOutcome::Complete { response } => response,
            other => panic!("expected completion, got {other:?}"),
        }
    }

    #[test]
    fn parses_replies_recorded_from_the_emulated_firmware() {
        assert_eq!(
            complete(
                Request::GetDeviceVersion,
                reply(0x03, b"\x01A0.2R0.4T0.0H0.0B0.1")
            ),
            Response::Version {
                text: "A0.2R0.4T0.0H0.0B0.1".into(),
                parsed: parse_firmware_version("A0.2R0.4T0.0H0.0B0.1".into()),
            }
        );
        assert_eq!(
            complete(Request::GetBattery { new_mode: true }, reply(0x08, &[0x4D])),
            Response::Battery {
                status: BatteryStatus {
                    level: 77,
                    charging: false
                }
            }
        );
        assert_eq!(
            complete(
                Request::GetGoals,
                reply(0x50, &hex("460001c800010500010800011e0001"))
            ),
            Response::Goals {
                goals: Goals {
                    steps: 7000,
                    calories: 200,
                    distance_km: 5,
                    sleep_hours: 8,
                    sport_minutes: Some(30)
                }
            }
        );
        assert_eq!(
            complete(
                Request::GetAlarms { expected: 1 },
                reply(
                    0x9C,
                    &hex("010100010402017f0402071e0502000006010107010708010a")
                )
            ),
            Response::Alarms {
                alarms: vec![Alarm {
                    id: 1,
                    enabled: true,
                    hour: 7,
                    minute: 30,
                    weekdays: 0x7F,
                    snooze_minutes: 10
                }]
            }
        );
        assert_eq!(
            complete(
                Request::GetPageOrder {
                    source: PageOrderSource::AppSetting
                },
                reply(0x2A, &hex("01050000000000"))
            ),
            Response::PageOrder {
                pages: vec![],
                reported_count: Some(5),
                raw: hex("01050000000000")
            }
        );
        assert_eq!(
            complete(Request::GetSwitches, reply(0x90, &hex("f0d32000"))),
            Response::Switches {
                states: SwitchStates {
                    auto_sleep: Some(false),
                    raise_to_wake: Some(true)
                },
                raw: hex("f0d32000")
            }
        );
    }

    #[test]
    fn acks_complete_sets_and_short_foreign_acks_are_ignored() {
        let mut m = Matcher::new(Request::SetBrightness { level: 4 });
        assert_eq!(m.offer(&ack(0x13, 0), NOW), MatchOutcome::Ignored);
        assert_eq!(
            m.offer(&ack(0x07, 0), NOW),
            MatchOutcome::Complete {
                response: Response::Ack {
                    command: 0x07,
                    status: 0
                }
            }
        );
        assert_eq!(
            complete(Request::GetSportRecords { count: 3 }, ack(0x54, 1)),
            Response::Ack {
                command: 0x54,
                status: 1
            }
        );
    }

    #[test]
    fn capabilities_accept_the_stock_echo_and_time_out_empty() {
        assert_eq!(
            complete(Request::GetCapabilities, reply(0x2A, &[])),
            Response::Capabilities {
                value: capabilities(0)
            }
        );
        let caps = complete(Request::GetCapabilities, reply(0xDC, &[0x03, 0x20, 0, 0]));
        assert_eq!(
            caps,
            Response::Capabilities {
                value: Capabilities {
                    raw: 0x2003,
                    labels: vec![
                        "Calendar".into(),
                        "Memory Probe".into(),
                        "Battery Stream".into()
                    ]
                }
            }
        );
        assert_eq!(
            Matcher::new(Request::ReadMemory {
                address: 0,
                length: 4
            })
            .timeout_fallback(),
            Some(Response::Bytes { data: vec![] })
        );
    }

    #[test]
    fn record_syncs_collect_unique_indices() {
        let mut m = Matcher::new(Request::GetHeartRateRecords { count: 2 });
        let sample = |i: u8, bpm: u8| reply(0x5B, &[i, 0, 0x40, 0x1F, 0xCB, 0x6A, bpm]);
        assert_eq!(
            m.offer(&sample(1, 60), NOW),
            MatchOutcome::Progress {
                received: 1,
                total: 2
            }
        );
        assert_eq!(m.offer(&sample(1, 61), NOW), MatchOutcome::Ignored);
        assert_eq!(m.offer(&sample(9, 61), NOW), MatchOutcome::Ignored);
        match m.offer(&sample(2, 62), NOW) {
            MatchOutcome::Complete {
                response: Response::HeartRateSamples { samples },
            } => {
                assert_eq!(
                    samples.iter().map(|s| s.bpm).collect::<Vec<_>>(),
                    vec![60, 62]
                );
                assert_eq!(samples[0].timestamp, 0x6ACB1F40);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn workout_layouts() {
        let mut fixed = vec![1, 0];
        for v in [1_791_700_000u32, 1000, 50_000, 800, 600, 120, 1_791_700_600] {
            fixed.extend(v.to_le_bytes());
        }
        fixed.push(3);
        let r = parse_workout(&fixed, 1, NOW).unwrap();
        assert_eq!(
            (r.index, r.steps, r.duration_seconds, r.workout_type),
            (1, 1000, 600, 3)
        );
        assert_eq!(r.end_timestamp, 1_791_700_600);

        let mut tlv = vec![0x00, 0x04];
        tlv.extend(1_791_700_000u32.to_le_bytes());
        tlv.extend([0x04, 0x02, 0x58, 0x02, 0x06, 0x01, 0x05]);
        let r = parse_workout(&tlv, 4, NOW).unwrap();
        assert_eq!((r.index, r.duration_seconds, r.workout_type), (4, 600, 5));
        assert_eq!(r.end_timestamp, 1_791_700_600);
    }

    #[test]
    fn workout_heart_rate_detail_varints() {
        let mut d = vec![0x00];
        let mut ts = 1_791_700_000u32;
        while ts >= 0x80 {
            d.push((ts as u8 & 0x7F) | 0x80);
            ts >>= 7;
        }
        d.push(ts as u8);
        d.extend([0x07, 120, 0x01, 5, 0x07, 125]);
        let samples = resolve_hr_detail(&parse_hr_detail(&d), NOW);
        assert_eq!(samples.len(), 2);
        assert_eq!(samples[1].timestamp, 1_791_700_005);
        assert_eq!(samples[1].bpm, 125);
    }

    #[test]
    fn page_order_parsers() {
        assert_eq!(
            parse_page_order_driver_blob(&[0, 2, 5, 0xFF, 7, 2]),
            vec![2, 5, 7]
        );
        assert_eq!(
            parse_page_order_driver_blob(&[3, 0, 9, 8, 4]),
            vec![9, 8, 4]
        );
        assert_eq!(
            parse_page_order_app_setting(&[1, 4, 2, 5, 0xD6, 9]),
            (vec![2, 5], Some(2))
        );
    }

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }
}
