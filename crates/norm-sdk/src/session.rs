//! A sans-IO command session.
//!
//! The host owns the radio and the clock. It feeds notifications in, writes
//! whatever [`Session::poll_transmit`] returns, and calls [`Session::tick`]
//! by [`Session::next_deadline_ms`]. The session keeps one response-awaiting
//! command in flight, orders the queue by priority, matches replies (including
//! generic acks) and reports timeouts, mirroring the NormConnect command queue.

use std::collections::VecDeque;
use std::sync::Mutex;

use crate::frame::{Channel, Packet, Reassembler, WriteChannel};
use crate::message::{classify, WatchMessage};
use crate::request::Request;
use crate::response::{MatchOutcome, Matcher, Response};

/// Immediate writes wait at most this long behind an in-flight reply.
pub const IMMEDIATE_MAX_DEFER_MS: u64 = 10_000;

/// Queue order; lower runs first, ties run in submission order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, uniffi::Enum)]
pub enum Priority {
    Bind,
    Call,
    Page,
    Remote,
    Sync,
    Notify,
    MustDo,
    Normal,
    Contact,
    Reset,
}

/// A write the host must perform now, in order.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct OutgoingWrite {
    pub request_id: u64,
    pub label: String,
    pub channel: WriteChannel,
    /// A whole frame; split it to the negotiated write length if needed.
    pub data: Vec<u8>,
    /// True when the session now waits for this request's reply.
    pub awaits_response: bool,
}

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum SessionEvent {
    /// Every decoded frame, with the request that consumed it, if any.
    Received {
        packet: Packet,
        message: WatchMessage,
        consumed_by: Option<u64>,
    },
    Progress {
        request_id: u64,
        received: u32,
        total: u32,
    },
    Completed {
        request_id: u64,
        response: Response,
    },
    /// No reply in time. `partial` holds collected records or the
    /// documented empty result for probes.
    TimedOut {
        request_id: u64,
        partial: Option<Response>,
    },
    Cancelled {
        request_id: u64,
    },
}

struct Queued {
    id: u64,
    request: Request,
    priority: Priority,
}

struct InFlight {
    id: u64,
    matcher: Matcher,
    deadline_ms: u64,
}

#[derive(Default)]
struct Inner {
    decoder: Reassembler,
    queue: Vec<Queued>,
    immediate: VecDeque<(u64, Request, u64)>,
    current: Option<InFlight>,
    next_id: u64,
    link_ready: bool,
}

impl Inner {
    fn id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    fn writes(id: u64, request: &Request, awaits_response: bool) -> Vec<OutgoingWrite> {
        let label = request.label();
        let frames = request.frames();
        let last = frames.len().saturating_sub(1);
        frames
            .into_iter()
            .enumerate()
            .map(|(i, f)| OutgoingWrite {
                request_id: id,
                label: label.clone(),
                channel: f.channel,
                data: f.data,
                awaits_response: awaits_response && i == last,
            })
            .collect()
    }
}

#[derive(uniffi::Object, Default)]
pub struct Session {
    inner: Mutex<Inner>,
}

#[uniffi::export]
impl Session {
    #[uniffi::constructor]
    pub fn new() -> Self {
        Self::default()
    }

    /// Queues a request and returns its ID.
    pub fn enqueue(&self, request: Request, priority: Priority) -> u64 {
        let mut s = self.inner.lock().unwrap();
        let id = s.id();
        let at = s.queue.partition_point(|q| q.priority <= priority);
        s.queue.insert(
            at,
            Queued {
                id,
                request,
                priority,
            },
        );
        id
    }

    /// Sends a request ahead of the queue as a write-only command (its reply,
    /// if any, is not awaited). Held while another reply is awaited, for at
    /// most [`IMMEDIATE_MAX_DEFER_MS`], so bulk writes don't starve it.
    pub fn send_immediate(&self, request: Request, now_ms: u64) -> u64 {
        let mut s = self.inner.lock().unwrap();
        let id = s.id();
        s.immediate.push_back((id, request, now_ms));
        id
    }

    /// Gates transmission, e.g. until notifications are subscribed.
    pub fn set_link_ready(&self, ready: bool) {
        self.inner.lock().unwrap().link_ready = ready;
    }

