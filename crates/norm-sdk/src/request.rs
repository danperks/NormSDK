//! Typed phone → watch requests and their wire encodings.
//!
//! Every variant maps to exactly one [`CommandKind`], whose
//! [`CommandSpec`](crate::catalog::CommandSpec) documents the frame layout,
//! the expected reply and the evidence behind it.

use crate::bytes::{fixed_field, put_u16, put_u32, utf8_char_prefix, utf8_prefix};
use crate::catalog::CommandKind;
use crate::frame::{encode_6e, encode_6f, Action, WriteChannel};
use crate::model::*;

/// AppSetting (`0x2A`) sub-types used by the requests below.
pub(crate) mod app_setting {
    pub const PAGE_QUEUE: u8 = 0x01;
    pub const CUSTOM_TEXT: u8 = 0x40;
    pub const READ_MEMORY: u8 = 0x40;
    pub const CALENDAR_SLOT: u8 = 0x41;
    pub const READ_RESOURCE: u8 = 0x41;
    pub const CALENDAR_COMMIT: u8 = 0x42;
    pub const CAPABILITIES: u8 = 0x43;
    pub const ROUTE_TRACE: u8 = 0xFD;
    pub const DIAGNOSTIC_DRIVER_74E: u8 = 0xFE;
}

/// Calendar page geometry; the firmware copies fixed 112-byte slots.
pub mod calendar {
    pub const TITLE_FIELD: usize = 40;
    pub const LINE_FIELD: usize = 24;
    pub const MAX_EVENTS: usize = 5;
    /// The scroll row whose unused line2/line3 carry the widget strings.
    pub const WIDGET_SLOT: usize = 1;
}

/// The custom-text page copies at most this many bytes.
pub const CUSTOM_TEXT_MAX_BYTES: usize = 120;
/// Largest resource read: the reply then fills one 244-byte notification.
pub const RESOURCE_READ_MAX: u8 = 0xEE;

/// One phone → watch command.
#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum Request {
    // Pairing and identity
    BindStart {
        mode: BindMode,
    },
    BindEnd,
    GetWatchId,
    GetDeviceVersion,
    GetDeviceVersionNoArgument,
    GetBattery {
        new_mode: bool,
    },
    GetDeviceVersion6e,
    GetBattery6e,

    // Time and profile
    SetDateTime {
        time: LocalDateTime,
        analog_hand_calibration: bool,
    },
    SetDateTime6e {
        time: LocalDateTime,
    },
    SetUserProfile {
        profile: UserProfile,
    },
    SetUserProfile6e {
        profile: UserProfile,
    },
    SetLanguage {
        language: u8,
    },

    // Display and device settings
    SetBrightness {
        level: u8,
    },
    GetBrightness,
    SetScreenTimeout {
        seconds: u8,
    },
    GetScreenTimeout,
    SetVibration {
        enabled: bool,
        screen_speed: u8,
    },
    GetVibration,
    GetVibrationStrength,
    GetSwitches,
    SetRaiseToWake {
        enabled: bool,
    },
    SetAutoSleepSwitch {
        enabled: bool,
    },
    SetUnit {
        metric: bool,
    },
    GetUnit,
    SetUnitsAndFormat6e {
        metric: bool,
        use_24h: bool,
        date_format: DateFormat,
        show_battery: bool,
    },
    GetUnitsAndFormat6e,
    SetGoal {
        kind: GoalKind,
        value: u32,
    },
    GetGoals,
    SetGoal6e {
        kind: Goal6eKind,
        value: u32,
    },
    GetGoal6e {
        kind: Goal6eKind,
    },
    SetSleepWindow {
        window: SleepWindow,
    },
    GetSleepWindow,
    SetInactivityAlert6e {
        alert: InactivityAlert,
    },
    GetInactivityAlert6e,
    SetAutoHeartRate {
        interval_minutes: u8,
    },
    GetAutoHeartRate,

    // Hand calibration
    HandMoveContinuous {
        hand: WatchHand,
        clockwise: bool,
        code: HandCalibrationCode,
    },
    HandSyncTime {
        hour: u8,
        minute: u8,
        second: u8,
        finish: bool,
    },
    HandMachineTiming {
        hour_angle: u16,
        minute_angle: u16,
        hour: u8,
        minute: u8,
        second: u8,
    },
    HandStep {
        hand: WatchHand,
        clockwise: bool,
        angle: u16,
    },

    // Health data
    GetDataCounts,
    GetSportRecords {
        count: u16,
    },
    GetSleepRecords {
        count: u16,
    },
    GetHeartRateRecords {
        count: u16,
    },
    GetWorkoutCount,
    GetWorkouts {
        count: u16,
    },
    GetWorkoutHeartRateCount,
    GetWorkoutHeartRate {
        count: u32,
    },
    DeleteSportData,
    DeleteSleepData,
    DeleteHeartRateData,
    DeleteWorkoutData,
    GetSportSleepCount6e {
        kind: SportSleepKind,
    },
    GetSportData6e,
    GetSleepData6e,

    // Alarms and reminders
    GetAlarmCounts,
    GetAlarms {
        expected: u8,
    },
    GetFreeAlarmId,
    SetAlarm {
        alarm: Alarm,
        is_new: bool,
    },
    SetReminderLegacy {
        reminder: Reminder,
    },
    GetRemindersLegacy,
    GetReminders6e,
    AddReminder6e {
        reminder: Reminder,
    },
    DeleteReminder6e {
        hour: u8,
        minute: u8,
    },
    DeleteAllReminders6e,

    // Weather, notifications, media and remote replies
    SetTemperatureUnit {
        unit: TemperatureUnit,
    },
    PushWeather {
        days: Vec<WeatherDay>,
        city: Option<String>,
        unit: TemperatureUnit,
    },
    PushMessageCounts {
        counts: Vec<MessageCount>,
    },
    PushSms {
        sender: String,
        body: String,
    },
    PushMusicState {
        playing: bool,
        title_and_artist: String,
    },
    PushMusicVolume {
        percent: u8,
    },
    PushMusicStop,
    FindPhoneStartAck,
    FindPhoneFound,
    PhoneGpsStatus {
        gps_available: bool,
    },
    PhoneGpsDistance {
        gps_available: bool,
        distance_m: u32,
        speed_kmh: u32,
        confirmations: GpsConfirmations,
    },
    PhoneGpsLocation {
        fix: GpsFix,
    },

    // Watch faces and carousel
    GetWatchFaces,
    SelectWatchFace {
        index: u8,
    },
    SetCrownFunctions {
        functions: Vec<CrownFunction>,
    },
    GetPageOrder {
        source: PageOrderSource,
    },
    SetPageOrder {
        pages: Vec<u8>,
    },
    GetPageRouteTrace,

    // Patched firmware
    GetCapabilities,
    ReadMemory {
        address: u32,
        length: u8,
    },
    ReadResource {
        handle: u32,
        offset: u32,
        length: u8,
    },
    SetCustomText {
        text: String,
    },
    PushCalendarEvents {
        events: Vec<CalendarEvent>,
    },

    // System
    EnterUpgradeMode,
    EnterUpgradeMode6e {
        mode: u8,
        subtype: u8,
        value: u32,
        label: String,
    },
    RestoreFactory6e,

    /// An arbitrary frame for probing. Not part of the annotated catalog's
    /// guarantees; prefer a typed request once a path is proven.
    Raw {
        label: String,
        frame: Vec<u8>,
        channel: WriteChannel,
        response_command: Option<u8>,
        timeout_ms: u32,
    },
}

