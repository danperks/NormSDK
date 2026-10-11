//! The annotated command catalog.
//!
//! One [`CommandSpec`] per [`CommandKind`]: what the command does, its exact
//! frame layout, what the watch answers, where the knowledge came from, and
//! whether the stock firmware handles it. `docs/COMMANDS.md` is generated from
//! this table (`cargo xtask docs`), merged with the replies recorded from the
//! emulated stock firmware (`tests/emulator`).

use crate::frame::{Action, FrameFormat, WriteChannel};
use crate::model::*;
use crate::request::Request;

/// 6F command codes with a handler in the stock firmware's command table
/// (`g_ew_ble_protocol_commands`, 0x57 entries, firmware `F0.2B01`).
/// Codes outside this list are answered `01 81 [code, 02]`.
pub const STOCK_6F_HANDLERS: [u8; 87] = [
    0x01, 0x02, 0x03, 0x04, 0x05, 0x07, 0x08, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x10, 0x13, 0x14, 0x15,
    0x17, 0x18, 0x19, 0x1A, 0x1E, 0x22, 0x25, 0x29, 0x2A, 0x30, 0x31, 0x50, 0x51, 0x52, 0x53, 0x54,
    0x55, 0x56, 0x58, 0x59, 0x5A, 0x5B, 0x5C, 0x5D, 0x5E, 0x5F, 0x60, 0x61, 0x66, 0x67, 0x68, 0x72,
    0x76, 0x77, 0x79, 0x90, 0x91, 0x93, 0x94, 0x98, 0x99, 0x9A, 0x9C, 0xB6, 0xB8, 0xB9, 0xBA, 0xD0,
    0xD2, 0xD7, 0xDA, 0xDB, 0xE2, 0xE9, 0xED, 0xEE, 0xEF, 0xF0, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7, 0xF8,
    0xF9, 0xFA, 0xFB, 0xFC, 0xFD, 0xFE, 0xFF,
];

/// Where a command path comes from. Mirrors the evidence labels used in the
/// NormConnect iOS app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Evidence {
    /// Seen in the official iOS app, or confirmed on hardware via that path.
    IosObserved,
    /// Android `MBluetooth6E` / `protocol6E`.
    Android6e,
    /// Android `MBluetooth` / `protocol` (6F).
    AndroidLegacy6f,
    /// Defined by the NormConnect rewrite.
    Rewrite,
    /// Only exists in NormCrack patched firmware.
    FirmwarePatch,
}

/// How the stock firmware treats the command.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum FirmwareSupport {
    /// The stock firmware has a handler for this 6F code.
    Stock,
    /// 6F code with no stock handler: the watch answers `01 81 [code, 02]`.
    NoStockHandler,
    /// 6E frame. The Norm Two's receive path only parses 6F, so stock
    /// firmware ignores it; kept for Android parity and other P03B builds.
    Ignored6e,
    /// Needs a NormCrack patched firmware feature.
    Patched { feature: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Category {
    Pairing,
    Time,
    Settings,
    Calibration,
    Health,
    Alarms,
    Weather,
    Notifications,
    Media,
    Remote,
    WatchFace,
    PatchedFirmware,
    System,
}

/// Every command the SDK can send, without its parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum CommandKind {
    BindStart,
    BindEnd,
    GetWatchId,
    GetDeviceVersion,
    GetDeviceVersionNoArgument,
    GetBattery,
    GetDeviceVersion6e,
    GetBattery6e,
    SetDateTime,
    SetDateTime6e,
    SetUserProfile,
    SetUserProfile6e,
    SetLanguage,
    SetBrightness,
    GetBrightness,
    SetScreenTimeout,
    GetScreenTimeout,
    SetVibration,
    GetVibration,
    GetVibrationStrength,
    GetSwitches,
    SetRaiseToWake,
    SetAutoSleepSwitch,
    SetUnit,
    GetUnit,
    SetUnitsAndFormat6e,
    GetUnitsAndFormat6e,
    SetGoal,
    GetGoals,
    SetGoal6e,
    GetGoal6e,
    SetSleepWindow,
    GetSleepWindow,
    SetInactivityAlert6e,
    GetInactivityAlert6e,
    SetAutoHeartRate,
    GetAutoHeartRate,
    HandMoveContinuous,
    HandSyncTime,
    HandMachineTiming,
    HandStep,
    GetDataCounts,
    GetSportRecords,
    GetSleepRecords,
    GetHeartRateRecords,
    GetWorkoutCount,
    GetWorkouts,
    GetWorkoutHeartRateCount,
    GetWorkoutHeartRate,
    DeleteSportData,
    DeleteSleepData,
    DeleteHeartRateData,
    DeleteWorkoutData,
    GetSportSleepCount6e,
    GetSportData6e,
    GetSleepData6e,
    GetAlarmCounts,
    GetAlarms,
    GetFreeAlarmId,
    SetAlarm,
    SetReminderLegacy,
    GetRemindersLegacy,
    GetReminders6e,
    AddReminder6e,
    DeleteReminder6e,
    DeleteAllReminders6e,
    SetTemperatureUnit,
    PushWeather,
    PushMessageCounts,
    PushSms,
    PushMusicState,
    PushMusicVolume,
    PushMusicStop,
    FindPhoneStartAck,
    FindPhoneFound,
    PhoneGpsStatus,
    PhoneGpsDistance,
    PhoneGpsLocation,
    GetWatchFaces,
    SelectWatchFace,
    SetCrownFunctions,
    GetPageOrder,
    SetPageOrder,
    GetPageRouteTrace,
    GetCapabilities,
    ReadMemory,
    ReadResource,
    SetCustomText,
    PushCalendarEvents,
    EnterUpgradeMode,
    EnterUpgradeMode6e,
    RestoreFactory6e,
    Raw,
}

impl CommandKind {
    pub const ALL: [CommandKind; 93] = {
        use CommandKind::*;
        [
            BindStart,
            BindEnd,
            GetWatchId,
            GetDeviceVersion,
            GetDeviceVersionNoArgument,
            GetBattery,
            GetDeviceVersion6e,
            GetBattery6e,
            SetDateTime,
            SetDateTime6e,
            SetUserProfile,
            SetUserProfile6e,
            SetLanguage,
            SetBrightness,
            GetBrightness,
            SetScreenTimeout,
            GetScreenTimeout,
            SetVibration,
            GetVibration,
            GetVibrationStrength,
            GetSwitches,
            SetRaiseToWake,
            SetAutoSleepSwitch,
            SetUnit,
            GetUnit,
            SetUnitsAndFormat6e,
            GetUnitsAndFormat6e,
            SetGoal,
            GetGoals,
            SetGoal6e,
            GetGoal6e,
            SetSleepWindow,
            GetSleepWindow,
            SetInactivityAlert6e,
            GetInactivityAlert6e,
            SetAutoHeartRate,
            GetAutoHeartRate,
            HandMoveContinuous,
            HandSyncTime,
            HandMachineTiming,
            HandStep,
            GetDataCounts,
            GetSportRecords,
            GetSleepRecords,
            GetHeartRateRecords,
            GetWorkoutCount,
            GetWorkouts,
            GetWorkoutHeartRateCount,
            GetWorkoutHeartRate,
            DeleteSportData,
            DeleteSleepData,
            DeleteHeartRateData,
            DeleteWorkoutData,
            GetSportSleepCount6e,
            GetSportData6e,
            GetSleepData6e,
            GetAlarmCounts,
            GetAlarms,
            GetFreeAlarmId,
            SetAlarm,
            SetReminderLegacy,
            GetRemindersLegacy,
            GetReminders6e,
            AddReminder6e,
            DeleteReminder6e,
            DeleteAllReminders6e,
            SetTemperatureUnit,
            PushWeather,
            PushMessageCounts,
            PushSms,
            PushMusicState,
            PushMusicVolume,
            PushMusicStop,
            FindPhoneStartAck,
            FindPhoneFound,
            PhoneGpsStatus,
            PhoneGpsDistance,
            PhoneGpsLocation,
            GetWatchFaces,
            SelectWatchFace,
            SetCrownFunctions,
            GetPageOrder,
            SetPageOrder,
            GetPageRouteTrace,
            GetCapabilities,
            ReadMemory,
            ReadResource,
            SetCustomText,
            PushCalendarEvents,
            EnterUpgradeMode,
            EnterUpgradeMode6e,
            RestoreFactory6e,
            Raw,
        ]
    };
}

