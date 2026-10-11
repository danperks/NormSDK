//! Wire framing for the two packet formats the watch speaks.
//!
//! * **6F** (the main Norm Two protocol, Android's "legacy" `MBluetooth`):
//!   `[0x6F][cmd][action][len_lo][len_hi][payload…][0x8F]`. Length-prefixed.
//! * **6E** (Android's `MBluetooth6E` / `protocol6E`):
//!   `[0x6E][0x01][cmd][payload…][0x8F]`. Not length-prefixed, so it is
//!   terminated by the first `0x8F`.
//!
//! The stock Norm Two firmware only parses 6F frames; 6E support is kept for
//! parity with the Android app and for probing other P03B builds.

use std::collections::HashMap;
use std::sync::Mutex;

pub const HEADER_6F: u8 = 0x6F;
pub const HEADER_6E: u8 = 0x6E;
pub const TRAILER: u8 = 0x8F;

const HEADER_6F_LEN: usize = 5;
/// Well above any real frame; only used to resynchronise quickly when a
/// corrupt length field is seen.
const MAX_PAYLOAD_6F: usize = 8192;
const MAX_FRAME_6E: usize = 8192;

/// Which of the two frame layouts a packet uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum FrameFormat {
    /// `6F cmd action len_lo len_hi payload… 8F`
    Frame6f,
    /// `6E 01 cmd payload… 8F`
    Frame6e,
}

/// The 6F action byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum Action {
    /// `0x70`: read/query.
    Check,
    /// `0x71`: write/configure.
    Set,
    /// `0x80`: reply to a check (also used by a few phone→watch pushes).
    CheckResponse,
    /// `0x81`: reply to a set; the generic ack `01 81 [cmd, status]` uses it.
    SetResponse,
    /// Any other byte, kept verbatim.
    Other { raw: u8 },
}

impl Action {
    pub fn byte(self) -> u8 {
        match self {
            Action::Check => 0x70,
            Action::Set => 0x71,
            Action::CheckResponse => 0x80,
            Action::SetResponse => 0x81,
            Action::Other { raw } => raw,
        }
    }

    pub fn from_byte(byte: u8) -> Action {
        match byte {
            0x70 => Action::Check,
            0x71 => Action::Set,
            0x80 => Action::CheckResponse,
            0x81 => Action::SetResponse,
            raw => Action::Other { raw },
        }
    }
}

/// The notify characteristic a packet arrived on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum Channel {
    /// `8002`, replies to commands written to `8001`.
    Primary,
    /// `8004`, watch-initiated remote events (find phone, phone GPS, live HR).
    Remote,
    /// `8005`, transparent channel.
    Transparent,
}

/// The write characteristic an outgoing frame must go to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum WriteChannel {
    /// `8001`, every command.
    Primary,
    /// `8003`, replies to watch-initiated remote events.
    Remote,
}

/// One complete frame received from the watch.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Packet {
    pub format: FrameFormat,
    pub command: u8,
    /// For 6E frames this is always `CheckResponse`, as in the original apps.
    pub action: Action,
    pub payload: Vec<u8>,
    pub channel: Channel,
}

impl Packet {
    /// The generic ack `01 81 [command, status]`, when this is one.
    pub fn ack(&self) -> Option<(u8, u8)> {
        (self.format == FrameFormat::Frame6f && self.command == 0x01 && self.payload.len() >= 2)
            .then(|| (self.payload[0], self.payload[1]))
    }

    /// Re-encodes the packet as it appeared on the wire.
    pub fn to_frame(&self) -> Vec<u8> {
        match self.format {
            FrameFormat::Frame6f => encode_6f(self.command, self.action, &self.payload),
            FrameFormat::Frame6e => encode_6e(self.command, &self.payload),
        }
    }
}