/// One write the host must perform, in order.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct OutgoingFrame {
    pub channel: WriteChannel,
    pub data: Vec<u8>,
}

fn f6f(command: u8, action: Action, payload: &[u8]) -> Vec<OutgoingFrame> {
    vec![OutgoingFrame {
        channel: WriteChannel::Primary,
        data: encode_6f(command, action, payload),
    }]
}

fn set(command: u8, payload: &[u8]) -> Vec<OutgoingFrame> {
    f6f(command, Action::Set, payload)
}

fn check(command: u8, payload: &[u8]) -> Vec<OutgoingFrame> {
    f6f(command, Action::Check, payload)
}

fn f6e(command: u8, payload: &[u8]) -> Vec<OutgoingFrame> {
    vec![OutgoingFrame {
        channel: WriteChannel::Primary,
        data: encode_6e(command, payload),
    }]
}

fn remote(command: u8, action: Action, payload: &[u8]) -> Vec<OutgoingFrame> {
    vec![OutgoingFrame {
        channel: WriteChannel::Remote,
        data: encode_6f(command, action, payload),
    }]
}

fn clamp_i8(value: i32) -> u8 {
    value.clamp(i32::from(i8::MIN), i32::from(i8::MAX)) as i8 as u8
}

/// Phone-GPS confirm bytes are inverted on the wire: 0 means confirmed.
fn confirm_byte(confirmed: bool) -> u8 {
    if confirmed {
        0x00
    } else {
        0x01
    }
}

fn gps_status(gps_available: bool) -> Vec<u8> {
    vec![0x00, 0x01, if gps_available { 0x00 } else { 0x01 }]
}

fn zone_bytes(utc_offset_minutes: i32) -> [u8; 3] {
    let absolute = utc_offset_minutes.unsigned_abs();
    [
        if utc_offset_minutes < 0 { 0 } else { 1 },
        (absolute / 60).min(23) as u8,
        (absolute % 60).min(59) as u8,
    ]
}

fn calendar_slot_frame(slot: u8, event: &CalendarEvent) -> OutgoingFrame {
    use calendar::*;
    let mut payload = vec![
        app_setting::CALENDAR_SLOT,
        (1 + TITLE_FIELD + 3 * LINE_FIELD) as u8,
        slot,
    ];
    payload.extend(fixed_field(&event.title, TITLE_FIELD));
    payload.extend(fixed_field(&event.line1, LINE_FIELD));
    payload.extend(fixed_field(&event.line2, LINE_FIELD));
    payload.extend(fixed_field(&event.line3, LINE_FIELD));
    OutgoingFrame {
        channel: WriteChannel::Primary,
        data: encode_6f(0x2A, Action::Set, &payload),
    }
}

