//! Value types shared by requests, responses and watch events.
//!
//! Wall-clock conversions (time zones, calendars, unit display) stay on the
//! host platform: requests take local date/time fields and responses carry the
//! watch's raw timestamps.

/// A local date and time plus the zone offset the watch should display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct LocalDateTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    /// Offset from UTC in minutes (e.g. 60 for BST, -300 for EST).
    pub utc_offset_minutes: i32,
}

/// How the watch should confirm a bind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BindMode {
    /// `0x02`: the watch shows a QR code and the app scans it. The default.
    QrCode,
    /// `0x01`: the user confirms on the watch.
    ManualConfirm,
    /// `0x00 uid_le32`: rebind with a known user ID.
    Uid { uid: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Gender {
    Female,
    Male,
}

impl Gender {
    /// The Norm Two profile record (6F `0x30`): 0 = male, 1 = female.
    pub(crate) fn byte(self) -> u8 {
        match self {
            Gender::Male => 0,
            Gender::Female => 1,
        }
    }

    /// Android `protocol6E.SetUserInfo`: 0 = female, 1 = male.
    pub(crate) fn byte_6e(self) -> u8 {
        match self {
            Gender::Female => 0,
            Gender::Male => 1,
        }
    }
}

/// The wearer's body profile, which the watch's calorie model reads. The
/// firmware clamps age to 1–100, height to 90–240 cm and weight to
/// 30.0–200.0 kg before using them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct UserProfile {
    pub gender: Gender,
    pub age: u8,
    pub height_cm: u8,
    /// Weight in tenths of a kilogram (705 = 70.5 kg).
    pub weight_tenths_kg: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum DateFormat {
    MonthDayYear,
    DayMonthYear,
    YearMonthDay,
}

/// Goal slots on the 6F `0x50` goal command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum GoalKind {
    /// Steps. Sent in hundreds on the wire, so values round down to 100.
    Steps,
    /// Active calories, kcal.
    Calories,
    /// Distance, km.
    DistanceKm,
    /// Sleep, hours.
    SleepHours,
}

impl GoalKind {
    pub(crate) fn byte(self) -> u8 {
        match self {
            GoalKind::Steps => 0,
            GoalKind::Calories => 1,
            GoalKind::DistanceKm => 2,
            GoalKind::SleepHours => 3,
        }
    }
}

/// Goal types on the 6E `0xA2` goal command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Goal6eKind {
    Calories,
    Distance,
    Steps,
    /// Minutes; the wire carries seconds.
    SleepMinutes,
}

impl Goal6eKind {
    pub(crate) fn byte(self) -> u8 {
        match self {
            Goal6eKind::Calories => 0x81,
            Goal6eKind::Distance => 0x82,
            Goal6eKind::Steps => 0x85,
            Goal6eKind::SleepMinutes => 0x86,
        }
    }
}

/// The preset sleep window (6F `0x58`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct SleepWindow {
    pub start_hour: u8,
    pub start_minute: u8,
    pub end_hour: u8,
    pub end_minute: u8,
    /// Weekday bitmap, bit 0 = Monday … bit 6 = Sunday. `0x7F` is every day.
    pub weekdays: u8,
}

/// The inactivity (move) reminder (6E `0x43`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct InactivityAlert {
    pub enabled: bool,
    /// Weekday bitmap, bit 0 = Monday … bit 6 = Sunday.
    pub weekdays: u8,
    pub interval_minutes: u8,
    pub start_hour: u8,
    pub start_minute: u8,
    pub end_hour: u8,
    pub end_minute: u8,
    pub step_threshold: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum WatchHand {
    Hour,
    Minute,
}

impl WatchHand {
    pub(crate) fn byte(self) -> u8 {
        match self {
            WatchHand::Hour => 0,
            WatchHand::Minute => 1,
        }
    }
}

/// Control codes for continuous hand movement during manual calibration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum HandCalibrationCode {
    Stop,
    ContinuousMove,
    StartManual,
    FinishManual,
    ExitHybridCalibration,
}