/// Builds a 6F frame.
pub fn encode_6f(command: u8, action: Action, payload: &[u8]) -> Vec<u8> {
    let len = payload.len();
    let mut frame = Vec::with_capacity(len + 6);
    frame.push(HEADER_6F);
    frame.push(command);
    frame.push(action.byte());
    frame.push((len & 0xFF) as u8);
    frame.push(((len >> 8) & 0xFF) as u8);
    frame.extend_from_slice(payload);
    frame.push(TRAILER);
    frame
}

/// Builds a 6E frame.
pub fn encode_6e(command: u8, payload: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(payload.len() + 4);
    frame.push(HEADER_6E);
    frame.push(0x01);
    frame.push(command);
    frame.extend_from_slice(payload);
    frame.push(TRAILER);
    frame
}

/// Splits a frame into writes of at most `max_len` bytes (the negotiated
/// write length). The watch reassembles them by the 6F length field.
pub fn chunk(frame: &[u8], max_len: u32) -> Vec<Vec<u8>> {
    let size = (max_len as usize).max(1);
    frame.chunks(size).map(<[u8]>::to_vec).collect()
}

#[derive(Default)]
struct Buffers {
    f6f: Vec<u8>,
    f6e: Vec<u8>,
}

/// Reassembles notifications into packets.
///
/// Each notify characteristic keeps its own buffers because fragments from
/// `8002`, `8004` and `8005` can interleave. 6F frames are consumed by their
/// declared length, so payload bytes equal to a marker never truncate them;
/// 6E frames end at the first `0x8F`.
#[derive(Default)]
pub struct Reassembler {
    buffers: HashMap<Channel, Buffers>,
}

impl Reassembler {
    pub fn reset(&mut self) {
        self.buffers.clear();
    }

    pub fn feed(&mut self, channel: Channel, data: &[u8]) -> Vec<Packet> {
        let mut out = Vec::new();
        let buffers = self.buffers.entry(channel).or_default();

        for &byte in data {
            if !buffers.f6f.is_empty() {
                buffers.f6f.push(byte);
                if buffers.f6f.len() < HEADER_6F_LEN {
                    continue;
                }
                let payload_len = usize::from(buffers.f6f[3]) | (usize::from(buffers.f6f[4]) << 8);
                if payload_len > MAX_PAYLOAD_6F {
                    buffers.f6f.clear();
                    if byte == HEADER_6F {
                        buffers.f6f.push(byte);
                    } else if byte == HEADER_6E {
                        buffers.f6e.push(byte);
                    }
                    continue;
                }
                if buffers.f6f.len() > HEADER_6F_LEN + payload_len {
                    if let Some(packet) = parse_6f(&buffers.f6f, channel) {
                        out.push(packet);
                    }
                    buffers.f6f.clear();
                }
                continue;
            }

            if !buffers.f6e.is_empty() {
                match byte {
                    TRAILER => {
                        buffers.f6e.push(byte);
                        if let Some(packet) = parse_6e(&buffers.f6e, channel) {
                            out.push(packet);
                        }
                        buffers.f6e.clear();
                    }
                    HEADER_6F => {
                        buffers.f6e.clear();
                        buffers.f6f.push(byte);
                    }
                    HEADER_6E => {
                        buffers.f6e.clear();
                        buffers.f6e.push(byte);
                    }
                    _ => {
                        buffers.f6e.push(byte);
                        if buffers.f6e.len() > MAX_FRAME_6E {
                            buffers.f6e.clear();
                        }
                    }
                }
                continue;
            }

            match byte {
                HEADER_6F => buffers.f6f.push(byte),
                HEADER_6E => buffers.f6e.push(byte),
                _ => {}
            }
        }

        if buffers.f6f.is_empty() && buffers.f6e.is_empty() {
            self.buffers.remove(&channel);
        }
        out
    }
}

fn parse_6f(frame: &[u8], channel: Channel) -> Option<Packet> {
    if frame.len() < 6 {
        return None;
    }
    let payload_len = usize::from(frame[3]) | (usize::from(frame[4]) << 8);
    let total = HEADER_6F_LEN + payload_len + 1;
    if frame.len() < total || frame[total - 1] != TRAILER {
        return None;
    }
    Some(Packet {
        format: FrameFormat::Frame6f,
        command: frame[1],
        action: Action::from_byte(frame[2]),
        payload: frame[HEADER_6F_LEN..HEADER_6F_LEN + payload_len].to_vec(),
        channel,
    })
}