fn empty_event() -> CalendarEvent {
    CalendarEvent {
        title: String::new(),
        line1: String::new(),
        line2: String::new(),
        line3: String::new(),
        widget_title: String::new(),
        widget_subtitle: String::new(),
    }
}

impl Request {
    /// The wire frames for this request, in send order. Most requests are a
    /// single frame; calendar pushes are five slot frames plus a commit.
    pub fn frames(&self) -> Vec<OutgoingFrame> {
        use Request::*;
        match self {
            BindStart { mode } => match mode {
                BindMode::QrCode => set(0x93, &[0x02]),
                BindMode::ManualConfirm => set(0x93, &[0x01]),
                BindMode::Uid { uid } => {
                    let mut p = vec![0x00];
                    put_u32(&mut p, *uid);
                    set(0x93, &p)
                }
            },
            BindEnd => set(0x94, &[0x01]),
            GetWatchId => check(0x02, &[0x00]),
            GetDeviceVersion => check(0x03, &[0x01]),
            GetDeviceVersionNoArgument => check(0x03, &[]),
            GetBattery { new_mode } => check(0x08, &[u8::from(*new_mode)]),
            GetDeviceVersion6e => f6e(0x03, &[0x03]),
            GetBattery6e => f6e(0x0F, &[0x01]),

            SetDateTime {
                time,
                analog_hand_calibration,
            } => {
                let zone = zone_bytes(time.utc_offset_minutes);
                let [y0, y1] = time.year.to_le_bytes();
                set(
                    0x04,
                    &[
                        y0,
                        y1,
                        time.month,
                        time.day,
                        time.hour,
                        time.minute,
                        time.second,
                        0x00,
                        u8::from(*analog_hand_calibration),
                        zone[0],
                        zone[1],
                        zone[2],
                    ],
                )
            }
            SetDateTime6e { time } => {
                let [y0, y1] = time.year.to_le_bytes();
                f6e(
                    0x15,
                    &[
                        y0,
                        y1,
                        time.month,
                        time.day,
                        time.hour,
                        time.minute,
                        time.second,
                    ],
                )
            }
            SetUserProfile { profile } => {
                let mut p = vec![profile.gender.byte(), profile.age, profile.height_cm];
                put_u16(&mut p, profile.weight_tenths_kg);
                set(0x30, &p)
            }
            SetUserProfile6e { profile } => {
                let weight = (u32::from(profile.weight_tenths_kg) * 10) as u16;
                let mut p = vec![profile.gender.byte_6e()];
                put_u16(&mut p, u16::from(profile.age));
                p.extend([0x01, 0x01, profile.height_cm]);
                put_u16(&mut p, weight);
                f6e(0x0C, &p)
            }
            SetLanguage { language } => set(0x09, &[*language]),

            SetBrightness { level } => set(0x07, &[(*level).clamp(1, 5) * 20]),
            GetBrightness => check(0x07, &[0x00]),
            SetScreenTimeout { seconds } => {
                let snapped = [5u8, 15, 30, 60]
                    .into_iter()
                    .min_by_key(|s| (i16::from(*s) - i16::from(*seconds)).abs())
                    .unwrap();
                set(0x13, &[snapped])
            }
            GetScreenTimeout => check(0x13, &[0x00]),
            SetVibration {
                enabled,
                screen_speed,
            } => set(
                0xF0,
                &[(*screen_speed).clamp(2, 3), u8::from(*enabled), 0x01],
            ),
            GetVibration => check(0xF0, &[0x00]),
            GetVibrationStrength => check(0x10, &[0x00]),
            GetSwitches => check(0x90, &[0x00]),
            SetRaiseToWake { enabled } => set(0x90, &[0x01, 0x0E, u8::from(*enabled)]),
            SetAutoSleepSwitch { enabled } => set(0x90, &[0x01, 0x03, u8::from(*enabled)]),
            SetUnit { metric } => set(0x0C, &[if *metric { 0x00 } else { 0x01 }]),
            GetUnit => check(0x0C, &[0x00]),
            SetUnitsAndFormat6e {
                metric,
                use_24h,
                date_format,
                show_battery,
            } => {
                let mut byte = 0x04u8;
                if *use_24h {
                    byte |= 0x01;
                }
                if *metric {
                    byte |= 0x02;
                }
                byte |= match date_format {
                    DateFormat::MonthDayYear => 0,
                    DateFormat::DayMonthYear => 1,
                    DateFormat::YearMonthDay => 2,
                } << 3;
                if *show_battery {
                    byte |= 0x40;
                }
                f6e(0x34, &[byte])
            }
            GetUnitsAndFormat6e => f6e(0x34, &[0xFF]),
            SetGoal { kind, value } => {
                let raw = match kind {
                    GoalKind::Steps => value / 100,
                    _ => *value,
                } as u16;
                let [lo, hi] = raw.to_le_bytes();
                set(0x50, &[kind.byte(), lo, hi, 0x01])
            }
            GetGoals => check(0x50, &[0x00]),
            SetGoal6e { kind, value } => {
                let raw = if *kind == Goal6eKind::SleepMinutes {
                    value * 60
                } else {
                    *value
                };
                let mut p = vec![kind.byte()];
                put_u32(&mut p, raw);
                f6e(0xA2, &p)
            }
            GetGoal6e { kind } => f6e(0xA2, &[kind.byte(), 0, 0, 0, 0]),
            SetSleepWindow { window } => set(
                0x58,
                &[
                    window.start_hour,
                    window.start_minute,
                    window.end_hour,
                    window.end_minute,
                    window.weekdays,
                ],
            ),
            GetSleepWindow => check(0x58, &[0x00]),
            SetInactivityAlert6e { alert } => {
                let mut cycle = alert.weekdays & 0x7F;
                if alert.enabled {
                    cycle |= 0x80;
                }
                let mut p = vec![
                    0x01,
                    cycle,
                    alert.interval_minutes,
                    alert.start_hour,
                    alert.start_minute,
                    alert.end_hour,
                    alert.end_minute,
                ];
                put_u16(&mut p, alert.step_threshold);
                f6e(0x43, &p)
            }
            GetInactivityAlert6e => f6e(0x43, &[0; 9]),
            SetAutoHeartRate { interval_minutes } => set(0x5C, &[*interval_minutes]),
            GetAutoHeartRate => check(0x5C, &[0x00]),

            HandMoveContinuous {
                hand,
                clockwise,
                code,
            } => set(0xB8, &[hand.byte(), u8::from(*clockwise), code.byte()]),
            HandSyncTime {
                hour,
                minute,
                second,
                finish,
            } => set(
                0xB8,
                &[
                    WatchHand::Minute.byte(),
                    0x01,
                    0x04,
                    *hour,
                    *minute,
                    *second,
                    u8::from(*finish),
                ],
            ),
            HandMachineTiming {
                hour_angle,
                minute_angle,
                hour,
                minute,
                second,
            } => {
                let mut p = Vec::new();
                put_u16(&mut p, *hour_angle);
                put_u16(&mut p, *minute_angle);
                p.extend([*hour, *minute, *second]);
                set(0xBA, &p)
            }
            HandStep {
                hand,
                clockwise,
                angle,
            } => {
                let mut p = vec![hand.byte(), u8::from(*clockwise)];
                put_u16(&mut p, *angle);
                set(0xB6, &p)
            }

            GetDataCounts => check(0x52, &[0x00]),
            GetSportRecords { .. } => check(0x54, &[0x00, 0x00]),
            GetSleepRecords { .. } => check(0x56, &[0x00]),
            GetHeartRateRecords { .. } => check(0x5B, &[0x00]),
            GetWorkoutCount => check(0x66, &[0x00]),
            GetWorkouts { .. } => check(0x67, &[0x00, 0x00]),
            GetWorkoutHeartRateCount => check(0xBC, &[0x00]),
            GetWorkoutHeartRate { .. } => check(0xBC, &[0x01]),
            DeleteSportData => set(0x53, &[0x00]),
            DeleteSleepData => set(0x55, &[0x00]),
            DeleteHeartRateData => set(0x5A, &[0x00]),
            DeleteWorkoutData => set(0x68, &[0x00]),
            GetSportSleepCount6e { kind } => f6e(
                0x30,
                &[match kind {
                    SportSleepKind::Sport => 0x01,
                    SportSleepKind::Sleep => 0x02,
                }],
            ),
            GetSportData6e => f6e(0x06, &[0x01]),
            GetSleepData6e => f6e(0x31, &[0x01]),

            GetAlarmCounts => check(0x9C, &[0x00]),
            GetAlarms { .. } => check(0x9C, &[0x03]),
            GetFreeAlarmId => check(0x9C, &[0x04]),
            SetAlarm { alarm, is_new } => {
                let mut p = vec![if *is_new { 0x00 } else { 0x01 }, alarm.id];
                p.extend([
                    0x00,
                    0x01,
                    0x04, // type: alarm
                    0x02,
                    0x01,
                    alarm.weekdays,
                    0x04,
                    0x02,
                    alarm.hour,
                    alarm.minute,
                    0x06,
                    0x01,
                    u8::from(alarm.enabled),
                    0x07,
                    0x01,
                    0x07, // ring + vibrate
                    0x08,
                    0x01,
                    alarm.snooze_minutes,
                ]);
                set(0x9C, &p)
            }
            SetReminderLegacy { reminder } => set(
                0x11,
                &[
                    reminder.index,
                    u8::from(reminder.enabled),
                    reminder.kind.legacy_byte(),
                    reminder.hour,
                    reminder.minute,
                    reminder.weekdays,
                ],
            ),
            GetRemindersLegacy => check(0x11, &[0x00]),
            GetReminders6e => f6e(0x20, &[0x01]),
            AddReminder6e { reminder } => f6e(
                0x40,
                &[
                    reminder.kind.byte_6e(),
                    reminder.hour,
                    reminder.minute,
                    reminder.weekdays,
                ],
            ),
            DeleteReminder6e { hour, minute } => f6e(0x09, &[*hour, *minute]),
            DeleteAllReminders6e => f6e(0x21, &[0x01]),

            SetTemperatureUnit { unit } => set(0x22, &[0x01, unit.byte()]),
            PushWeather { days, city, unit } => {
                let mut p = Vec::new();
                for day in days.iter().take(4) {
                    p.extend([
                        unit.byte(),
                        clamp_i8(day.temperature),
                        clamp_i8(day.low),
                        clamp_i8(day.high),
                        day.condition,
                    ]);
                }
                if let Some(city) = city {
                    p.extend_from_slice(utf8_prefix(city, 90));
                }
                set(0x77, &p)
            }
            PushMessageCounts { counts } => {
                let mut p = vec![counts.len() as u8];
                for c in counts {
                    p.extend([c.kind.byte(), c.count.min(255) as u8]);
                }
                set(0x70, &p)
            }
            PushSms { sender, body } => {
                let sender = utf8_char_prefix(sender, 20);
                let body = utf8_char_prefix(body, 200);
                let mut p = vec![sender.len() as u8];
                p.extend(&sender);
                p.push(body.len() as u8);
                p.extend(&body);
                set(0x71, &p)
            }
            PushMusicState {
                playing,
                title_and_artist,
            } => {
                let mut p = vec![0x01, if *playing { 0x00 } else { 0x01 }];
                p.extend(utf8_char_prefix(title_and_artist, 120));
                f6f(0xD0, Action::CheckResponse, &p)
            }
            PushMusicVolume { percent } => f6f(
                0xD0,
                Action::CheckResponse,
                &[0x01, 0x02, (*percent).min(100)],
            ),
            PushMusicStop => f6f(0xD0, Action::CheckResponse, &[0x01, 0xFF]),
            FindPhoneStartAck => remote(0x01, Action::SetResponse, &[0xD2, 0x00]),
            FindPhoneFound => set(0x1A, &[0x10]),
            PhoneGpsStatus { gps_available } => {
                remote(0xE9, Action::CheckResponse, &gps_status(*gps_available))
            }
            PhoneGpsDistance {
                gps_available,
                distance_m,
                speed_kmh,
                confirmations,
            } => {
                let mut p = gps_status(*gps_available);
                p.extend([0x01, 0x04]);
                put_u32(&mut p, *distance_m);
                p.extend([0x02, 0x04]);
                put_u32(&mut p, *speed_kmh);
                p.extend([
                    0x03,
                    0x04,
                    confirm_byte(confirmations.start),
                    confirm_byte(confirmations.pause),
                    confirm_byte(confirmations.resume),
                    confirm_byte(confirmations.stop),
                ]);
                remote(0xE9, Action::CheckResponse, &p)
            }
            PhoneGpsLocation { fix } => {
                let mut p = vec![0x04, 0x14];
                for v in [
                    fix.timestamp,
                    fix.latitude_e7,
                    fix.longitude_e7,
                    fix.altitude_cm,
                    fix.accuracy_cm,
                ] {
                    put_u32(&mut p, v as u32);
                }
                remote(0xE9, Action::CheckResponse, &p)
            }

            GetWatchFaces => check(0x1E, &[0x01]),
            SelectWatchFace { index } => set(0x1E, &[0x02, *index]),
            SetCrownFunctions { functions } => {
                let mut p = vec![functions.len() as u8];
                p.extend(functions.iter().map(|f| f.byte()));
                set(0x2E, &p)
            }
            GetPageOrder { source } => match source {
                PageOrderSource::AppSetting => check(0x2A, &[app_setting::PAGE_QUEUE]),
                PageOrderSource::DriverSlot74e => check(0x0E, &[]),
                PageOrderSource::DriverSlot6cc => check(0x0F, &[]),
                PageOrderSource::DiagnosticDriver74e => {
                    check(0x2A, &[app_setting::DIAGNOSTIC_DRIVER_74E])
                }
            },
            SetPageOrder { pages } => {
                let pages = sanitized_pages(pages);
                let mut p = vec![app_setting::PAGE_QUEUE, pages.len() as u8];
                p.extend(pages);
                set(0x2A, &p)
            }
            GetPageRouteTrace => check(0x2A, &[app_setting::ROUTE_TRACE]),

            GetCapabilities => check(0x2A, &[app_setting::CAPABILITIES]),
            ReadMemory { address, length } => {
                let mut p = vec![app_setting::READ_MEMORY];
                put_u32(&mut p, *address);
                p.push(*length);
                check(0x2A, &p)
            }
            ReadResource {
                handle,
                offset,
                length,
            } => {
                let mut p = vec![app_setting::READ_RESOURCE];
                put_u32(&mut p, *handle);
                put_u32(&mut p, *offset);
                p.push((*length).min(RESOURCE_READ_MAX));
                check(0x2A, &p)
            }
            SetCustomText { text } => {
                let bytes = utf8_prefix(text, CUSTOM_TEXT_MAX_BYTES);
                let mut p = vec![app_setting::CUSTOM_TEXT, bytes.len() as u8];
                p.extend_from_slice(bytes);
                set(0x2A, &p)
            }
            PushCalendarEvents { events } => {
                let shown: Vec<&CalendarEvent> = events.iter().take(calendar::MAX_EVENTS).collect();
                let mut slots: Vec<CalendarEvent> = shown.iter().map(|e| (*e).clone()).collect();
                slots.resize_with(calendar::MAX_EVENTS, empty_event);
                let first = shown.first();
                let row = &mut slots[calendar::WIDGET_SLOT];
                row.line2 = first.map(|e| e.widget_title.clone()).unwrap_or_default();
                row.line3 = first.map(|e| e.widget_subtitle.clone()).unwrap_or_default();
                let mut out: Vec<OutgoingFrame> = slots
                    .iter()
                    .enumerate()
                    .map(|(slot, event)| calendar_slot_frame(slot as u8, event))
                    .collect();
                out.extend(set(
                    0x2A,
                    &[app_setting::CALENDAR_COMMIT, 1, shown.len() as u8],
                ));
                out
            }

            EnterUpgradeMode => set(0x12, &[0x01]),
            EnterUpgradeMode6e {
                mode,
                subtype,
                value,
                label,
            } => {
                let mut p = vec![0u8; 26];
                p[0] = *mode;
                p[1] = *subtype;
                p[2..6].copy_from_slice(&value.to_le_bytes());
                let name = utf8_char_prefix(label, 20);
                p[6..6 + name.len()].copy_from_slice(&name);
                f6e(0x1C, &p)
            }
            RestoreFactory6e => f6e(0x11, &[0x01]),

            Raw { frame, channel, .. } => vec![OutgoingFrame {
                channel: *channel,
                data: frame.clone(),
            }],
        }
    }

