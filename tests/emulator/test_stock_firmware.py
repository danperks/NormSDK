"""NormSDK against the stock Norm Two firmware, run under NormCrack's emulator.

    python3 tests/emulator/test_stock_firmware.py            # run
    python3 tests/emulator/test_stock_firmware.py --record   # also rewrite docs/emulator-results.json

Every catalog command's example request goes through a real `normsdk.Session`
to the emulated firmware, and the outcome must match what the command's
annotation predicts (stock handler → ack 0 or a parsed reply, no handler →
ack 2, 6E → ignored). Round-trip tests then write settings and read them
back. Skipped when no NormCrack checkout is available.
"""

import json
import os
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))
sys.path.insert(0, os.path.join(REPO, 'tools', 'emulator'))
sys.path.insert(0, os.path.join(REPO, 'python'))

import normsdk as n  # noqa: E402
from emulated_watch import Driver, EmulatedWatch, normcrack_dir  # noqa: E402

RESULTS = os.path.join(REPO, 'docs', 'emulator-results.json')
K = n.CommandKind

# Where the stock firmware's answer differs from the default expectation for
# its support level. Each is a firmware fact, not an SDK quirk.
OVERRIDES = {
    K.GET_DEVICE_VERSION_NO_ARGUMENT: 'timeout',  # empty-payload frames are dropped
    K.SET_VIBRATION: 'ack:1',                     # 0xF0 rejects reads and writes
    K.GET_VIBRATION: 'ack:1',
    K.GET_SLEEP_RECORDS: 'ack:1',                 # nothing stored on the emulated watch
    K.GET_HEART_RATE_RECORDS: 'ack:1',
    K.GET_WORKOUTS: 'ack:1',
    K.GET_ALARMS: 'ack:1',
    K.FIND_PHONE_START_ACK: 'sent',               # replies; the watch does not answer them
    K.PHONE_GPS_STATUS: 'sent',
    K.PHONE_GPS_DISTANCE: 'sent',
    K.PHONE_GPS_LOCATION: 'sent',
    K.GET_CAPABILITIES: 'reply:CAPABILITIES',     # stock echoes an empty 0x2A = no capabilities
    K.READ_MEMORY: 'timeout',                     # stock echoes 0x2A; the probe waits for 0xDC
    K.READ_RESOURCE: 'timeout',
    K.GET_PAGE_ROUTE_TRACE: 'unparsed',           # stock echoes 0x2A without a trace
    K.SET_CUSTOM_TEXT: 'sent',
    K.PUSH_CALENDAR_EVENTS: 'sent',
    K.ENTER_UPGRADE_MODE: 'sent',
    K.RAW: 'reply:PACKET',                        # the example is a battery read
}

DESTRUCTIVE_LAST = [k for k in (getattr(K, name) for name in dir(K) if name.isupper()) if n.is_destructive(k)]


def variant(obj):
    return type(obj).__name__.split('.')[-1]


def default_expectation(spec):
    firmware = spec.firmware
    if firmware.is_IGNORED6E():
        return 'timeout' if spec.expects_response else 'sent'
    if firmware.is_NO_STOCK_HANDLER():
        return 'ack:2' if spec.expects_response else 'sent'
    if firmware.is_PATCHED():
        return None
    if not spec.expects_response:
        return 'sent'
    if spec.action is not None and spec.action.is_CHECK():
        return 'reply'
    return 'ack:0'


def outcome(event):
    if event is None:
        return 'sent'
    if event.is_TIMED_OUT():
        return 'timeout'
    response = event.response
    if response.is_ACK():
        return 'ack:%d' % response.status
    if response.is_UNPARSED():
        return 'unparsed'
    return 'reply:' + variant(response)


def matches(expected, actual):
    if expected is None:
        return True
    return actual == expected or (expected == 'reply' and actual.startswith('reply:'))


@unittest.skipIf(normcrack_dir() is None, 'NormCrack checkout not found (set NORMCRACK_DIR)')
class CatalogAgainstStockFirmware(unittest.TestCase):
    results = {}

    @classmethod
    def setUpClass(cls):
        cls.driver = Driver(EmulatedWatch())

    def test_every_command_matches_its_annotation(self):
        catalog = n.command_catalog()
        ordered = [s for s in catalog if s.kind not in DESTRUCTIVE_LAST] + \
                  [s for s in catalog if s.kind in DESTRUCTIVE_LAST]
        failures = []
        for spec in ordered:
            request = n.example_request(spec.kind)
            event, sent, received = self.driver.call(request)
            actual = outcome(event)
            expected = OVERRIDES.get(spec.kind, default_expectation(spec))
            self.results[spec.id] = {
                'outcome': actual,
                'expected': expected,
                'sent': [n.hex_string(f) for f in sent],
                'received': [n.hex_string(n.encode_frame_6f(p.command, p.action, p.payload)) for p in received
                             if p.format == n.FrameFormat.FRAME6F],
                'response': n.describe_response(event.response) if event is not None and event.is_COMPLETED() else None,
            }
            if not matches(expected, actual):
                failures.append('%s: expected %s, got %s (received %s)' % (
                    spec.id, expected, actual, self.results[spec.id]['received']))
        self.assertEqual(failures, [], '\n' + '\n'.join(failures))

    @classmethod
    def tearDownClass(cls):
        if '--record' in sys.argv and cls.results:
            os.makedirs(os.path.dirname(RESULTS), exist_ok=True)
            with open(RESULTS, 'w') as f:
                json.dump({'firmware': 'Apollo3_P03B_NORM2_F0.2B01 (stock), NormCrack fwemu',
                           'commands': cls.results}, f, indent=1, sort_keys=True)
                f.write('\n')