/// The annotation for one command.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CommandSpec {
    pub kind: CommandKind,
    /// Stable dotted identifier, e.g. `battery.get`.
    pub id: String,
    pub title: String,
    pub category: Category,
    /// What the command does and when to use it.
    pub summary: String,
    pub format: FrameFormat,
    pub command: u8,
    /// `None` for 6E frames, which have no action byte.
    pub action: Option<Action>,
    pub write_channel: WriteChannel,
    pub expects_response: bool,
    /// Reply code that completes the request. The generic ack
    /// `01 81 [code, status]` for this code completes it too.
    pub response_command: Option<u8>,
    /// Other reply codes that also complete it (e.g. stock firmware's empty
    /// `0x2A` echo where patched firmware answers `0xDC`).
    pub alternate_response_commands: Vec<u8>,
    pub multi_response: bool,
    /// Base timeout; record syncs scale it with the expected count.
    pub timeout_ms: u32,
    /// Payload layout, bytes in order.
    pub request_layout: String,
    pub response_layout: String,
    pub evidence: Evidence,
    pub firmware: FirmwareSupport,
    pub notes: String,
}

struct B(CommandSpec, Option<String>);

fn spec(
    kind: CommandKind,
    id: &str,
    title: &str,
    category: Category,
    format: FrameFormat,
    command: u8,
    action: Option<Action>,
) -> B {
    let (expects, response, timeout) = match (format, action) {
        (FrameFormat::Frame6f, Some(Action::Check)) => (true, Some(command), 10_000),
        (FrameFormat::Frame6f, Some(_)) => (true, Some(command), 5_000),
        _ => (false, None, 10_000),
    };
    B(
        CommandSpec {
            kind,
            id: id.into(),
            title: title.into(),
            category,
            summary: String::new(),
            format,
            command,
            action,
            write_channel: WriteChannel::Primary,
            expects_response: expects,
            response_command: response,
            alternate_response_commands: Vec::new(),
            multi_response: false,
            timeout_ms: timeout,
            request_layout: String::new(),
            response_layout: String::new(),
            evidence: Evidence::Rewrite,
            firmware: FirmwareSupport::Stock,
            notes: String::new(),
        },
        None,
    )
}

fn get(kind: CommandKind, id: &str, title: &str, category: Category, command: u8) -> B {
    spec(
        kind,
        id,
        title,
        category,
        FrameFormat::Frame6f,
        command,
        Some(Action::Check),
    )
}

fn set(kind: CommandKind, id: &str, title: &str, category: Category, command: u8) -> B {
    spec(
        kind,
        id,
        title,
        category,
        FrameFormat::Frame6f,
        command,
        Some(Action::Set),
    )
    .response("`01 81 [code, status]` ack; status 0 = applied.")
}

fn e6(kind: CommandKind, id: &str, title: &str, category: Category, command: u8) -> B {
    spec(
        kind,
        id,
        title,
        category,
        FrameFormat::Frame6e,
        command,
        None,
    )
}

impl B {
    fn summary(mut self, s: &str) -> Self {
        self.0.summary = s.into();
        self
    }
    fn request(mut self, s: &str) -> Self {
        self.0.request_layout = s.into();
        self
    }
    fn response(mut self, s: &str) -> Self {
        self.0.response_layout = s.into();
        self
    }
    fn evidence(mut self, e: Evidence) -> Self {
        self.0.evidence = e;
        self
    }
    fn notes(mut self, s: &str) -> Self {
        self.0.notes = s.into();
        self
    }
    fn reply(mut self, code: u8) -> Self {
        self.0.expects_response = true;
        self.0.response_command = Some(code);
        self
    }
    fn alternates(mut self, codes: &[u8]) -> Self {
        self.0.alternate_response_commands = codes.to_vec();
        self
    }
    fn write_only(mut self) -> Self {
        self.0.expects_response = false;
        self.0.response_command = None;
        self
    }
    fn multi(mut self) -> Self {
        self.0.multi_response = true;
        self
    }
    fn timeout(mut self, ms: u32) -> Self {
        self.0.timeout_ms = ms;
        self
    }
    fn remote(mut self) -> Self {
        self.0.write_channel = WriteChannel::Remote;
        self
    }
    fn patched(mut self, feature: &str) -> Self {
        self.1 = Some(feature.into());
        self
    }
    fn build(mut self) -> CommandSpec {
        self.0.firmware = match (self.1, self.0.format) {
            (Some(feature), _) => FirmwareSupport::Patched { feature },
            (None, FrameFormat::Frame6e) => FirmwareSupport::Ignored6e,
            (None, FrameFormat::Frame6f) if STOCK_6F_HANDLERS.contains(&self.0.command) => {
                FirmwareSupport::Stock
            }
            (None, FrameFormat::Frame6f) => FirmwareSupport::NoStockHandler,
        };
        if self.0.response_layout.is_empty() && !self.0.expects_response {
            self.0.response_layout = "None expected.".into();
        }
        self.0
    }
}