    pub fn kind(&self) -> CommandKind {
        use CommandKind as K;
        use Request::*;
        match self {
            BindStart { .. } => K::BindStart,
            BindEnd => K::BindEnd,
            GetWatchId => K::GetWatchId,
            GetDeviceVersion => K::GetDeviceVersion,
            GetDeviceVersionNoArgument => K::GetDeviceVersionNoArgument,
            GetBattery { .. } => K::GetBattery,
            GetDeviceVersion6e => K::GetDeviceVersion6e,
            GetBattery6e => K::GetBattery6e,
            SetDateTime { .. } => K::SetDateTime,
            SetDateTime6e { .. } => K::SetDateTime6e,
            SetUserProfile { .. } => K::SetUserProfile,
            SetUserProfile6e { .. } => K::SetUserProfile6e,
            SetLanguage { .. } => K::SetLanguage,
            SetBrightness { .. } => K::SetBrightness,
            GetBrightness => K::GetBrightness,
            SetScreenTimeout { .. } => K::SetScreenTimeout,
            GetScreenTimeout => K::GetScreenTimeout,
            SetVibration { .. } => K::SetVibration,
            GetVibration => K::GetVibration,
            GetVibrationStrength => K::GetVibrationStrength,
            GetSwitches => K::GetSwitches,
            SetRaiseToWake { .. } => K::SetRaiseToWake,
            SetAutoSleepSwitch { .. } => K::SetAutoSleepSwitch,
            SetUnit { .. } => K::SetUnit,
            GetUnit => K::GetUnit,
            SetUnitsAndFormat6e { .. } => K::SetUnitsAndFormat6e,
            GetUnitsAndFormat6e => K::GetUnitsAndFormat6e,
            SetGoal { .. } => K::SetGoal,
            GetGoals => K::GetGoals,
            SetGoal6e { .. } => K::SetGoal6e,
            GetGoal6e { .. } => K::GetGoal6e,
            SetSleepWindow { .. } => K::SetSleepWindow,
            GetSleepWindow => K::GetSleepWindow,
            SetInactivityAlert6e { .. } => K::SetInactivityAlert6e,
            GetInactivityAlert6e => K::GetInactivityAlert6e,
            SetAutoHeartRate { .. } => K::SetAutoHeartRate,
            GetAutoHeartRate => K::GetAutoHeartRate,
            HandMoveContinuous { .. } => K::HandMoveContinuous,
            HandSyncTime { .. } => K::HandSyncTime,
            HandMachineTiming { .. } => K::HandMachineTiming,
            HandStep { .. } => K::HandStep,
            GetDataCounts => K::GetDataCounts,
            GetSportRecords { .. } => K::GetSportRecords,
            GetSleepRecords { .. } => K::GetSleepRecords,
            GetHeartRateRecords { .. } => K::GetHeartRateRecords,
            GetWorkoutCount => K::GetWorkoutCount,
            GetWorkouts { .. } => K::GetWorkouts,
            GetWorkoutHeartRateCount => K::GetWorkoutHeartRateCount,
            GetWorkoutHeartRate { .. } => K::GetWorkoutHeartRate,
            DeleteSportData => K::DeleteSportData,
            DeleteSleepData => K::DeleteSleepData,
            DeleteHeartRateData => K::DeleteHeartRateData,
            DeleteWorkoutData => K::DeleteWorkoutData,
            GetSportSleepCount6e { .. } => K::GetSportSleepCount6e,
            GetSportData6e => K::GetSportData6e,
            GetSleepData6e => K::GetSleepData6e,
            GetAlarmCounts => K::GetAlarmCounts,
            GetAlarms { .. } => K::GetAlarms,
            GetFreeAlarmId => K::GetFreeAlarmId,
            SetAlarm { .. } => K::SetAlarm,
            SetReminderLegacy { .. } => K::SetReminderLegacy,
            GetRemindersLegacy => K::GetRemindersLegacy,
            GetReminders6e => K::GetReminders6e,
            AddReminder6e { .. } => K::AddReminder6e,
            DeleteReminder6e { .. } => K::DeleteReminder6e,
            DeleteAllReminders6e => K::DeleteAllReminders6e,
            SetTemperatureUnit { .. } => K::SetTemperatureUnit,
            PushWeather { .. } => K::PushWeather,
            PushMessageCounts { .. } => K::PushMessageCounts,
            PushSms { .. } => K::PushSms,
            PushMusicState { .. } => K::PushMusicState,
            PushMusicVolume { .. } => K::PushMusicVolume,
            PushMusicStop => K::PushMusicStop,
            FindPhoneStartAck => K::FindPhoneStartAck,
            FindPhoneFound => K::FindPhoneFound,
            PhoneGpsStatus { .. } => K::PhoneGpsStatus,
            PhoneGpsDistance { .. } => K::PhoneGpsDistance,
            PhoneGpsLocation { .. } => K::PhoneGpsLocation,
            GetWatchFaces => K::GetWatchFaces,
            SelectWatchFace { .. } => K::SelectWatchFace,
            SetCrownFunctions { .. } => K::SetCrownFunctions,
            GetPageOrder { .. } => K::GetPageOrder,
            SetPageOrder { .. } => K::SetPageOrder,
            GetPageRouteTrace => K::GetPageRouteTrace,
            GetCapabilities => K::GetCapabilities,
            ReadMemory { .. } => K::ReadMemory,
            ReadResource { .. } => K::ReadResource,
            SetCustomText { .. } => K::SetCustomText,
            PushCalendarEvents { .. } => K::PushCalendarEvents,
            EnterUpgradeMode => K::EnterUpgradeMode,
            EnterUpgradeMode6e { .. } => K::EnterUpgradeMode6e,
            RestoreFactory6e => K::RestoreFactory6e,
            Raw { .. } => K::Raw,
        }
    }

