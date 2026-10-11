"""Python bindings smoke tests (no emulator needed)."""

import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

import normsdk as n  # noqa: E402


class Bindings(unittest.TestCase):
    def test_encode_and_catalog(self):
        frames = n.encode_request(n.Request.GET_BATTERY(new_mode=True))
        self.assertEqual(bytes(frames[0].data), bytes([0x6F, 0x08, 0x70, 0x01, 0x00, 0x01, 0x8F]))
        self.assertEqual(len(n.command_catalog()), 93)
        self.assertTrue(n.command_spec(n.CommandKind.SET_LANGUAGE).firmware.is_NO_STOCK_HANDLER())

    def test_session_round_trip(self):
        session = n.Session()
        session.set_link_ready(True)
        rid = session.enqueue(n.Request.GET_GOALS(), n.Priority.NORMAL)
        self.assertEqual(len(session.poll_transmit(0)), 1)
        reply = n.encode_frame_6f(0x50, n.Action.CHECK_RESPONSE(), bytes.fromhex('460001c800010500010800011e0001'))
        events = []
        for chunk in n.chunk_frame(reply, 7):
            events += session.handle_notification(n.Channel.PRIMARY, chunk, 1)
        done = events[-1]
        self.assertTrue(done.is_COMPLETED())
        self.assertEqual(done.request_id, rid)
        self.assertEqual(done.response.goals.steps, 7000)
        self.assertEqual(done.response.goals.sport_minutes, 30)

    def test_watch_events(self):
        packet = n.FrameDecoder().feed(n.Channel.PRIMARY, n.encode_frame_6f(0xD0, n.Action.SET(), b'\x03'))[0]
        message = n.classify_packet(packet)
        self.assertTrue(message.is_MUSIC_CONTROL())
        self.assertTrue(message.action.is_NEXT())


if __name__ == '__main__':
    unittest.main(verbosity=2)