fn parse_6e(frame: &[u8], channel: Channel) -> Option<Packet> {
    if frame.len() < 4 {
        return None;
    }
    Some(Packet {
        format: FrameFormat::Frame6e,
        command: frame[2],
        action: Action::CheckResponse,
        payload: frame[3..frame.len() - 1].to_vec(),
        channel,
    })
}

/// Thread-safe reassembler for the foreign bindings.
#[derive(uniffi::Object, Default)]
pub struct FrameDecoder {
    inner: Mutex<Reassembler>,
}

#[uniffi::export]
impl FrameDecoder {
    #[uniffi::constructor]
    pub fn new() -> Self {
        Self::default()
    }

    /// Feeds one notification and returns every packet it completed.
    pub fn feed(&self, channel: Channel, data: Vec<u8>) -> Vec<Packet> {
        self.inner.lock().unwrap().feed(channel, &data)
    }

    /// Drops any partial frames, e.g. after a disconnect.
    pub fn reset(&self) {
        self.inner.lock().unwrap().reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_6f_and_6e() {
        assert_eq!(
            encode_6f(0x08, Action::Check, &[0x01]),
            vec![0x6F, 0x08, 0x70, 0x01, 0x00, 0x01, 0x8F]
        );
        assert_eq!(encode_6e(0x0F, &[0x01]), vec![0x6E, 0x01, 0x0F, 0x01, 0x8F]);
    }

    #[test]
    fn reassembles_fragments_and_ignores_marker_bytes_in_payload() {
        let frame = encode_6f(0x03, Action::CheckResponse, &[0x8F, 0x6F, 0x6E, 0x41]);
        let mut r = Reassembler::default();
        let mut packets = Vec::new();
        for part in chunk(&frame, 3) {
            packets.extend(r.feed(Channel::Primary, &part));
        }
        assert_eq!(packets.len(), 1);
        assert_eq!(packets[0].payload, vec![0x8F, 0x6F, 0x6E, 0x41]);
        assert_eq!(packets[0].action, Action::CheckResponse);
    }

    #[test]
    fn channels_reassemble_independently() {
        let a = encode_6f(0x08, Action::CheckResponse, &[0x4D]);
        let b = encode_6f(0xE9, Action::Check, &[0x00, 0x01, 0x05]);
        let mut r = Reassembler::default();
        assert!(r.feed(Channel::Primary, &a[..4]).is_empty());
        let remote = r.feed(Channel::Remote, &b);
        assert_eq!(remote.len(), 1);
        assert_eq!(remote[0].channel, Channel::Remote);
        let primary = r.feed(Channel::Primary, &a[4..]);
        assert_eq!(primary[0].payload, vec![0x4D]);
    }

    #[test]
    fn decodes_6e_and_several_frames_in_one_notification() {
        let mut data = encode_6e(0x00, &[0x50]);
        data.extend(encode_6f(0x01, Action::SetResponse, &[0x07, 0x00]));
        let packets = Reassembler::default().feed(Channel::Primary, &data);
        assert_eq!(packets.len(), 2);
        assert_eq!(packets[0].format, FrameFormat::Frame6e);
        assert_eq!(packets[1].ack(), Some((0x07, 0x00)));
    }

    #[test]
    fn resyncs_after_garbage_and_bad_trailer() {
        let mut data = vec![0x00, 0x12];
        let mut bad = encode_6f(0x08, Action::CheckResponse, &[0x4D]);
        *bad.last_mut().unwrap() = 0x00;
        data.extend(bad);
        data.extend(encode_6f(0x08, Action::CheckResponse, &[0x4E]));
        let packets = Reassembler::default().feed(Channel::Primary, &data);
        assert_eq!(packets.len(), 1);
        assert_eq!(packets[0].payload, vec![0x4E]);
    }
}