impl HandCalibrationCode {
    pub(crate) fn byte(self) -> u8 {
        match self {
            HandCalibrationCode::Stop => 0,
            HandCalibrationCode::ContinuousMove => 1,
            HandCalibrationCode::StartManual => 2,
            HandCalibrationCode::FinishManual => 3,
            HandCalibrationCode::ExitHybridCalibration => 5,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum TemperatureUnit {
    Celsius,
    Fahrenheit,
}

impl TemperatureUnit {
    pub(crate) fn byte(self) -> u8 {
        match self {
            TemperatureUnit::Celsius => 0,
            TemperatureUnit::Fahrenheit => 1,
        }
    }
}

/// One forecast day, already converted to the display unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct WeatherDay {
    /// Current (day 0) or average temperature. Clamped to −128…127.
    pub temperature: i32,
    pub low: i32,
    pub high: i32,
    /// The watch's weather condition code.
    pub condition: u8,
}

/// App category for a notification count push.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum NotificationKind {
    Sms,
    Email,
    Facebook,
    Twitter,
    Whatsapp,
    Instagram,
    Line,
    Kakao,
    Wechat,
    Messenger,
    Telegram,
    Skype,
    Linkedin,
    Other,
}

impl NotificationKind {
    pub(crate) fn byte(self) -> u8 {
        match self {
            NotificationKind::Sms => 0x01,
            NotificationKind::Email => 0x02,
            NotificationKind::Facebook => 0x03,
            NotificationKind::Twitter => 0x04,
            NotificationKind::Whatsapp => 0x05,
            NotificationKind::Instagram => 0x06,
            NotificationKind::Line => 0x07,
            NotificationKind::Kakao => 0x08,
            NotificationKind::Wechat => 0x09,
            NotificationKind::Messenger => 0x0A,
            NotificationKind::Telegram => 0x0B,
            NotificationKind::Skype => 0x0C,
            NotificationKind::Linkedin => 0x0D,
            NotificationKind::Other => 0x10,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct MessageCount {
    pub kind: NotificationKind,
    /// Clamped to 255 on the wire.
    pub count: u32,
}

/// A wake alarm (6F `0x9C`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct Alarm {
    /// The watch's alarm ID (see `GetFreeAlarmId` for new alarms).
    pub id: u8,
    pub enabled: bool,
    pub hour: u8,
    pub minute: u8,
    /// Weekday bitmap, bit 0 = Monday … bit 6 = Sunday. `0` rings once.
    pub weekdays: u8,
    pub snooze_minutes: u8,
}

/// Reminder categories. Legacy 6F and 6E use different numbering; both
/// mappings are handled by the encoders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum ReminderKind {
    Eat,
    Meeting,
    Sleep,
    Medicine,
    Drink,
    Sport,
    Bills,
    Custom,
    /// P03B's "255 Work" reminder.
    Work,
}

impl ReminderKind {
    pub(crate) fn legacy_byte(self) -> u8 {
        match self {
            ReminderKind::Eat => 0,
            ReminderKind::Meeting => 1,
            ReminderKind::Sleep => 3,
            ReminderKind::Medicine => 4,
            ReminderKind::Drink => 5,
            ReminderKind::Sport => 6,
            ReminderKind::Bills => 7,
            ReminderKind::Custom => 8,
            ReminderKind::Work => 9,
        }
    }

    pub(crate) fn from_legacy_byte(byte: u8) -> ReminderKind {
        match byte {
            0 => ReminderKind::Eat,
            1 => ReminderKind::Meeting,
            3 => ReminderKind::Sleep,
            4 => ReminderKind::Medicine,
            5 => ReminderKind::Drink,
            6 => ReminderKind::Sport,
            7 => ReminderKind::Bills,
            9 => ReminderKind::Work,
            _ => ReminderKind::Custom,
        }
    }

    pub(crate) fn byte_6e(self) -> u8 {
        match self {
            ReminderKind::Eat => 0,
            ReminderKind::Meeting => 1,
            ReminderKind::Drink => 2,
            ReminderKind::Sleep => 3,
            ReminderKind::Medicine => 4,
            ReminderKind::Sport => 6,
            ReminderKind::Bills | ReminderKind::Custom => 0,
            ReminderKind::Work => 0xFF,
        }
    }