    /// The reply command code this request waits for (the generic ack
    /// `01 81 [code, status]` also completes it).
    pub fn response_command(&self) -> Option<u8> {
        match self {
            Request::Raw {
                response_command, ..
            } => *response_command,
            Request::GetSportSleepCount6e {
                kind: SportSleepKind::Sleep,
            } => Some(0x15),
            Request::GetPageOrder { source } => Some(match source {
                PageOrderSource::AppSetting => 0x2A,
                PageOrderSource::DriverSlot74e | PageOrderSource::DiagnosticDriver74e => 0xDD,
                PageOrderSource::DriverSlot6cc => 0xE8,
            }),
            other => other.kind().spec().response_command,
        }
    }

    pub fn expects_response(&self) -> bool {
        match self {
            Request::Raw {
                response_command, ..
            } => response_command.is_some(),
            other => other.kind().spec().expects_response,
        }
    }

    /// True when several reply frames make up one response.
    pub fn is_multi_response(&self) -> bool {
        match self {
            Request::GetAlarms { expected } => *expected > 1,
            other => other.kind().spec().multi_response,
        }
    }

    /// How long to wait for the reply, scaled by record counts for syncs.
    pub fn timeout_ms(&self) -> u32 {
        use Request::*;
        let secs = |s: u32| s * 1000;
        match self {
            GetSportRecords { count } => secs(30.max(u32::from(*count) * 2)),
            GetSleepRecords { count } => secs(60.max(u32::from(*count) * 3)),
            GetHeartRateRecords { count } => secs(15.max(u32::from(*count) * 2)),
            GetWorkouts { count } => secs(30.max(u32::from(*count) * 3)),
            GetWorkoutHeartRate { count } => secs(30u32.max(count.saturating_mul(2))),
            GetAlarms { expected } => secs(5.max(u32::from(*expected) * 2)),
            Raw { timeout_ms, .. } => *timeout_ms,
            other => other.kind().spec().timeout_ms,
        }
    }