    /// The writes to perform now.
    pub fn poll_transmit(&self, now_ms: u64) -> Vec<OutgoingWrite> {
        let mut s = self.inner.lock().unwrap();
        if !s.link_ready {
            return Vec::new();
        }
        let mut out = Vec::new();
        while let Some((_, _, queued_at)) = s.immediate.front() {
            if s.current.is_some() && now_ms.saturating_sub(*queued_at) < IMMEDIATE_MAX_DEFER_MS {
                break;
            }
            let (id, request, _) = s.immediate.pop_front().unwrap();
            out.extend(Inner::writes(id, &request, false));
        }
        while s.current.is_none() && !s.queue.is_empty() {
            let Queued { id, request, .. } = s.queue.remove(0);
            let awaits = request.expects_response();
            out.extend(Inner::writes(id, &request, awaits));
            if awaits {
                let deadline_ms = now_ms + u64::from(request.timeout_ms());
                s.current = Some(InFlight {
                    id,
                    matcher: Matcher::new(request),
                    deadline_ms,
                });
            }
        }
        out
    }

    /// Feeds one notification. `now_ms` is Unix time in milliseconds.
    pub fn handle_notification(
        &self,
        channel: Channel,
        data: Vec<u8>,
        now_ms: u64,
    ) -> Vec<SessionEvent> {
        let mut s = self.inner.lock().unwrap();
        let packets = s.decoder.feed(channel, &data);
        let now_unix = (now_ms / 1000) as i64;
        let mut events = Vec::new();
        for packet in packets {
            let message = classify(&packet);
            // Phone-GPS events on the remote channel never answer a command.
            let routable = !(packet.channel == Channel::Remote && packet.command == 0xE9);
            let outcome = match s.current.as_mut() {
                Some(current) if routable => {
                    Some((current.id, current.matcher.offer(&packet, now_unix)))
                }
                _ => None,
            };
            let (consumed_by, follow) = match outcome {
                Some((id, MatchOutcome::Progress { received, total })) => (
                    Some(id),
                    Some(SessionEvent::Progress {
                        request_id: id,
                        received,
                        total,
                    }),
                ),
                Some((id, MatchOutcome::Complete { response })) => {
                    s.current = None;
                    (
                        Some(id),
                        Some(SessionEvent::Completed {
                            request_id: id,
                            response,
                        }),
                    )
                }
                _ => (None, None),
            };
            events.push(SessionEvent::Received {
                packet,
                message,
                consumed_by,
            });
            events.extend(follow);
        }
        events
    }

    /// Expires the in-flight request if its deadline has passed.
    pub fn tick(&self, now_ms: u64) -> Vec<SessionEvent> {
        let mut s = self.inner.lock().unwrap();
        match &s.current {
            Some(c) if now_ms >= c.deadline_ms => {
                let c = s.current.take().unwrap();
                vec![SessionEvent::TimedOut {
                    request_id: c.id,
                    partial: c.matcher.timeout_fallback(),
                }]
            }
            _ => Vec::new(),
        }
    }

    /// When to call [`Session::tick`] and [`Session::poll_transmit`] next.
    pub fn next_deadline_ms(&self) -> Option<u64> {
        let s = self.inner.lock().unwrap();
        let current = s.current.as_ref().map(|c| c.deadline_ms);
        let immediate = s
            .current
            .as_ref()
            .and(s.immediate.front())
            .map(|(_, _, at)| at + IMMEDIATE_MAX_DEFER_MS);
        current.into_iter().chain(immediate).min()
    }

    pub fn is_awaiting_response(&self) -> bool {
        self.inner.lock().unwrap().current.is_some()
    }

    /// The in-flight request's ID, if any.
    pub fn current_request(&self) -> Option<u64> {
        self.inner.lock().unwrap().current.as_ref().map(|c| c.id)
    }

    pub fn pending_count(&self) -> u32 {
        let s = self.inner.lock().unwrap();
        (s.queue.len() + s.immediate.len()) as u32
    }

    /// Drops the in-flight request so the queue can move on.
    pub fn cancel_current(&self) -> Option<SessionEvent> {
        let mut s = self.inner.lock().unwrap();
        s.current
            .take()
            .map(|c| SessionEvent::Cancelled { request_id: c.id })
    }

