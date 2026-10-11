#!/usr/bin/env python3
"""NormConsole: read a Norm Two's output and send it commands.

    normconsole.py --emulator                 # in-process emulated watch (needs NormCrack)
    normconsole.py --bridge 127.0.0.1:7333    # tools/emulator/bridge.py
    normconsole.py --ble [NAME_OR_ADDRESS]    # a real watch over Bluetooth (pip install bleak)

    normconsole.py --emulator --run battery.get --run "settings.brightness.set level=3"

Commands at the prompt:

    list [text]            catalog entries, optionally filtered
    info ID                the annotation for one command
    send ID [k=v ...]      send a command; omitted parameters use the example
                           values. Values are Python expressions, with the SDK
                           available as `n` (e.g. unit=n.TemperatureUnit.FAHRENHEIT)
    raw HEX [remote]       write a raw frame to 8001 (or 8003)
    quit

Everything the watch sends is printed as it arrives.
"""

import argparse
import asyncio
import inspect
import os
import shlex
import sys
import threading
import time

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))
sys.path.insert(0, os.path.join(REPO, 'python'))
sys.path.insert(0, os.path.join(REPO, 'tools', 'emulator'))

import normsdk as n  # noqa: E402

PROFILE = n.gatt_profile()
NOTIFY = {
    PROFILE.command_notify.lower(): n.Channel.PRIMARY,
    PROFILE.remote_notify.lower(): n.Channel.REMOTE,
    PROFILE.transparent_notify.lower(): n.Channel.TRANSPARENT,
}


def now_ms():
    return int(time.time() * 1000)


# ---- transports ------------------------------------------------------------
# Each transport calls `on_notify(channel, data)` and offers `write(channel, data)`.

class EmulatorTransport:
    def __init__(self, on_notify):
        from emulated_watch import EmulatedWatch
        print('booting the emulated watch…')
        self.watch = EmulatedWatch()
        self.on_notify = on_notify
        self.lock = threading.Lock()

    async def start(self):
        asyncio.get_running_loop().create_task(self._run())

    async def _run(self):
        last = time.monotonic()
        while True:
            await asyncio.sleep(0.01)
            now = time.monotonic()
            with self.lock:
                notes = self.watch.run(max(1, min(50, int((now - last) * 1000))))
            last = now
            for channel, data in notes:
                self.on_notify(channel, data)

    async def write(self, channel, data):
        with self.lock:
            self.watch.write(channel, data)


class BridgeTransport:
    def __init__(self, address, on_notify):
        host, _, port = address.partition(':')
        self.host, self.port = host or '127.0.0.1', int(port or 7333)
        self.on_notify = on_notify

    async def start(self):
        import json
        self.reader, self.writer = await asyncio.open_connection(self.host, self.port)
        hello = json.loads(await self.reader.readline())
        print('connected to %s (%s)' % (hello.get('name'), hello.get('firmware')))

        async def pump():
            while line := await self.reader.readline():
                message = json.loads(line)
                if message.get('type') == 'notify':
                    channel = n.Channel.REMOTE if message['channel'] == 'remote' else n.Channel.PRIMARY
                    self.on_notify(channel, bytes.fromhex(message['data']))
            print('bridge closed')

        asyncio.get_running_loop().create_task(pump())

    async def write(self, channel, data):
        import json
        name = 'remote' if channel == n.WriteChannel.REMOTE else 'primary'
        self.writer.write((json.dumps({'type': 'write', 'channel': name, 'data': bytes(data).hex()}) + '\n').encode())
        await self.writer.drain()


class BleTransport:
    def __init__(self, target, on_notify):
        self.target = target
        self.on_notify = on_notify

    async def start(self):
        from bleak import BleakClient, BleakScanner
        if self.target and ':' in self.target or (self.target and len(self.target) == 36):
            device = await BleakScanner.find_device_by_address(self.target, timeout=15)
        else:
            device = await BleakScanner.find_device_by_filter(
                lambda d, _: n.is_norm_device_name(d.name) and (not self.target or (d.name or '').startswith(self.target)),
                timeout=15)
        if device is None:
            raise SystemExit('no Norm Two found')
        print('connecting to %s (%s)…' % (device.name, device.address))
        self.client = BleakClient(device)
        await self.client.connect()
        for uuid, channel in NOTIFY.items():
            try:
                await self.client.start_notify(uuid, lambda _, data, ch=channel: self.on_notify(ch, bytes(data)))
            except Exception as exc:  # noqa: BLE001 - optional characteristics
                if channel == n.Channel.PRIMARY:
                    raise
                print('note: %s unavailable (%s)' % (uuid, exc))
        print('connected')

    async def write(self, channel, data):
        uuid = PROFILE.remote_write if channel == n.WriteChannel.REMOTE else PROFILE.command_write
        char = self.client.services.get_characteristic(uuid)
        with_response = 'write' in char.properties
        size = max(20, char.max_write_without_response_size)
        for chunk in n.chunk_frame(bytes(data), size):
            await self.client.write_gatt_char(char, chunk, response=with_response)


# ---- console ---------------------------------------------------------------