    /// A short label for logs.
    pub fn label(&self) -> String {
        match self {
            Request::Raw { label, .. } => format!("raw:{label}"),
            other => other.kind().spec().id,
        }
    }
}

/// Keeps page IDs 1…255 and at most ten of them, like the firmware's array.
pub(crate) fn sanitized_pages(pages: &[u8]) -> Vec<u8> {
    pages.iter().copied().filter(|p| *p > 0).take(10).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(r: Request) -> Vec<u8> {
        let frames = r.frames();
        assert_eq!(frames.len(), 1);
        frames[0].data.clone()
    }

    fn time() -> LocalDateTime {
        LocalDateTime {
            year: 2026,
            month: 10,
            day: 11,
            hour: 13,
            minute: 5,
            second: 9,
            utc_offset_minutes: 60,
        }
    }

    #[test]
    fn matches_the_swift_encoders() {
        assert_eq!(
            one(Request::BindStart {
                mode: BindMode::QrCode
            }),
            [0x6F, 0x93, 0x71, 0x01, 0x00, 0x02, 0x8F]
        );
        assert_eq!(
            one(Request::BindStart {
                mode: BindMode::Uid { uid: 0x01020304 }
            }),
            [0x6F, 0x93, 0x71, 0x05, 0x00, 0x00, 0x04, 0x03, 0x02, 0x01, 0x8F]
        );
        assert_eq!(
            one(Request::SetDateTime {
                time: time(),
                analog_hand_calibration: false
            }),
            [0x6F, 0x04, 0x71, 0x0C, 0x00, 0xEA, 0x07, 10, 11, 13, 5, 9, 0, 0, 1, 1, 0, 0x8F]
        );
        let west = LocalDateTime {
            utc_offset_minutes: -330,
            ..time()
        };
        let frame = one(Request::SetDateTime {
            time: west,
            analog_hand_calibration: true,
        });
        assert_eq!(&frame[13..17], &[1, 0, 5, 30]);
        assert_eq!(
            one(Request::SetBrightness { level: 9 }),
            [0x6F, 0x07, 0x71, 0x01, 0x00, 100, 0x8F]
        );
        assert_eq!(one(Request::SetScreenTimeout { seconds: 20 })[5], 15);
        assert_eq!(
            one(Request::SetGoal {
                kind: GoalKind::Steps,
                value: 10_050
            })[5..9],
            [0, 100, 0, 1]
        );
        assert_eq!(
            one(Request::SetUnitsAndFormat6e {
                metric: true,
                use_24h: true,
                date_format: DateFormat::DayMonthYear,
                show_battery: true
            }),
            [0x6E, 0x01, 0x34, 0x4F, 0x8F]
        );
        assert_eq!(
            one(Request::SetUserProfile6e {
                profile: UserProfile {
                    gender: Gender::Male,
                    age: 30,
                    height_cm: 175,
                    weight_tenths_kg: 700,
                }
            }),
            [0x6E, 0x01, 0x0C, 1, 30, 0, 1, 1, 175, 0x58, 0x1B, 0x8F]
        );
        let profile = UserProfile {
            gender: Gender::Male,
            age: 20,
            height_cm: 170,
            weight_tenths_kg: 600,
        };
        assert_eq!(
            one(Request::SetUserProfile { profile }),
            [0x6F, 0x30, 0x71, 0x05, 0x00, 0, 20, 170, 0x58, 0x02, 0x8F]
        );
        let female = UserProfile {
            gender: Gender::Female,
            ..profile
        };
        assert_eq!(one(Request::SetUserProfile { profile: female })[5], 1);
        assert_eq!(
            one(Request::FindPhoneStartAck),
            [0x6F, 0x01, 0x81, 0x02, 0x00, 0xD2, 0x00, 0x8F]
        );
        assert_eq!(
            Request::FindPhoneStartAck.frames()[0].channel,
            WriteChannel::Remote
        );
    }

    #[test]
    fn weather_clamps_temperatures_and_city() {
        let day = WeatherDay {
            temperature: -200,
            low: 5,
            high: 300,
            condition: 2,
        };
        let frame = one(Request::PushWeather {
            days: vec![day; 6],
            city: Some("x".repeat(120)),
            unit: TemperatureUnit::Celsius,
        });
        let payload = &frame[5..frame.len() - 1];
        assert_eq!(payload.len(), 4 * 5 + 90);
        assert_eq!(&payload[..5], &[0, 0x80, 5, 0x7F, 2]);
    }

    #[test]
    fn calendar_pushes_five_slots_and_a_commit() {
        let event = CalendarEvent {
            title: "Standup".into(),
            line1: "Monday 12/10".into(),
            line2: "09:30 - 09:45".into(),
            line3: "15 mins".into(),
            widget_title: "Team standup".into(),
            widget_subtitle: "MON 9:30 15min".into(),
        };
        let frames = Request::PushCalendarEvents {
            events: vec![event],
        }
        .frames();
        assert_eq!(frames.len(), 6);
        assert_eq!(frames[0].data.len(), 6 + 3 + 40 + 72);
        let slot1 = &frames[1].data[5..];
        assert_eq!(&slot1[3 + 40 + 24..3 + 40 + 24 + 12], b"Team standup");
        assert_eq!(
            frames[5].data,
            [0x6F, 0x2A, 0x71, 0x03, 0x00, 0x42, 0x01, 0x01, 0x8F]
        );
    }

    #[test]
    fn phone_gps_frames() {
        let frame = one(Request::PhoneGpsDistance {
            gps_available: true,
            distance_m: 1234,
            speed_kmh: 9,
            confirmations: GpsConfirmations {
                start: true,
                ..Default::default()
            },
        });
        assert_eq!(
            &frame[5..frame.len() - 1],
            &[0, 1, 0, 1, 4, 0xD2, 4, 0, 0, 2, 4, 9, 0, 0, 0, 3, 4, 0, 1, 1, 1]
        );
    }
}