    /// Drops everything, e.g. on disconnect, and resets reassembly.
    pub fn clear(&self) -> Vec<SessionEvent> {
        let mut s = self.inner.lock().unwrap();
        let mut ids: Vec<u64> = s.current.take().map(|c| c.id).into_iter().collect();
        ids.extend(s.queue.drain(..).map(|q| q.id));
        ids.extend(s.immediate.drain(..).map(|(id, _, _)| id));
        s.decoder.reset();
        ids.into_iter()
            .map(|request_id| SessionEvent::Cancelled { request_id })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::{encode_6f, Action};
    use crate::model::BatteryStatus;

    fn session() -> Session {
        let s = Session::new();
        s.set_link_ready(true);
        s
    }

    #[test]
    fn one_reply_in_flight_and_priority_order() {
        let s = session();
        let normal = s.enqueue(Request::GetBattery { new_mode: true }, Priority::Normal);
        let bind = s.enqueue(
            Request::BindStart {
                mode: crate::model::BindMode::QrCode,
            },
            Priority::Bind,
        );
        let writes = s.poll_transmit(0);
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0].request_id, bind);
        assert!(s.poll_transmit(1).is_empty());

        let ack = encode_6f(0x01, Action::SetResponse, &[0x93, 0x00]);
        let events = s.handle_notification(Channel::Primary, ack, 2);
        assert!(
            matches!(events[1], SessionEvent::Completed { request_id, .. } if request_id == bind)
        );

        let writes = s.poll_transmit(3);
        assert_eq!(writes[0].request_id, normal);
        let reply = encode_6f(0x08, Action::CheckResponse, &[0xCD]);
        let events = s.handle_notification(Channel::Primary, reply, 4);
        assert_eq!(
            events[1],
            SessionEvent::Completed {
                request_id: normal,
                response: Response::Battery {
                    status: BatteryStatus {
                        level: 77,
                        charging: true
                    }
                }
            }
        );
    }

    #[test]
    fn write_only_requests_flow_through_and_timeouts_fire() {
        let s = session();
        s.enqueue(
            Request::PhoneGpsStatus {
                gps_available: true,
            },
            Priority::Normal,
        );
        let id = s.enqueue(Request::GetWatchId, Priority::Normal);
        let writes = s.poll_transmit(0);
        assert_eq!(writes.len(), 2);
        assert_eq!(writes[0].channel, WriteChannel::Remote);
        assert!(!writes[0].awaits_response && writes[1].awaits_response);
        assert_eq!(s.next_deadline_ms(), Some(10_000));
        assert!(s.tick(9_999).is_empty());
        assert_eq!(
            s.tick(10_000),
            vec![SessionEvent::TimedOut {
                request_id: id,
                partial: None
            }]
        );
        assert!(!s.is_awaiting_response());
    }

    #[test]
    fn immediates_wait_for_the_reply_then_go() {
        let s = session();
        s.enqueue(Request::GetGoals, Priority::Normal);
        s.poll_transmit(0);
        let imm = s.send_immediate(Request::SetBrightness { level: 3 }, 100);
        assert!(s.poll_transmit(200).is_empty());
        assert_eq!(s.next_deadline_ms(), Some(10_000));
        let forced = s.poll_transmit(10_100);
        assert_eq!(forced.len(), 1);
        assert_eq!(forced[0].request_id, imm);
        assert!(!forced[0].awaits_response);
    }

    #[test]
    fn gps_events_on_the_remote_channel_are_not_consumed() {
        let s = session();
        s.enqueue(Request::GetBattery { new_mode: true }, Priority::Normal);
        s.poll_transmit(0);
        let gps = encode_6f(0xE9, Action::Check, &[0x00, 0x01, 0x05]);
        let events = s.handle_notification(Channel::Remote, gps, 1);
        assert_eq!(events.len(), 1);
        assert!(matches!(
            &events[0],
            SessionEvent::Received {
                consumed_by: None,
                ..
            }
        ));
        assert!(s.is_awaiting_response());
    }

    #[test]
    fn nothing_is_sent_until_the_link_is_ready() {
        let s = Session::new();
        s.enqueue(Request::GetBattery { new_mode: true }, Priority::Normal);
        assert!(s.poll_transmit(0).is_empty());
        s.set_link_ready(true);
        assert_eq!(s.poll_transmit(0).len(), 1);
        assert_eq!(s.clear().len(), 1);
    }
}