class Console:
    def __init__(self):
        self.session = n.Session()
        self.transport = None
        self.waiters = {}
        self.catalog = {spec.id: spec for spec in n.command_catalog()}

    def on_notify(self, channel, data):
        self.handle(self.session.handle_notification(channel, data, now_ms()))

    def handle(self, events):
        for e in events:
            if e.is_RECEIVED():
                frame = n.encode_frame_6f(e.packet.command, e.packet.action, e.packet.payload) \
                    if e.packet.format == n.FrameFormat.FRAME6F else n.encode_frame_6e(e.packet.command, e.packet.payload)
                tag = '' if e.consumed_by is None else ' (#%d)' % e.consumed_by
                print('  watch  %s%s\n         %s' % (n.hex_string(frame), tag, n.describe_message(e.message)))
            elif e.is_PROGRESS():
                print('  #%d progress %d/%d' % (e.request_id, e.received, e.total))
            elif e.is_COMPLETED():
                print('  #%d → %s' % (e.request_id, n.describe_response(e.response)))
                self._finish(e.request_id)
            elif e.is_TIMED_OUT():
                partial = n.describe_response(e.partial) if e.partial else 'nothing'
                print('  #%d timed out (%s)' % (e.request_id, partial))
                self._finish(e.request_id)

    def _finish(self, request_id):
        waiter = self.waiters.pop(request_id, None)
        if waiter and not waiter.done():
            waiter.set_result(True)

    async def pump(self):
        while True:
            for write in self.session.poll_transmit(now_ms()):
                print('  phone  %s  [%s]' % (n.hex_string(write.data), write.label))
                await self.transport.write(write.channel, write.data)
                if not write.awaits_response:
                    self._finish(write.request_id)
            self.handle(self.session.tick(now_ms()))
            await asyncio.sleep(0.02)

    def build(self, spec, assignments):
        request = n.example_request(spec.kind)
        if not assignments:
            return request
        variant = type(request)
        params = inspect.signature(variant.__init__).parameters
        values = {name: getattr(request, name) for name in params if name != 'self'}
        for item in assignments:
            key, _, expr = item.partition('=')
            if key not in values:
                raise ValueError('%s takes %s' % (spec.id, ', '.join(values) or 'no parameters'))
            values[key] = eval(expr, {'n': n})  # noqa: S307 - local developer console
        return variant(**values)

    async def send(self, request):
        rid = self.session.enqueue(request, n.Priority.NORMAL)
        done = asyncio.get_running_loop().create_future()
        self.waiters[rid] = done
        try:
            await asyncio.wait_for(done, n.request_info(request).timeout_ms / 1000 + 2)
        except asyncio.TimeoutError:
            pass
        await asyncio.sleep(0.2)

    async def command(self, line):
        words = shlex.split(line)
        if not words:
            return True
        verb, args = words[0], words[1:]
        if verb in ('quit', 'exit'):
            return False
        if verb == 'help':
            print(__doc__)
        elif verb == 'list':
            for spec in self.catalog.values():
                if not args or args[0].lower() in (spec.id + spec.title).lower():
                    print('  %-34s %s' % (spec.id, spec.title))
        elif verb == 'info' and args:
            spec = self.catalog.get(args[0])
            if not spec:
                print('unknown command %s' % args[0])
            else:
                for field in ('title', 'summary', 'request_layout', 'response_layout', 'firmware', 'evidence', 'notes'):
                    print('  %-16s %s' % (field, getattr(spec, field)))
                print('  %-16s %s' % ('example', n.describe_request(n.example_request(spec.kind))))
        elif verb == 'send' and args:
            spec = self.catalog.get(args[0])
            if not spec:
                print('unknown command %s (try `list`)' % args[0])
                return True
            if n.is_destructive(spec.kind) and not isinstance(self.transport, (EmulatorTransport, BridgeTransport)):
                answer = await asyncio.to_thread(input, '%s is destructive on a real watch. Type yes: ' % spec.id)
                if answer.strip() != 'yes':
                    return True
            try:
                request = self.build(spec, args[1:])
            except Exception as exc:  # noqa: BLE001
                print('error: %s' % exc)
                return True
            await self.send(request)
        elif verb == 'raw' and args:
            remote = args[-1] == 'remote'
            channel = n.WriteChannel.REMOTE if remote else n.WriteChannel.PRIMARY
            frame = bytes.fromhex(''.join(args[:-1] if remote else args))
            await self.send(n.Request.RAW(label='console', frame=frame, channel=channel,
                                          response_command=None, timeout_ms=0))
        else:
            print('unknown command; type help')
        return True

    async def main(self, args):
        if args.emulator:
            self.transport = EmulatorTransport(self.on_notify)
        elif args.bridge:
            self.transport = BridgeTransport(args.bridge, self.on_notify)
        else:
            self.transport = BleTransport(args.ble, self.on_notify)
        await self.transport.start()
        self.session.set_link_ready(True)
        asyncio.get_running_loop().create_task(self.pump())
        for line in args.run:
            print('> ' + line)
            await self.command(line)
        if args.run and not args.interactive:
            return
        while True:
            line = await asyncio.to_thread(input, 'norm> ')
            if not await self.command(line):
                return


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    target = ap.add_mutually_exclusive_group(required=True)
    target.add_argument('--emulator', action='store_true')
    target.add_argument('--bridge', metavar='HOST:PORT')
    target.add_argument('--ble', nargs='?', const='', metavar='NAME_OR_ADDRESS')
    ap.add_argument('--run', action='append', default=[], metavar='COMMAND', help='run a console command')
    ap.add_argument('--interactive', action='store_true', help='stay at the prompt after --run')
    args = ap.parse_args()
    try:
        asyncio.run(Console().main(args))
    except (KeyboardInterrupt, EOFError):
        pass


if __name__ == '__main__':
    main()