impl CommandKind {
    /// The annotation for this command.
    pub fn spec(self) -> CommandSpec {
        use Category::*;
        use CommandKind as K;
        use Evidence::*;
        let b = match self {
            // ---- pairing and identity ----------------------------------------
            K::BindStart => set(self, "bind.start", "Start binding", Pairing, 0x93)
                .summary("Starts pairing. In QR mode the watch shows a QR code for the app to scan.")
                .request("`[mode]`: `02` QR code (default), `01` confirm on watch; or `00 uid_le32` to rebind a known user.")
                .response("`01 81 [93, status]` ack, or `93` with a little-endian user ID.")
                .timeout(30_000)
                .evidence(AndroidLegacy6f),
            K::BindEnd => set(self, "bind.end", "Finish binding", Pairing, 0x94)
                .summary("Completes pairing; the watch leaves the bind screen.")
                .request("`[01]`")
                .response("`01 81 [94, status]` ack, or `94 [flag]` where 1 = first bind.")
                .evidence(AndroidLegacy6f),
            K::GetWatchId => get(self, "identity.watch_id", "Read watch ID", Pairing, 0x02)
                .summary("Reads the watch's identifier.")
                .request("`[00]`")
                .response("ASCII ID, e.g. `FCP03A18050801000000`; non-printable IDs are shown as hex.")
                .evidence(IosObserved)
                .notes("The stock firmware drops 6F frames with an empty payload, so the empty request the iOS app sent is never answered; `[00]` (or `[01]`) is."),
            K::GetDeviceVersion => get(self, "identity.version", "Read firmware version", Pairing, 0x03)
                .summary("Reads the 20-character version string; the `B` segment identifies patched builds.")
                .request("`[01]`")
                .response("`[01] ASCII version`, e.g. `01 'A0.2R0.4T0.0H0.0B0.1'`. A leading byte ≤ 06 is dropped.")
                .timeout(8_000)
                .evidence(IosObserved)
                .notes("After an OTA the watch can take several seconds to answer."),
            K::GetDeviceVersionNoArgument => get(self, "identity.version_no_arg", "Read firmware version (no argument)", Pairing, 0x03)
                .summary("Version read with an empty payload, as the iOS app's OTA check sends it.")
                .request("Empty.")
                .response("Same as `identity.version`.")
                .evidence(IosObserved)
                .notes("Stock firmware drops empty-payload frames, so this is never answered there. Use `identity.version`."),
            K::GetBattery => get(self, "battery.get", "Read battery", Pairing, 0x08)
                .summary("Reads the battery level and, in new mode, the charging flag.")
                .request("`[01]` new mode (default) or `[00]` old mode.")
                .response("`[byte]`: new mode bit 7 = charging, bits 0–6 = level; old mode = level.")
                .evidence(IosObserved),
            K::GetDeviceVersion6e => e6(self, "identity.version_6e", "Read firmware version (6E)", Pairing, 0x03)
                .summary("Android P03B version read.")
                .request("`[03]`")
                .response("6E `0x09`: version bytes, shown dotted (e.g. `0.2.1`).")
                .reply(0x09)
                .evidence(Android6e),
            K::GetBattery6e => e6(self, "battery.get_6e", "Read battery (6E)", Pairing, 0x0F)
                .summary("Android P03B battery read.")
                .request("`[01]`")
                .response("6E `0x00`: `[level | charging<<7]`.")
                .reply(0x00)
                .evidence(Android6e),

            // ---- time and profile ----------------------------------------------
            K::SetDateTime => set(self, "time.set", "Set date and time", Time, 0x04)
                .summary("Sets the watch clock and time zone. With hand calibration set, the analogue movement restarts from its calibrated position.")
                .request("`year_le16 month day hour minute second 00 calib sign tz_hour tz_minute` (sign 1 = east of UTC).")
                .timeout(3_000)
                .evidence(AndroidLegacy6f)
                .notes("Hardware-probed: Norm Two accepts the Android 12-byte payload. A check (`70 [00]`) reads back `year_le16 month day hour minute second`."),
            K::SetDateTime6e => e6(self, "time.set_6e", "Set date and time (6E)", Time, 0x15)
                .summary("Android `protocol6E.DateTime`.")
                .request("`year_le16 month day hour minute second`")
                .evidence(Android6e),
            K::SetUserProfile => set(self, "profile.set", "Set body profile", Time, 0x05)
                .summary("Sets gender, age, height and weight for calorie and distance estimates.")
                .request("`gender age height_cm weight_kg_le16` (gender 0 = female, 1 = male).")
                .evidence(AndroidLegacy6f),
            K::SetUserProfile6e => e6(self, "profile.set_6e", "Set body profile (6E)", Time, 0x0C)
                .summary("Android `protocol6E.SetUserInfo`.")
                .request("`gender age_le16 01 01 height_cm weight_x100_le16`")
                .evidence(Android6e),
            K::SetLanguage => set(self, "settings.language", "Set language", Settings, 0x09)
                .summary("Sets the UI language during binding.")
                .request("`[language]`")
                .evidence(IosObserved)
                .notes("No stock handler: the emulated firmware answers `01 81 [09, 02]`. Language is chosen on the watch."),

            // ---- display and device settings -----------------------------------
            K::SetBrightness => set(self, "settings.brightness.set", "Set brightness", Settings, 0x07)
                .summary("Sets screen brightness, levels 1–5.")
                .request("`[level × 20]` (20, 40, 60, 80, 100).")
                .evidence(AndroidLegacy6f)
                .notes("Hardware-probed on P03B; the 6E `0x58` path is not answered."),
            K::GetBrightness => get(self, "settings.brightness.get", "Read brightness", Settings, 0x07)
                .request("`[00]`")
                .response("`[percent]` (or `[x, percent]`); level = percent / 20.")
                .evidence(AndroidLegacy6f),
            K::SetScreenTimeout => set(self, "settings.screen_timeout.set", "Set screen timeout", Settings, 0x13)
                .summary("Sets how long the screen stays on. Snapped to 5, 15, 30 or 60 seconds.")
                .request("`[seconds]`")
                .evidence(IosObserved),
            K::GetScreenTimeout => get(self, "settings.screen_timeout.get", "Read screen timeout", Settings, 0x13)
                .request("`[00]`")
                .response("`[seconds]`")
                .evidence(IosObserved),
            K::SetVibration => set(self, "settings.vibration.set", "Set vibration", Settings, 0xF0)
                .summary("Turns vibration on or off and sets the screen speed (2 = weak, 3 = strong in the iOS UI).")
                .request("`[speed enabled 01]`")
                .evidence(AndroidLegacy6f)
                .notes("The emulated stock firmware answers `01 81 [F0, 01]` (rejected) to both read and write."),
            K::GetVibration => get(self, "settings.vibration.get", "Read vibration", Settings, 0xF0)
                .request("`[00]`")
                .response("`[speed enabled …]`")
                .timeout(4_000)
                .evidence(AndroidLegacy6f),
            K::GetVibrationStrength => get(self, "settings.vibration_strength.get", "Read vibration strength", Settings, 0x10)
                .request("`[00]`")
                .response("`[strength]`")
                .evidence(IosObserved),
            K::GetSwitches => get(self, "settings.switches.get", "Read switch bitmap", Settings, 0x90)
                .summary("Reads the legacy switch bitmap that holds raise-to-wake and auto-sleep.")
                .request("`[00]`")
                .response("Bitmap: byte 0 bit `0x08` = auto sleep, byte 1 bit `0x40` = raise to wake.")
                .evidence(AndroidLegacy6f),
            K::SetRaiseToWake => set(self, "settings.raise_to_wake.set", "Set raise to wake", Settings, 0x90)
                .summary("Turns lift-to-wake on or off.")
                .request("`01 0E [enabled]`")
                .evidence(AndroidLegacy6f)
                .notes("Hardware-probed: the 6F switch path works; Android's 6E `0x57` wrapper times out."),
            K::SetAutoSleepSwitch => set(self, "settings.auto_sleep.set", "Set auto sleep", Settings, 0x90)
                .summary("Turns automatic sleep detection on or off.")
                .request("`01 03 [enabled]`")
                .evidence(AndroidLegacy6f),
            K::SetUnit => set(self, "settings.unit.set", "Set distance unit", Settings, 0x0C)
                .summary("Metric or imperial.")
                .request("`[00]` metric, `[01]` imperial.")
                .evidence(AndroidLegacy6f)
                .notes("Hardware-confirmed P03B path; the combined 6E `0x34` read is not answered."),
            K::GetUnit => get(self, "settings.unit.get", "Read distance unit", Settings, 0x0C)
                .request("`[00]`")
                .response("`[unit]` or `[0C, unit]`; 0 = metric.")
                .timeout(4_000)
                .evidence(AndroidLegacy6f),
            K::SetUnitsAndFormat6e => e6(self, "settings.format.set_6e", "Set units and formats (6E)", Settings, 0x34)
                .summary("Packs time format, units, date format and battery display into one byte.")
                .request("`[bits]`: 0 = 24 h, 1 = metric, 2 = custom date, 3–4 = date format, 6 = show battery %.")
                .evidence(Android6e),
            K::GetUnitsAndFormat6e => e6(self, "settings.format.get_6e", "Read units and formats (6E)", Settings, 0x34)
                .request("`[FF]`")
                .response("6E `0x34`: last byte uses the same bits.")
                .reply(0x34)
                .timeout(4_000)
                .evidence(Android6e),
            K::SetGoal => set(self, "goals.set", "Set a goal", Settings, 0x50)
                .summary("Sets one daily goal.")
                .request("`kind value_le16 01`: kind 0 steps (value ÷ 100), 1 kcal, 2 km, 3 sleep hours.")
                .evidence(AndroidLegacy6f),
            K::GetGoals => get(self, "goals.get", "Read goals", Settings, 0x50)
                .request("`[00]`")
                .response("Triples `value_le16 flag` for steps (×100), kcal, km, sleep hours and, when present, active minutes.")
                .evidence(AndroidLegacy6f),
            K::SetGoal6e => e6(self, "goals.set_6e", "Set a goal (6E)", Settings, 0xA2)
                .request("`type value_le32`: 81 kcal, 82 distance, 85 steps, 86 sleep (seconds).")
                .evidence(Android6e),
            K::GetGoal6e => e6(self, "goals.get_6e", "Read a goal (6E)", Settings, 0xA2)
                .request("`type 00 00 00 00`")
                .response("6E `0xA2`: `type value_le32`.")
                .reply(0xA2)
                .evidence(Android6e),
            K::SetSleepWindow => set(self, "sleep.window.set", "Set sleep window", Settings, 0x58)
                .summary("Sets the preset bedtime and wake time.")
                .request("`start_h start_m end_h end_m weekdays`")
                .evidence(AndroidLegacy6f)
                .notes("Hardware-probed; the Android 6E `0x36` setter is not used by Norm Two."),
            K::GetSleepWindow => get(self, "sleep.window.get", "Read sleep window", Settings, 0x58)
                .request("`[00]`")
                .response("`start_h start_m end_h end_m weekdays`")
                .evidence(AndroidLegacy6f),
            K::SetInactivityAlert6e => e6(self, "inactivity.set_6e", "Set inactivity alert (6E)", Settings, 0x43)
                .summary("Move reminder: interval, active hours and step threshold.")
                .request("`01 cycle interval start_h start_m end_h end_m steps_le16`; cycle bit 7 = enabled, bits 0–6 = weekdays.")
                .evidence(Android6e),
            K::GetInactivityAlert6e => e6(self, "inactivity.get_6e", "Read inactivity alert (6E)", Settings, 0x43)
                .request("Nine zero bytes.")
                .response("6E `0x43`: `cycle interval start_h start_m end_h end_m …`.")
                .reply(0x43)
                .evidence(Android6e),
            K::SetAutoHeartRate => set(self, "heart_rate.auto.set", "Set automatic heart rate", Settings, 0x5C)
                .summary("Sets the background heart-rate interval; 0 disables it.")
                .request("`[minutes]`")
                .evidence(AndroidLegacy6f)
                .notes("Android's 6E setter is a stub; the official iOS app changes this, and the stock firmware handles `0x5C`."),
            K::GetAutoHeartRate => get(self, "heart_rate.auto.get", "Read automatic heart rate", Settings, 0x5C)
                .request("`[00]`")
                .response("`[minutes]`; 0 = off.")
                .evidence(AndroidLegacy6f),

            // ---- hand calibration -----------------------------------------------
            K::HandMoveContinuous => set(self, "hands.move_continuous", "Move a hand continuously", Calibration, 0xB8)
                .summary("Manual calibration: start, stop or finish moving a hand (Android `watchMoveKeep`).")
                .request("`hand clockwise code`: hand 0 hour / 1 minute; code 0 stop, 1 move, 2 start manual, 3 finish, 5 exit.")
                .evidence(AndroidLegacy6f),
            K::HandSyncTime => set(self, "hands.sync_time", "Restart hands at a time", Calibration, 0xB8)
                .summary("Extended `0xB8` that restarts the movement after manual calibration (iOS `sendClockSyncTime`).")
                .request("`01 01 04 hour minute second finish`")
                .evidence(IosObserved),
            K::HandMachineTiming => set(self, "hands.machine_timing", "Map hand angles to time", Calibration, 0xBA)
                .summary("Camera calibration: maps calibrated hand angles to the current time.")
                .request("`hour_angle_le16 minute_angle_le16 hour minute second`")
                .evidence(AndroidLegacy6f),
            K::HandStep => set(self, "hands.step", "Step a hand", Calibration, 0xB6)
                .summary("Moves one hand by an angle (Android `watchMoveOne`).")
                .request("`hand clockwise angle_le16`")
                .evidence(AndroidLegacy6f),

            // ---- health data ------------------------------------------------------
            K::GetDataCounts => get(self, "sync.counts", "Read record counts", Health, 0x52)
                .summary("How many sport, sleep and heart-rate records are waiting.")
                .request("`[00]`")
                .response("`sport_le16 sleep_le16 [hr_le16 …]`")
                .evidence(Rewrite),
            K::GetSportRecords => get(self, "sync.sport", "Fetch activity records", Health, 0x54)
                .summary("Streams `count` activity records.")
                .request("`[00 00]`")
                .response("One frame per record: `index_le16 ts_le32 steps_le32 [kcal_milli_le32 dist_m_le32 minutes_le32 … static_kcal_milli_le32]`.")
                .multi()
                .timeout(30_000)
                .evidence(Rewrite)
                .notes("With no records the watch answers `01 81 [54, 01]`."),
            K::GetSleepRecords => get(self, "sync.sleep", "Fetch sleep records", Health, 0x56)
                .summary("Streams `count` sleep state changes.")
                .request("`[00]`")
                .response("One frame per point: `index_le16 ts_le32 status`.")
                .multi()
                .timeout(60_000)
                .evidence(Rewrite),
            K::GetHeartRateRecords => get(self, "sync.heart_rate", "Fetch heart-rate records", Health, 0x5B)
                .summary("Streams `count` stored heart-rate samples.")
                .request("`[00]`")
                .response("One frame per sample: `index_le16 ts_le32 bpm`.")
                .multi()
                .timeout(15_000)
                .evidence(Rewrite),
            K::GetWorkoutCount => get(self, "sync.workout_count", "Read workout count", Health, 0x66)
                .request("`[00]`")
                .response("`count_le16`")
                .evidence(AndroidLegacy6f),
            K::GetWorkouts => get(self, "sync.workouts", "Fetch workouts", Health, 0x67)
                .summary("Streams `count` workout summaries (fixed layout, or TLV on some builds).")
                .request("`[00 00]`")
                .response("Fixed: `index_le16 start_le32 steps kcal dist duration avg_hr end_le32 type [pace speed laps zones×5]` (u32 LE); or TLV tags 00–0E.")
                .multi()
                .timeout(30_000)
                .evidence(AndroidLegacy6f),
            K::GetWorkoutHeartRateCount => get(self, "sync.workout_hr_count", "Read workout heart-rate detail count", Health, 0xBC)
                .request("`[00]`")
                .response("`count_le32`, or an ack with a non-zero status when unavailable.")
                .evidence(AndroidLegacy6f),
            K::GetWorkoutHeartRate => get(self, "sync.workout_hr", "Fetch workout heart-rate detail", Health, 0xBC)
                .summary("Streams per-workout heart-rate detail as varint key/value pairs.")
                .request("`[01]`")
                .response("Varint pairs: 0 abs ts, 1 relative ts, 2 negative relative ts, 3 steps, 4 kcal, 5 distance, 6 duration, 7 avg HR.")
                .multi()
                .timeout(30_000)
                .evidence(AndroidLegacy6f),
            K::DeleteSportData => set(self, "sync.delete_sport", "Delete activity records", Health, 0x53)
                .request("`[00]`")
                .evidence(Rewrite),
            K::DeleteSleepData => set(self, "sync.delete_sleep", "Delete sleep records", Health, 0x55)
                .request("`[00]`")
                .evidence(Rewrite),
            K::DeleteHeartRateData => set(self, "sync.delete_heart_rate", "Delete heart-rate records", Health, 0x5A)
                .request("`[00]`")
                .evidence(Rewrite),
            K::DeleteWorkoutData => set(self, "sync.delete_workouts", "Delete workouts", Health, 0x68)
                .request("`[00]`")
                .evidence(AndroidLegacy6f),
            K::GetSportSleepCount6e => e6(self, "sync.count_6e", "Read sport or sleep count (6E)", Health, 0x30)
                .request("`[01]` sport or `[02]` sleep.")
                .response("6E `0x12` (sport) or `0x15` (sleep): `count_le24`.")
                .reply(0x12)
                .evidence(Android6e)
                .notes("The sleep variant waits for `0x15`."),
            K::GetSportData6e => e6(self, "sync.sport_6e", "Fetch activity (6E)", Health, 0x06)
                .request("`[01]`")
                .response("6E `0x05`: `x ts_le32 steps_le32 kcal_le32 …`.")
                .reply(0x05)
                .evidence(Android6e),
            K::GetSleepData6e => e6(self, "sync.sleep_6e", "Fetch sleep (6E)", Health, 0x31)
                .request("`[01]`")
                .response("6E `0x13`: `status ts_le32`.")
                .reply(0x13)
                .evidence(Android6e),

            // ---- alarms and reminders --------------------------------------------
            K::GetAlarmCounts => get(self, "alarms.counts", "Read alarm counts", Alarms, 0x9C)
                .request("`[00]`")
                .response("`reminder_count alarm_count`")
                .timeout(5_000)
                .evidence(AndroidLegacy6f),
            K::GetAlarms => get(self, "alarms.get", "Read alarms", Alarms, 0x9C)
                .summary("Reads every wake alarm, one frame each.")
                .request("`[03]`")
                .response("Per alarm: `x id` then TLVs `00` type (4 = alarm), `02` weekdays, `04` hour minute, `06` enabled, `08` snooze.")
                .multi()
                .timeout(5_000)
                .evidence(AndroidLegacy6f),
            K::GetFreeAlarmId => get(self, "alarms.free_id", "Read a free alarm ID", Alarms, 0x9C)
                .request("`[04]`")
                .response("`[id]`")
                .timeout(5_000)
                .evidence(AndroidLegacy6f),
            K::SetAlarm => set(self, "alarms.set", "Write an alarm", Alarms, 0x9C)
                .summary("Creates or updates a wake alarm.")
                .request("`is_new?00:01 id` then TLVs `00 01 04`, `02 01 weekdays`, `04 02 hour minute`, `06 01 enabled`, `07 01 07`, `08 01 snooze`.")
                .evidence(AndroidLegacy6f),
            K::SetReminderLegacy => set(self, "reminders.set_legacy", "Write a reminder (6F)", Alarms, 0x11)
                .request("`index enabled type hour minute weekdays`")
                .evidence(Rewrite)
                .notes("No stock handler for `0x11`; the P03B path is the 6E reminder family."),
            K::GetRemindersLegacy => get(self, "reminders.get_legacy", "Read reminders (6F)", Alarms, 0x11)
                .request("`[00]` (the iOS app sent an empty payload, which stock firmware drops).")
                .response("`index enabled type hour minute weekdays`")
                .evidence(Rewrite),
            K::GetReminders6e => e6(self, "reminders.get_6e", "Read reminders (6E)", Alarms, 0x20)
                .summary("Asks the watch to send its reminders; each arrives as a 6E `0x11` event.")
                .request("`[01]`")
                .response("6E `0x20` completion; reminders arrive as 6E `0x11 [type hour minute weekdays]`.")
                .reply(0x20)
                .timeout(4_000)
                .evidence(Android6e),
            K::AddReminder6e => e6(self, "reminders.add_6e", "Add a reminder (6E)", Alarms, 0x40)
                .request("`type hour minute weekdays` (6E type numbering).")
                .evidence(Android6e),
            K::DeleteReminder6e => e6(self, "reminders.delete_6e", "Delete a reminder (6E)", Alarms, 0x09)
                .request("`hour minute`")
                .evidence(Android6e),
            K::DeleteAllReminders6e => e6(self, "reminders.delete_all_6e", "Delete all reminders (6E)", Alarms, 0x21)
                .request("`[01]`")
                .evidence(Android6e),

            // ---- weather, notifications, media, remote ---------------------------
            K::SetTemperatureUnit => set(self, "weather.unit", "Set temperature unit", Weather, 0x22)
                .summary("Device-setting sub-command 1: switches the watch's °C/°F glyph. Send before a forecast.")
                .request("`01 [unit]` (0 = °C, 1 = °F).")
                .evidence(AndroidLegacy6f),
            K::PushWeather => set(self, "weather.push", "Push forecast", Weather, 0x77)
                .summary("Pushes up to four forecast days and the city name.")
                .request("Per day `unit temp low high condition` (temps as signed bytes in the display unit), then up to 90 bytes of UTF-8 city.")
                .evidence(AndroidLegacy6f),
            K::PushMessageCounts => set(self, "notifications.counts", "Push unread counts", Notifications, 0x70)
                .request("`n` then `kind count` pairs.")
                .evidence(AndroidLegacy6f)
                .notes("No stock handler for `0x70`. Norm Two shows iOS notifications via ANCS."),
            K::PushSms => set(self, "notifications.sms", "Push a text message", Notifications, 0x71)
                .request("`len sender(≤20 chars) len body(≤200 chars)`")
                .evidence(AndroidLegacy6f)
                .notes("No stock handler for `0x71`."),
            K::PushMusicState => spec(self, "music.state", "Push music state", Media, FrameFormat::Frame6f, 0xD0, Some(Action::CheckResponse))
                .summary("Play state plus a \"title - artist\" line for the music page.")
                .request("`01 [00 playing | 01 paused] UTF-8 title (≤120 chars)`")
                .response("`01 81 [D0, status]` ack.")
                .evidence(AndroidLegacy6f)
                .notes("The handler zeroes the artist buffer, which blanks what the watch resolved over Apple Media Service; NormConnect only sends volume."),
            K::PushMusicVolume => spec(self, "music.volume", "Push music volume", Media, FrameFormat::Frame6f, 0xD0, Some(Action::CheckResponse))
                .request("`01 02 percent`")
                .response("`01 81 [D0, status]` ack.")
                .evidence(AndroidLegacy6f),
            K::PushMusicStop => spec(self, "music.stop", "Push music stop", Media, FrameFormat::Frame6f, 0xD0, Some(Action::CheckResponse))
                .request("`01 FF`")
                .response("`01 81 [D0, status]` ack.")
                .evidence(AndroidLegacy6f),
            K::FindPhoneStartAck => spec(self, "remote.find_phone_ack", "Acknowledge find phone", Remote, FrameFormat::Frame6f, 0x01, Some(Action::SetResponse))
                .summary("Phone's reply to the watch's find-phone start (`D2 71 [00]`).")
                .request("`D2 00` (generic set response) on remote write `8003`.")
                .remote()
                .write_only()
                .evidence(AndroidLegacy6f),
            K::FindPhoneFound => set(self, "remote.find_phone_found", "Phone found", Remote, 0x1A)
                .summary("Dismisses the watch's find-phone screen after the user stops the alert on the phone.")
                .request("`[10]` (control device: find-phone success).")
                .evidence(AndroidLegacy6f),
            K::PhoneGpsStatus => spec(self, "remote.gps_status", "Phone GPS status", Remote, FrameFormat::Frame6f, 0xE9, Some(Action::CheckResponse))
                .summary("Tells the watch whether phone GPS is available for a linked workout.")
                .request("`00 01 [0 available | 1 unavailable]` on `8003`.")
                .remote()
                .write_only()
                .evidence(AndroidLegacy6f),
            K::PhoneGpsDistance => spec(self, "remote.gps_distance", "Phone GPS distance", Remote, FrameFormat::Frame6f, 0xE9, Some(Action::CheckResponse))
                .summary("Distance, speed and lifecycle confirmations during a phone-linked workout.")
                .request("Status TLV, `01 04 meters_le32`, `02 04 kmh_le32`, `03 04 start pause resume stop` (0 = confirmed).")
                .remote()
                .write_only()
                .evidence(AndroidLegacy6f),
            K::PhoneGpsLocation => spec(self, "remote.gps_location", "Phone GPS location", Remote, FrameFormat::Frame6f, 0xE9, Some(Action::CheckResponse))
                .summary("A phone GPS fix in reply to the watch's location request.")
                .request("`04 14 ts lat_e7 lon_e7 alt_cm accuracy_cm` (all LE32).")
                .remote()
                .write_only()
                .evidence(AndroidLegacy6f),

            // ---- watch faces and carousel --------------------------------------
            K::GetWatchFaces => get(self, "faces.list", "List watch faces", WatchFace, 0x1E)
                .request("`[01]`")
                .response("`count current id…`")
                .evidence(IosObserved),
            K::SelectWatchFace => set(self, "faces.select", "Select a watch face", WatchFace, 0x1E)
                .request("`02 index`")
                .evidence(IosObserved),
            K::SetCrownFunctions => set(self, "crown.set", "Assign crown functions", WatchFace, 0x2E)
                .request("`n function…`: 1 timer, 2 stopwatch, 3 weather, 4 HR, 5 alarm, 6 settings, 7 2nd zone, 8 music.")
                .evidence(AndroidLegacy6f)
                .notes("No stock handler for `0x2E`."),
            K::GetPageOrder => get(self, "pages.get", "Read carousel order", WatchFace, 0x2A)
                .summary("Reads the swipeable page order. The source picks the firmware path.")
                .request("AppSetting `[01]`; driver slot `0x0E` / `0x0F` with an empty payload; or diagnostic `[FE]`.")
                .response("AppSetting: `01 count id…` (stock often zeroes IDs). Driver slots: `0xDD` / `0xE8` raw blobs.")
                .timeout(4_000)
                .evidence(FirmwarePatch)
                .notes("Response code depends on the source: `0x2A`, `0xDD` or `0xE8`. The driver-slot reads are empty frames, which stock firmware drops; stock AppSetting reports the slot count with zeroed IDs."),
            K::SetPageOrder => set(self, "pages.set", "Write carousel order", WatchFace, 0x2A)
                .summary("Writes up to ten page IDs (2 home, 3 mindfulness, 4 workout, 5 rings, 6 sleep, 7 HR, 8 music, 9 weather).")
                .request("`01 count id…`")
                .response("`01 81 [2A, status]` ack, or `2A [status]`.")
                .evidence(FirmwarePatch),
            K::GetPageRouteTrace => get(self, "pages.route_trace", "Read page route trace", WatchFace, 0x2A)
                .request("`[FD]`")
                .response("`FD len` then a latest record and 8-byte records.")
                .timeout(6_000)
                .evidence(FirmwarePatch)
                .patched("route telemetry diagnostic"),

            // ---- patched firmware -------------------------------------------------
            K::GetCapabilities => get(self, "patched.capabilities", "Read patched capabilities", PatchedFirmware, 0x2A)
                .summary("Reads the patched firmware's feature bitmask; stock firmware answers an empty `0x2A`, read as no capabilities.")
                .request("`[43]`")
                .response("`DC bitmask_le32`")
                .reply(0xDC)
                .alternates(&[0x2A])
                .timeout(2_000)
                .evidence(FirmwarePatch)
                .patched("services blob")
                .notes("A timeout is also reported as no capabilities."),
            K::ReadMemory => get(self, "patched.read_memory", "Read watch memory", PatchedFirmware, 0x2A)
                .summary("Reads up to ~64 bytes of SRAM (or flash with Flash Read).")
                .request("`40 addr_le32 len`")
                .response("`DC bytes…`; empty when rejected.")
                .reply(0xDC)
                .timeout(2_000)
                .evidence(FirmwarePatch)
                .patched("memory probe")
                .notes("Bounds-checked to SRAM `0x10000000–0x10060000`. A timeout is reported as empty."),
            K::ReadResource => get(self, "patched.read_resource", "Read a resource", PatchedFirmware, 0x2A)
                .summary("Reads icon, glyph or font bytes from resource flash by handle.")
                .request("`41 handle_le32 offset_le32 len` (len ≤ 238).")
                .response("`DC bytes…`; empty past the end or while the screen is off.")
                .reply(0xDC)
                .timeout(3_000)
                .evidence(FirmwarePatch)
                .patched("resource read"),
            K::SetCustomText => set(self, "patched.custom_text", "Set custom page text", PatchedFirmware, 0x2A)
                .summary("Text for the custom carousel page (≤120 UTF-8 bytes).")
                .request("`40 len UTF-8…`")
                .write_only()
                .evidence(FirmwarePatch)
                .patched("dynamic text"),
            K::PushCalendarEvents => set(self, "patched.calendar", "Push calendar events", PatchedFirmware, 0x2A)
                .summary("Fills the calendar page and home widget: five slot frames, then a commit.")
                .request("Per slot `41 71 slot title[40] line1[24] line2[24] line3[24]`; commit `42 01 count`.")
                .write_only()
                .evidence(FirmwarePatch)
                .patched("calendar")
                .notes("Slot 1's line2/line3 carry the first event's widget title and subtitle."),

            // ---- system ---------------------------------------------------------------
            K::EnterUpgradeMode => set(self, "system.upgrade_mode", "Enter upgrade mode", System, 0x12)
                .request("`[01]`")
                .write_only()
                .timeout(30_000)
                .evidence(IosObserved)
                .notes("Not needed for the Apollo OTA service (`1530`) on Norm Two; no stock handler."),
            K::EnterUpgradeMode6e => e6(self, "system.upgrade_mode_6e", "Enter upgrade mode (6E)", System, 0x1C)
                .request("`mode subtype value_le32 name[20]` in a 26-byte buffer.")
                .evidence(Android6e),
            K::RestoreFactory6e => e6(self, "system.factory_reset_6e", "Factory reset (6E)", System, 0x11)
                .summary("Unbinds and wipes the watch. Destructive.")
                .request("`[01]`")
                .evidence(Android6e),
            K::Raw => spec(self, "raw", "Raw frame", System, FrameFormat::Frame6f, 0x00, None)
                .summary("Any frame, for probing. Response matching is whatever the caller sets.")
                .request("Caller-supplied frame.")
                .response("Caller-supplied response code.")
                .evidence(Rewrite),
        };
        b.build()
    }

