#!/usr/bin/env python3
"""Serves an emulated Norm Two over TCP so any client can talk to it like a
GATT link. Runs the stock firmware in real time.

    python3 tools/emulator/bridge.py [--port 7333] [--fresh] [--verbose]

One JSON object per line:

    server → client  {"type": "hello", "name": "Norm2#EMU", "firmware": "…"}
    client → server  {"type": "write", "channel": "primary" | "remote", "data": "6F08700100018F"}
    server → client  {"type": "notify", "channel": "primary" | "remote", "data": "…"}

Android emulators reach the host as 10.0.2.2; the iOS simulator as 127.0.0.1.
"""

import argparse
import json
import os
import select
import socket
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from emulated_watch import EmulatedWatch, normsdk  # noqa: E402

CHANNELS = {normsdk.Channel.PRIMARY: 'primary', normsdk.Channel.REMOTE: 'remote'}
WRITE_CHANNELS = {'primary': normsdk.WriteChannel.PRIMARY, 'remote': normsdk.WriteChannel.REMOTE}


def serve(watch, port, verbose):
    server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    server.bind(('0.0.0.0', port))
    server.listen(1)
    print('emulated Norm2 listening on :%d' % port, flush=True)
    client, buffer = None, b''
    last = time.monotonic()
    while True:
        sockets = [server] + ([client] if client else [])
        readable, _, _ = select.select(sockets, [], [], 0.01)
        for s in readable:
            if s is server:
                if client:
                    client.close()
                client, _ = server.accept()
                client.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
                buffer = b''
                hello = {'type': 'hello', 'name': 'Norm2#EMU', 'firmware': 'Apollo3_P03B_NORM2_F0.2B01 (stock)'}
                client.sendall((json.dumps(hello) + '\n').encode())
                print('client connected', flush=True)
                continue
            chunk = s.recv(65536)
            if not chunk:
                print('client disconnected', flush=True)
                client.close()
                client = None
                continue
            buffer += chunk
            while b'\n' in buffer:
                line, buffer = buffer.split(b'\n', 1)
                if not line.strip():
                    continue
                message = json.loads(line)
                if message.get('type') == 'write':
                    data = bytes.fromhex(message['data'])
                    if verbose:
                        print('phone -> watch', data.hex(' '), flush=True)
                    watch.write(WRITE_CHANNELS.get(message.get('channel'), normsdk.WriteChannel.PRIMARY), data)

        now = time.monotonic()
        elapsed = max(1, min(50, int((now - last) * 1000)))
        last = now
        for channel, data in watch.run(elapsed):
            if verbose:
                print('watch -> phone', data.hex(' '), flush=True)
            if client:
                try:
                    note = {'type': 'notify', 'channel': CHANNELS[channel], 'data': data.hex().upper()}
                    client.sendall((json.dumps(note) + '\n').encode())
                except OSError:
                    client = None


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--port', type=int, default=7333)
    ap.add_argument('--fresh', action='store_true', help='boot a factory-fresh (unpaired) watch')
    ap.add_argument('--verbose', action='store_true')
    args = ap.parse_args()
    watch = EmulatedWatch(paired=not args.fresh)
    try:
        serve(watch, args.port, args.verbose)
    except KeyboardInterrupt:
        pass


if __name__ == '__main__':
    main()