@unittest.skipIf(normcrack_dir() is None, 'NormCrack checkout not found (set NORMCRACK_DIR)')
class RoundTrips(unittest.TestCase):
    """Writes, then reads back through the firmware's own storage."""

    @classmethod
    def setUpClass(cls):
        cls.driver = Driver(EmulatedWatch())

    def call(self, request):
        event, _, _ = self.driver.call(request)
        self.assertIsNotNone(event, request)
        self.assertTrue(event.is_COMPLETED(), '%s timed out' % request)
        return event.response

    def write(self, request):
        response = self.call(request)
        self.assertTrue(response.is_ACK(), response)
        self.assertEqual(response.status, 0, response)

    def test_identity(self):
        version = self.call(n.Request.GET_DEVICE_VERSION())
        self.assertEqual(version.text, 'A0.2R0.4T0.0H0.0B0.1')
        self.assertFalse(version.parsed.is_patched)
        battery = self.call(n.Request.GET_BATTERY(new_mode=True))
        self.assertTrue(0 < battery.status.level <= 100)

    def test_bind(self):
        self.write(n.Request.BIND_START(mode=n.BindMode.QR_CODE()))
        self.write(n.Request.BIND_END())

    def test_brightness(self):
        for level in (2, 5, 3):
            self.write(n.Request.SET_BRIGHTNESS(level=level))
            self.assertEqual(self.call(n.Request.GET_BRIGHTNESS()).level, level)

    def test_screen_timeout(self):
        self.write(n.Request.SET_SCREEN_TIMEOUT(seconds=28))
        self.assertEqual(self.call(n.Request.GET_SCREEN_TIMEOUT()).seconds, 30)

    def test_unit(self):
        self.write(n.Request.SET_UNIT(metric=False))
        self.assertFalse(self.call(n.Request.GET_UNIT()).metric)
        self.write(n.Request.SET_UNIT(metric=True))
        self.assertTrue(self.call(n.Request.GET_UNIT()).metric)

    def test_switches(self):
        for on in (False, True):
            self.write(n.Request.SET_RAISE_TO_WAKE(enabled=on))
            self.write(n.Request.SET_AUTO_SLEEP_SWITCH(enabled=on))
            states = self.call(n.Request.GET_SWITCHES()).states
            self.assertEqual((states.raise_to_wake, states.auto_sleep), (on, on))

    def test_sleep_window(self):
        window = n.SleepWindow(start_hour=23, start_minute=15, end_hour=6, end_minute=45, weekdays=0x1F)
        self.write(n.Request.SET_SLEEP_WINDOW(window=window))
        self.assertEqual(self.call(n.Request.GET_SLEEP_WINDOW()).window, window)

    def test_goals(self):
        self.write(n.Request.SET_GOAL(kind=n.GoalKind.STEPS, value=12345))
        self.write(n.Request.SET_GOAL(kind=n.GoalKind.CALORIES, value=450))
        goals = self.call(n.Request.GET_GOALS()).goals
        self.assertEqual((goals.steps, goals.calories), (12300, 450))

    def test_auto_heart_rate(self):
        self.write(n.Request.SET_AUTO_HEART_RATE(interval_minutes=15))
        value = self.call(n.Request.GET_AUTO_HEART_RATE()).value
        self.assertEqual((value.enabled, value.interval_minutes), (True, 15))

    def test_alarm(self):
        free = self.call(n.Request.GET_FREE_ALARM_ID()).id
        alarm = n.Alarm(id=free, enabled=True, hour=6, minute=50, weekdays=0x41, snooze_minutes=5)
        self.write(n.Request.SET_ALARM(alarm=alarm, is_new=True))
        counts = self.call(n.Request.GET_ALARM_COUNTS()).counts
        alarms = self.call(n.Request.GET_ALARMS(expected=counts.alarms)).alarms
        self.assertIn(alarm, alarms)

    def test_date_time(self):
        time = n.LocalDateTime(year=2027, month=2, day=3, hour=4, minute=5, second=6, utc_offset_minutes=0)
        self.write(n.Request.SET_DATE_TIME(time=time, analog_hand_calibration=False))
        reply = self.call(n.Request.RAW(label='time-read', frame=n.encode_frame_6f(0x04, n.Action.CHECK(), b'\x00'),
                                        channel=n.WriteChannel.PRIMARY, response_command=0x04, timeout_ms=2000))
        self.assertEqual(bytes(reply.packet.payload[:6]), bytes([0xEB, 0x07, 2, 3, 4, 5]))

    def test_watch_id(self):
        self.assertEqual(self.call(n.Request.GET_WATCH_ID()).id, 'FCP03A18050801000000')

    def test_pushes(self):
        self.write(n.Request.SET_TEMPERATURE_UNIT(unit=n.TemperatureUnit.CELSIUS))
        self.write(n.example_request(K.PUSH_WEATHER))
        self.write(n.Request.PUSH_MUSIC_VOLUME(percent=40))
        self.write(n.Request.FIND_PHONE_FOUND())

    def test_page_order_reports_the_stock_count(self):
        response = self.call(n.Request.GET_PAGE_ORDER(source=n.PageOrderSource.APP_SETTING))
        self.assertEqual(response.reported_count, 5)

    def test_unhandled_and_6e_commands(self):
        response = self.call(n.Request.SET_LANGUAGE(language=0))
        self.assertEqual((response.command, response.status), (0x09, 2))
        event, _, received = self.driver.call(n.Request.GET_BATTERY6E())
        self.assertTrue(event.is_TIMED_OUT())
        self.assertEqual(received, [])


if __name__ == '__main__':
    argv = [a for a in sys.argv if a != '--record']
    unittest.main(argv=argv, verbosity=2)
