//! NormSDK: the Norm Two (P03B) smartwatch protocol, shared by the iOS,
//! Android and Python clients through UniFFI.
//!
//! * [`frame`]: 6F/6E framing and notification reassembly.
//! * [`request`]: typed commands and their encodings.
//! * [`catalog`]: one annotated [`CommandSpec`] per command.
//! * [`response`]: typed replies and the reply matcher.
//! * [`message`]: classification of watch-initiated events.
//! * [`session`]: a sans-IO queue that ties them together.
//!
//! The library never touches Bluetooth or the clock; hosts pass both in.

uniffi::setup_scaffolding!();

mod bytes;
pub mod catalog;
pub mod frame;
pub mod gatt;
pub mod message;
pub mod model;
pub mod request;
pub mod response;
pub mod session;
pub mod version;

pub use catalog::{
    Category, CommandKind, CommandSpec, Evidence, FirmwareSupport, STOCK_6F_HANDLERS,
};
pub use frame::{Action, Channel, FrameDecoder, FrameFormat, Packet, Reassembler, WriteChannel};
pub use message::WatchMessage;
pub use model::*;
pub use request::{OutgoingFrame, Request};
pub use response::{MatchOutcome, Matcher, Response, ResponseMatcher};
pub use session::{OutgoingWrite, Priority, Session, SessionEvent};

/// Per-request matching details (timeouts scale with record counts).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RequestInfo {
    pub kind: CommandKind,
    pub label: String,
    pub expects_response: bool,
    pub response_command: Option<u8>,
    pub multi_response: bool,
    pub timeout_ms: u32,
}

/// The library version.
#[uniffi::export]
pub fn sdk_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// The frames to write for a request, in order.
#[uniffi::export]
pub fn encode_request(request: Request) -> Vec<OutgoingFrame> {
    request.frames()
}

#[uniffi::export]
pub fn request_info(request: Request) -> RequestInfo {
    RequestInfo {
        kind: request.kind(),
        label: request.label(),
        expects_response: request.expects_response(),
        response_command: request.response_command(),
        multi_response: request.is_multi_response(),
        timeout_ms: request.timeout_ms(),
    }
}

/// Every annotated command, in catalog order.
#[uniffi::export]
pub fn command_catalog() -> Vec<CommandSpec> {
    CommandKind::ALL.iter().map(|k| k.spec()).collect()
}

#[uniffi::export]
pub fn command_spec(kind: CommandKind) -> CommandSpec {
    kind.spec()
}

/// A representative request for a command, with sensible parameters.
#[uniffi::export]
pub fn example_request(kind: CommandKind) -> Request {
    kind.example()
}

/// True for commands that wipe data or leave normal mode.
#[uniffi::export]
pub fn is_destructive(kind: CommandKind) -> bool {
    kind.is_destructive()
}

/// True when the stock firmware has a handler for this 6F command code.
#[uniffi::export]
pub fn stock_firmware_handles(command: u8) -> bool {
    STOCK_6F_HANDLERS.contains(&command)
}

#[uniffi::export]
pub fn encode_frame_6f(command: u8, action: Action, payload: Vec<u8>) -> Vec<u8> {
    frame::encode_6f(command, action, &payload)
}

#[uniffi::export]
pub fn encode_frame_6e(command: u8, payload: Vec<u8>) -> Vec<u8> {
    frame::encode_6e(command, &payload)
}

/// Splits a frame into writes of at most `max_len` bytes.
#[uniffi::export]
pub fn chunk_frame(frame: Vec<u8>, max_len: u32) -> Vec<Vec<u8>> {
    frame::chunk(&frame, max_len)
}

/// `"6F 08 70 …"`, for logs.
#[uniffi::export]
pub fn hex_string(data: Vec<u8>) -> String {
    bytes::hex(&data)
}

/// A readable one-line description of a request, for logs.
#[uniffi::export]
pub fn describe_request(request: Request) -> String {
    format!("{request:?}")
}

/// A readable one-line description of a response, for logs.
#[uniffi::export]
pub fn describe_response(response: Response) -> String {
    format!("{response:?}")
}

/// A readable one-line description of a watch message, for logs.
#[uniffi::export]
pub fn describe_message(message: WatchMessage) -> String {
    format!("{message:?}")
}

/// Converts a Celsius reading to the watch's signed-byte temperature in the
/// chosen unit, rounding once from the unrounded value.
#[uniffi::export]
pub fn watch_temperature(celsius: f64, unit: TemperatureUnit) -> i32 {
    let value = match unit {
        TemperatureUnit::Celsius => celsius,
        TemperatureUnit::Fahrenheit => celsius * 9.0 / 5.0 + 32.0,
    };
    value.round().clamp(f64::from(i8::MIN), f64::from(i8::MAX)) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temperatures_round_and_clamp() {
        assert_eq!(watch_temperature(21.5, TemperatureUnit::Celsius), 22);
        assert_eq!(watch_temperature(-40.0, TemperatureUnit::Fahrenheit), -40);
        assert_eq!(watch_temperature(400.0, TemperatureUnit::Celsius), 127);
    }
}