    pub(crate) fn from_byte_6e(byte: u8) -> ReminderKind {
        match byte {
            0 => ReminderKind::Eat,
            1 => ReminderKind::Meeting,
            2 => ReminderKind::Drink,
            3 => ReminderKind::Sleep,
            4 => ReminderKind::Medicine,
            6 => ReminderKind::Sport,
            0xFF => ReminderKind::Work,
            _ => ReminderKind::Custom,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct Reminder {
    pub index: u8,
    pub enabled: bool,
    pub kind: ReminderKind,
    pub hour: u8,
    pub minute: u8,
    /// Weekday bitmap, bit 0 = Monday … bit 6 = Sunday.
    pub weekdays: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum CrownFunction {
    Timer,
    Stopwatch,
    Weather,
    HeartRate,
    Alarm,
    Settings,
    SecondTimeZone,
    Music,
}

impl CrownFunction {
    pub(crate) fn byte(self) -> u8 {
        match self {
            CrownFunction::Timer => 0x01,
            CrownFunction::Stopwatch => 0x02,
            CrownFunction::Weather => 0x03,
            CrownFunction::HeartRate => 0x04,
            CrownFunction::Alarm => 0x05,
            CrownFunction::Settings => 0x06,
            CrownFunction::SecondTimeZone => 0x07,
            CrownFunction::Music => 0x08,
        }
    }
}

/// One calendar event as the patched calendar page renders it. The phone
/// does all selection and formatting.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CalendarEvent {
    /// Front-card title (up to two 40 px lines) or scroll-row title. ≤39 bytes.
    pub title: String,
    /// Front card: date ("Monday 18/07"). Scroll row: "day start · duration". ≤23 bytes.
    pub line1: String,
    /// Front card: "14:00 - 15:30". Unused on rows. ≤23 bytes.
    pub line2: String,
    /// Front card: "1 hour 30 mins". Unused on rows. ≤23 bytes.
    pub line3: String,
    /// Home-widget title. Only the first event's is sent; empty uses `title`.
    pub widget_title: String,
    /// Compact widget sub-line ("MON 24/06 6pm 30min"). First event only.
    pub widget_subtitle: String,
}

/// Where to read the carousel page order from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum PageOrderSource {
    /// Official app path: AppSetting `0x2A` type `0x01`.
    AppSetting,
    /// Firmware driver slot `0x0E`, replies on `0xDD` from `app_ctx + 0x74E`.
    DriverSlot74e,
    /// Firmware driver slot `0x0F`, replies on `0xE8` from `app_ctx + 0x6CC`.
    DriverSlot6cc,
    /// Patched diagnostic: AppSetting type `0xFE`, replies on `0xDD`.
    DiagnosticDriver74e,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum SportSleepKind {
    Sport,
    Sleep,
}

/// Which phone-GPS workout transitions the phone has confirmed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, uniffi::Record)]
pub struct GpsConfirmations {
    pub start: bool,
    pub pause: bool,
    pub resume: bool,
    pub stop: bool,
}

/// A phone GPS fix, in the watch's fixed-point units.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct GpsFix {
    pub timestamp: i64,
    pub latitude_e7: i64,
    pub longitude_e7: i64,
    pub altitude_cm: i64,
    pub accuracy_cm: i64,
}

// ---- response values -------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct BatteryStatus {
    /// 0–100.
    pub level: u8,
    /// Bit 7 of the new-mode reply; always false in old mode.
    pub charging: bool,
}

/// The 20-character version string, e.g. `A0.2R0.4T0.0H0.0B0.1`.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FirmwareVersion {
    pub raw: String,
    /// Five 4-character segments when the string has the standard shape.
    pub segments: Vec<String>,
    /// The Apollo main-firmware segment (`B…`), which patch builds change.
    pub apollo_tag: Option<String>,
    /// False for the stock `B0.1` / `B1.0` tags.
    pub is_patched: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct Goals {
    pub steps: u32,
    pub calories: u32,
    pub distance_km: u32,
    pub sleep_hours: u32,
    /// Present when the watch sends the fifth (active minutes) slot.
    pub sport_minutes: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct VibrationSettings {
    pub enabled: bool,
    pub screen_speed: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct SwitchStates {
    /// Byte 1, bit `0x40`. `None` when the reply is too short.
    pub raise_to_wake: Option<bool>,
    /// Byte 0, bit `0x08`.
    pub auto_sleep: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct DataCounts {
    pub sport: u16,
    pub sleep: u16,
    pub heart_rate: u16,
}

/// One activity record (6F `0x54`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct SportRecord {
    pub index: u16,
    /// Seconds, in the watch's local-time epoch.
    pub timestamp: i64,
    pub steps: u32,
    /// Total calories in milli-kcal.
    pub calories: u32,
    /// Static (BMR) calories in milli-kcal.
    pub static_calories: u32,
    pub distance_m: u32,
    pub duration_min: u32,
}

/// One sleep state change (6F `0x56`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct SleepPoint {
    pub index: u16,
    pub timestamp: i64,
    /// Exactly what the watch sent.
    pub raw_status: u8,
    /// 0 deep, 1 light, 2 awake, 3 ready-to-sleep, 4 exit-sleep, 5 REM,
    /// 16 begin, 17 end (raw 18 is normalised to 17).
    pub status: u8,
}

/// One stored heart-rate sample (6F `0x5B`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct HeartRateSample {
    pub index: u16,
    pub timestamp: i64,
    pub bpm: u8,
}

/// One workout summary (6F `0x67`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct WorkoutRecord {
    pub index: u16,
    pub start_timestamp: i64,
    pub end_timestamp: i64,
    pub workout_type: u32,
    pub steps: u32,
    pub calories: u32,
    pub distance_m: u32,
    pub duration_seconds: u32,
    pub average_heart_rate: u32,
    pub pace: u32,
    pub speed: u32,
    pub lap_count: u32,
    pub zone_warm_up: u32,
    pub zone_fat_burning: u32,
    pub zone_aerobic: u32,
    pub zone_anaerobic: u32,
    pub zone_limit: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct WorkoutHeartRateSample {
    pub timestamp: i64,
    pub bpm: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct AlarmCounts {
    pub reminders: u8,
    pub alarms: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct WatchFace {
    pub index: u8,
    pub id: u8,
    pub is_current: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct AutoHeartRate {
    pub enabled: bool,
    pub interval_minutes: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct UnitsAndFormat {
    pub metric: bool,
    pub use_24h: bool,
}

/// The patched firmware's feature bitmask (AppSetting `0x2A` sub-type `0x43`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Capabilities {
    pub raw: u32,
    pub labels: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct PageRouteRecord {
    pub counter: u8,
    pub site_id: u8,
    pub event_id: u16,
    pub column1_resource: u8,
    pub column2_resource: u8,
    pub create_resource: u8,
    pub pointer_low_byte: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PageRouteTrace {
    pub firmware_counter: u8,
    pub latest: PageRouteRecord,
    pub records: Vec<PageRouteRecord>,
}