    /// A representative request, used by the docs and the emulator tests.
    pub fn example(self) -> Request {
        use CommandKind as K;
        use Request as R;
        let time = LocalDateTime {
            year: 2026,
            month: 10,
            day: 11,
            hour: 12,
            minute: 30,
            second: 0,
            utc_offset_minutes: 60,
        };
        let profile = UserProfile {
            height_cm: 175,
            weight_kg: 70,
            age: 30,
            gender: Gender::Male,
        };
        let reminder = Reminder {
            index: 0,
            enabled: true,
            kind: ReminderKind::Drink,
            hour: 10,
            minute: 0,
            weekdays: 0x1F,
        };
        match self {
            K::BindStart => R::BindStart {
                mode: BindMode::QrCode,
            },
            K::BindEnd => R::BindEnd,
            K::GetWatchId => R::GetWatchId,
            K::GetDeviceVersion => R::GetDeviceVersion,
            K::GetDeviceVersionNoArgument => R::GetDeviceVersionNoArgument,
            K::GetBattery => R::GetBattery { new_mode: true },
            K::GetDeviceVersion6e => R::GetDeviceVersion6e,
            K::GetBattery6e => R::GetBattery6e,
            K::SetDateTime => R::SetDateTime {
                time,
                analog_hand_calibration: false,
            },
            K::SetDateTime6e => R::SetDateTime6e { time },
            K::SetUserProfile => R::SetUserProfile { profile },
            K::SetUserProfile6e => R::SetUserProfile6e { profile },
            K::SetLanguage => R::SetLanguage { language: 0 },
            K::SetBrightness => R::SetBrightness { level: 4 },
            K::GetBrightness => R::GetBrightness,
            K::SetScreenTimeout => R::SetScreenTimeout { seconds: 15 },
            K::GetScreenTimeout => R::GetScreenTimeout,
            K::SetVibration => R::SetVibration {
                enabled: true,
                screen_speed: 3,
            },
            K::GetVibration => R::GetVibration,
            K::GetVibrationStrength => R::GetVibrationStrength,
            K::GetSwitches => R::GetSwitches,
            K::SetRaiseToWake => R::SetRaiseToWake { enabled: true },
            K::SetAutoSleepSwitch => R::SetAutoSleepSwitch { enabled: true },
            K::SetUnit => R::SetUnit { metric: true },
            K::GetUnit => R::GetUnit,
            K::SetUnitsAndFormat6e => R::SetUnitsAndFormat6e {
                metric: true,
                use_24h: true,
                date_format: DateFormat::DayMonthYear,
                show_battery: true,
            },
            K::GetUnitsAndFormat6e => R::GetUnitsAndFormat6e,
            K::SetGoal => R::SetGoal {
                kind: GoalKind::Steps,
                value: 10_000,
            },
            K::GetGoals => R::GetGoals,
            K::SetGoal6e => R::SetGoal6e {
                kind: Goal6eKind::Steps,
                value: 10_000,
            },
            K::GetGoal6e => R::GetGoal6e {
                kind: Goal6eKind::Steps,
            },
            K::SetSleepWindow => R::SetSleepWindow {
                window: SleepWindow {
                    start_hour: 22,
                    start_minute: 30,
                    end_hour: 7,
                    end_minute: 0,
                    weekdays: 0x7F,
                },
            },
            K::GetSleepWindow => R::GetSleepWindow,
            K::SetInactivityAlert6e => R::SetInactivityAlert6e {
                alert: InactivityAlert {
                    enabled: true,
                    weekdays: 0x1F,
                    interval_minutes: 60,
                    start_hour: 9,
                    start_minute: 0,
                    end_hour: 18,
                    end_minute: 0,
                    step_threshold: 100,
                },
            },
            K::GetInactivityAlert6e => R::GetInactivityAlert6e,
            K::SetAutoHeartRate => R::SetAutoHeartRate {
                interval_minutes: 10,
            },
            K::GetAutoHeartRate => R::GetAutoHeartRate,
            K::HandMoveContinuous => R::HandMoveContinuous {
                hand: WatchHand::Minute,
                clockwise: true,
                code: HandCalibrationCode::Stop,
            },
            K::HandSyncTime => R::HandSyncTime {
                hour: 12,
                minute: 30,
                second: 0,
                finish: true,
            },
            K::HandMachineTiming => R::HandMachineTiming {
                hour_angle: 0,
                minute_angle: 0,
                hour: 12,
                minute: 30,
                second: 0,
            },
            K::HandStep => R::HandStep {
                hand: WatchHand::Minute,
                clockwise: true,
                angle: 2,
            },
            K::GetDataCounts => R::GetDataCounts,
            K::GetSportRecords => R::GetSportRecords { count: 1 },
            K::GetSleepRecords => R::GetSleepRecords { count: 1 },
            K::GetHeartRateRecords => R::GetHeartRateRecords { count: 1 },
            K::GetWorkoutCount => R::GetWorkoutCount,
            K::GetWorkouts => R::GetWorkouts { count: 1 },
            K::GetWorkoutHeartRateCount => R::GetWorkoutHeartRateCount,
            K::GetWorkoutHeartRate => R::GetWorkoutHeartRate { count: 1 },
            K::DeleteSportData => R::DeleteSportData,
            K::DeleteSleepData => R::DeleteSleepData,
            K::DeleteHeartRateData => R::DeleteHeartRateData,
            K::DeleteWorkoutData => R::DeleteWorkoutData,
            K::GetSportSleepCount6e => R::GetSportSleepCount6e {
                kind: SportSleepKind::Sport,
            },
            K::GetSportData6e => R::GetSportData6e,
            K::GetSleepData6e => R::GetSleepData6e,
            K::GetAlarmCounts => R::GetAlarmCounts,
            K::GetAlarms => R::GetAlarms { expected: 1 },
            K::GetFreeAlarmId => R::GetFreeAlarmId,
            K::SetAlarm => R::SetAlarm {
                alarm: Alarm {
                    id: 1,
                    enabled: true,
                    hour: 7,
                    minute: 30,
                    weekdays: 0x1F,
                    snooze_minutes: 10,
                },
                is_new: true,
            },
            K::SetReminderLegacy => R::SetReminderLegacy { reminder },
            K::GetRemindersLegacy => R::GetRemindersLegacy,
            K::GetReminders6e => R::GetReminders6e,
            K::AddReminder6e => R::AddReminder6e { reminder },
            K::DeleteReminder6e => R::DeleteReminder6e {
                hour: 10,
                minute: 0,
            },
            K::DeleteAllReminders6e => R::DeleteAllReminders6e,
            K::SetTemperatureUnit => R::SetTemperatureUnit {
                unit: TemperatureUnit::Celsius,
            },
            K::PushWeather => R::PushWeather {
                days: vec![
                    WeatherDay {
                        temperature: 14,
                        low: 9,
                        high: 16,
                        condition: 1,
                    },
                    WeatherDay {
                        temperature: 13,
                        low: 8,
                        high: 17,
                        condition: 2,
                    },
                    WeatherDay {
                        temperature: 12,
                        low: 7,
                        high: 15,
                        condition: 3,
                    },
                    WeatherDay {
                        temperature: 11,
                        low: 6,
                        high: 14,
                        condition: 1,
                    },
                ],
                city: Some("London".into()),
                unit: TemperatureUnit::Celsius,
            },
            K::PushMessageCounts => R::PushMessageCounts {
                counts: vec![MessageCount {
                    kind: NotificationKind::Sms,
                    count: 2,
                }],
            },
            K::PushSms => R::PushSms {
                sender: "Alex".into(),
                body: "On my way".into(),
            },
            K::PushMusicState => R::PushMusicState {
                playing: true,
                title_and_artist: "Song - Artist".into(),
            },
            K::PushMusicVolume => R::PushMusicVolume { percent: 50 },
            K::PushMusicStop => R::PushMusicStop,
            K::FindPhoneStartAck => R::FindPhoneStartAck,
            K::FindPhoneFound => R::FindPhoneFound,
            K::PhoneGpsStatus => R::PhoneGpsStatus {
                gps_available: true,
            },
            K::PhoneGpsDistance => R::PhoneGpsDistance {
                gps_available: true,
                distance_m: 1200,
                speed_kmh: 10,
                confirmations: GpsConfirmations {
                    start: true,
                    ..Default::default()
                },
            },
            K::PhoneGpsLocation => R::PhoneGpsLocation {
                fix: GpsFix {
                    timestamp: 1_791_720_000,
                    latitude_e7: 515_074_000,
                    longitude_e7: -1_278_000,
                    altitude_cm: 1_100,
                    accuracy_cm: 500,
                },
            },
            K::GetWatchFaces => R::GetWatchFaces,
            K::SelectWatchFace => R::SelectWatchFace { index: 0 },
            K::SetCrownFunctions => R::SetCrownFunctions {
                functions: vec![CrownFunction::Music],
            },
            K::GetPageOrder => R::GetPageOrder {
                source: PageOrderSource::AppSetting,
            },
            K::SetPageOrder => R::SetPageOrder {
                pages: vec![2, 5, 7, 6, 4, 3, 8, 9],
            },
            K::GetPageRouteTrace => R::GetPageRouteTrace,
            K::GetCapabilities => R::GetCapabilities,
            K::ReadMemory => R::ReadMemory {
                address: 0x1000_0000,
                length: 16,
            },
            K::ReadResource => R::ReadResource {
                handle: 0x0001_0000,
                offset: 0,
                length: 16,
            },
            K::SetCustomText => R::SetCustomText {
                text: "Hello from NormSDK".into(),
            },
            K::PushCalendarEvents => R::PushCalendarEvents {
                events: vec![CalendarEvent {
                    title: "Standup".into(),
                    line1: "Monday 12/10".into(),
                    line2: "09:30 - 09:45".into(),
                    line3: "15 mins".into(),
                    widget_title: "Team standup".into(),
                    widget_subtitle: "MON 9:30 15min".into(),
                }],
            },
            K::EnterUpgradeMode => R::EnterUpgradeMode,
            K::EnterUpgradeMode6e => R::EnterUpgradeMode6e {
                mode: 1,
                subtype: 4,
                value: 0,
                label: "P03B".into(),
            },
            K::RestoreFactory6e => R::RestoreFactory6e,
            K::Raw => R::Raw {
                label: "battery".into(),
                frame: vec![0x6F, 0x08, 0x70, 0x01, 0x00, 0x01, 0x8F],
                channel: WriteChannel::Primary,
                response_command: Some(0x08),
                timeout_ms: 2_000,
            },
        }
    }

