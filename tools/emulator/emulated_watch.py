"""A Norm Two running its stock firmware under NormCrack's CPU emulator.

`EmulatedWatch` looks like the watch's GATT link: write whole or fragmented
frames to it, advance time, and read notifications back. It needs a NormCrack
checkout (the emulator, the stock image and the resource-flash dump), found
via `NORMCRACK_DIR` or `../NormCrack` next to this repo.

Phone writes run through the firmware's own receive path; replies are taken
where the firmware hands frames to its BLE sender and are re-framed as 6F
notifications, split to `mtu` bytes to exercise reassembly.
"""

import datetime
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))
sys.path.insert(0, os.path.join(REPO, 'python'))

import normsdk  # noqa: E402

# Frames the firmware starts itself on the remote characteristic (8004).
REMOTE_COMMANDS = {0xE2, 0xE9}


def normcrack_dir():
    path = os.environ.get('NORMCRACK_DIR') or os.path.join(os.path.dirname(REPO), 'NormCrack')
    if not os.path.exists(os.path.join(path, 'tools', 'fwemu.py')):
        return None
    return path


def load_fwemu():
    path = normcrack_dir()
    if path is None:
        raise RuntimeError('NormCrack checkout not found; set NORMCRACK_DIR')
    tools = os.path.join(path, 'tools')
    if tools not in sys.path:
        sys.path.insert(0, tools)
    import fwemu  # noqa: E402
    return fwemu


class EmulatedWatch:
    def __init__(self, paired=True, boot_ms=4000, clock=None, mtu=20, log=None):
        fwemu = load_fwemu()
        self.mtu = mtu
        self._log = log or (lambda *_: None)
        self.machine = fwemu.Machine(
            log=lambda *_: None,
            state=fwemu.STATE if paired else None,
            clock=clock or datetime.datetime(2026, 10, 11, 12, 0, 0),
            fast_runtime=True,
            cycles_per_instruction=10,
            slice_us=1000,
        )
        self._decoders = {ch: normsdk.FrameDecoder() for ch in (normsdk.Channel.PRIMARY, normsdk.Channel.REMOTE)}
        self._sent = 0
        self.machine.run(boot_ms)
        self._sent = len(self.machine.ble_tx)

    @property
    def now_ms(self):
        return self.machine.now_us // 1000

    def write(self, channel, data):
        """A phone write to 8001 (`WriteChannel.PRIMARY`) or 8003 (`REMOTE`).
        Fragments are buffered until a frame is complete."""
        decoder = self._decoders[normsdk.Channel.REMOTE if channel == normsdk.WriteChannel.REMOTE
                                 else normsdk.Channel.PRIMARY]
        for packet in decoder.feed(normsdk.Channel.PRIMARY, bytes(data)):
            frame = normsdk.encode_frame_6f(packet.command, packet.action, packet.payload) \
                if packet.format == normsdk.FrameFormat.FRAME6F \
                else normsdk.encode_frame_6e(packet.command, packet.payload)
            self._log('phone -> watch ' + normsdk.hex_string(frame))
            self.machine.ble_rx.append(bytes(frame))

    def run(self, ms):
        """Advances the watch by `ms` and returns the notifications it sent as
        `(Channel, bytes)` pairs, fragmented to the MTU."""
        self.machine.run(ms)
        out = []
        for command, action, payload in self.machine.ble_tx[self._sent:]:
            frame = normsdk.encode_frame_6f(command, _action(action), bytes(payload))
            channel = normsdk.Channel.REMOTE if command in REMOTE_COMMANDS else normsdk.Channel.PRIMARY
            self._log('watch -> phone ' + normsdk.hex_string(frame))
            out.extend((channel, bytes(chunk)) for chunk in normsdk.chunk_frame(frame, self.mtu))
        self._sent = len(self.machine.ble_tx)
        return out

    def screenshot(self, path):
        self.machine.save_png(path)


def _action(byte):
    return {
        0x70: normsdk.Action.CHECK(),
        0x71: normsdk.Action.SET(),
        0x80: normsdk.Action.CHECK_RESPONSE(),
        0x81: normsdk.Action.SET_RESPONSE(),
    }.get(byte) or normsdk.Action.OTHER(raw=byte)


class Driver:
    """Runs a `normsdk.Session` against an `EmulatedWatch` on emulated time."""

    def __init__(self, watch, step_ms=25):
        self.watch = watch
        self.session = normsdk.Session()
        self.session.set_link_ready(True)
        self.step_ms = step_ms
        self.received = []

    def _pump(self):
        events = []
        for write in self.session.poll_transmit(self.watch.now_ms):
            self.watch.write(write.channel, write.data)
        for channel, chunk in self.watch.run(self.step_ms):
            events += self.session.handle_notification(channel, chunk, self.watch.now_ms)
        events += self.session.tick(self.watch.now_ms)
        for e in events:
            if e.is_RECEIVED():
                self.received.append(e.packet)
        return events

    def call(self, request, priority=None, settle_ms=150):
        """Sends one request and returns `(event, frames_sent, packets_received)`
        where `event` is its Completed/TimedOut event, or None for write-only
        requests (after `settle_ms` of replies)."""
        sent = [bytes(f.data) for f in normsdk.encode_request(request)]
        start = len(self.received)
        rid = self.session.enqueue(request, priority or normsdk.Priority.NORMAL)
        info = normsdk.request_info(request)
        deadline = self.watch.now_ms + info.timeout_ms + 1000
        result = None
        while self.watch.now_ms < deadline:
            for e in self._pump():
                if (e.is_COMPLETED() or e.is_TIMED_OUT()) and e.request_id == rid:
                    result = e
            if result is not None:
                break
            if not info.expects_response and not self.session.is_awaiting_response() \
                    and self.session.pending_count() == 0:
                end = self.watch.now_ms + settle_ms
                while self.watch.now_ms < end:
                    self._pump()
                break
        return result, sent, self.received[start:]