    /// True for commands that change persistent watch state in a way a test
    /// harness should not run against real hardware unprompted.
    pub fn is_destructive(self) -> bool {
        matches!(
            self,
            CommandKind::RestoreFactory6e
                | CommandKind::EnterUpgradeMode
                | CommandKind::EnterUpgradeMode6e
                | CommandKind::DeleteSportData
                | CommandKind::DeleteSleepData
                | CommandKind::DeleteHeartRateData
                | CommandKind::DeleteWorkoutData
                | CommandKind::DeleteAllReminders6e
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn every_kind_has_a_complete_annotation_and_matching_example() {
        let mut ids = HashSet::new();
        for kind in CommandKind::ALL {
            let spec = kind.spec();
            assert_eq!(spec.kind, kind);
            assert!(ids.insert(spec.id.clone()), "duplicate id {}", spec.id);
            assert!(
                !spec.title.is_empty() && !spec.request_layout.is_empty(),
                "{kind:?}"
            );
            assert!(
                !spec.response_layout.is_empty(),
                "{kind:?} lacks a response layout"
            );
            let example = kind.example();
            assert_eq!(example.kind(), kind);
            let frames = example.frames();
            assert!(!frames.is_empty());
            if kind != CommandKind::Raw {
                let first = &frames[0].data;
                let code = if spec.format == FrameFormat::Frame6f {
                    first[1]
                } else {
                    first[2]
                };
                assert_eq!(code, spec.command, "{kind:?}");
                assert_eq!(frames[0].channel, spec.write_channel, "{kind:?}");
            }
        }
        assert_eq!(ids.len(), CommandKind::ALL.len());
    }

    #[test]
    fn firmware_support_is_derived_from_the_stock_table() {
        assert_eq!(
            CommandKind::GetBattery.spec().firmware,
            FirmwareSupport::Stock
        );
        assert_eq!(
            CommandKind::SetLanguage.spec().firmware,
            FirmwareSupport::NoStockHandler
        );
        assert_eq!(
            CommandKind::GetBattery6e.spec().firmware,
            FirmwareSupport::Ignored6e
        );
        assert!(matches!(
            CommandKind::ReadMemory.spec().firmware,
            FirmwareSupport::Patched { .. }
        ));
    }
}
