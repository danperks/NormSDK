# NormSDK

One implementation of the Norm Two (P03B) smartwatch's Bluetooth protocol, shared by iOS, Android and Python apps.

The protocol lives in Rust and is exposed through [UniFFI](https://mozilla.github.io/uniffi-rs/). Every command the apps send is a typed `Request`, and each one has an annotation: the frame layout, the expected reply, where the knowledge came from, and whether the stock firmware handles it. Every command was run against the stock firmware under emulation, and the replies were recorded.

The library never touches Bluetooth or the clock. The host app owns the radio. It passes notifications in, writes whatever the session hands back, and supplies the time. That lets the same code run behind CoreBluetooth, Android BLE, bleak, or an emulator.

> Unofficial. Not affiliated with Norm, Appscomm or any watch vendor. No firmware or vendor code is included.

## What's in the box

| Path | What it is |
|---|---|
| [`crates/norm-sdk`](crates/norm-sdk) | The Rust core: framing, reassembly, typed requests and responses, the annotated catalog, and the session. |
| [`docs/COMMANDS.md`](docs/COMMANDS.md) | The command reference, generated from the catalog and the emulator recordings. |
| [`Package.swift`](Package.swift), [`Sources/NormSDK`](Sources/NormSDK), [`dist/apple`](dist/apple) | The Swift package: generated bindings plus a static XCFramework for iOS, the iOS Simulator and macOS. |
| [`android/normsdk`](android/normsdk) | An Android library module: generated Kotlin plus `.so` files for arm64-v8a, armeabi-v7a and x86_64. |
| [`python/normsdk`](python/normsdk) | The Python package: generated bindings plus the macOS host library. |
| [`scripts/build.sh`](scripts/build.sh) | The builder that regenerates everything above. |
| [`examples/`](examples) and [`android/app`](android/app) | Small apps that print what the watch sends and let you send it commands. |
| [`tools/emulator`](tools/emulator), [`tests/emulator`](tests/emulator) | A link to the emulated watch, a TCP bridge, and the firmware test suite. |

## Using it

**Swift (iOS 15+, macOS 12+).** Add the package `https://github.com/danperks/NormSDK` and import `NormSDK`.

**Android (minSdk 24).** Include `android/normsdk` as a module, or copy its `src/main` into your own. It depends on JNA (`net.java.dev.jna:jna:5.17.0@aar`).

**Python 3.9+.** Run `pip install ./python`, then `import normsdk`. The committed library is for macOS; on other platforms, run `scripts/build.sh python` first.

The session drives everything:

```swift
let session = Session()
session.setLinkReady(ready: true)                     // once notifications are subscribed
let id = session.enqueue(request: .getBattery(newMode: true), priority: .normal)

for write in session.pollTransmit(nowMs: now) {       // call on a ~20 ms timer
    peripheral.writeValue(write.data, for: characteristic(write.channel), type: .withoutResponse)
}
// in didUpdateValueFor:
for event in session.handleNotification(channel: .primary, data: value, nowMs: now) {
    if case let .completed(requestId, .battery(status)) = event, requestId == id { print(status.level) }
}
_ = session.tick(nowMs: now)                          // fires timeouts
```

Kotlin and Python use the same names: `Session`, `Request.GetBattery`, and `Request.GET_BATTERY`. Each `SessionEvent.Received` carries a `WatchMessage` for frames the watch starts itself, such as find phone, media keys, phone-GPS workouts and live heart rate.

A few protocol facts the annotations capture:

- Every 6F set is answered with `01 81 [cmd, status]`. Status 0 means applied, 1 means rejected or no data, and 2 means the command has no handler.
- The stock firmware ignores 6E frames, and it drops 6F frames with an empty payload.
- [`docs/COMMANDS.md`](docs/COMMANDS.md) shows the stock firmware's real answer to each command.

## Example apps

- [`examples/python-console`](examples/python-console): an interactive console. It connects with `--ble` (via bleak), `--bridge`, or `--emulator`.
- [`examples/apple/NormLink`](examples/apple/NormLink): CoreBluetooth and bridge transports, a session driver, and the `normconsole` macOS CLI (`swift run normconsole --bridge`).
- [`examples/apple/NormConsoleApp`](examples/apple/NormConsoleApp): a SwiftUI iOS app with a live log, the command catalog and raw frames. Open `NormConsole.xcodeproj`.
- [`android/app`](android/app): the same idea for Android. It connects over BLE, or to the bridge at `10.0.2.2:7333` from the Android emulator.

## Building

```sh
scripts/build.sh all       # or: apple | android | python
cargo xtask docs           # regenerate docs/COMMANDS.md
```

You need Rust (stable). Apple builds also need Xcode. Android builds need the NDK (set `ANDROID_NDK_HOME` or `ANDROID_HOME`) and `cargo-ndk`. Generated bindings and libraries are committed, so apps can depend on the repo directly.

## Testing

```sh
cargo test --workspace                                # Rust core
swift test                                            # Swift bindings
(cd android && ./gradlew :normsdk:testDebugUnitTest)  # Kotlin bindings on the JVM
python3 python/tests/test_bindings.py                 # Python bindings
```

The firmware suite runs every catalog command, plus read-after-write round trips, against the stock Norm Two firmware running in [NormCrack](https://github.com/danperks/NormCrack)'s Cortex-M4 emulator:

```sh
NORMCRACK_DIR=../NormCrack python3 tests/emulator/test_stock_firmware.py --record
python3 tools/emulator/bridge.py      # then point any example app, or the Swift/Kotlin bridge tests, at :7333
```

These tests need a NormCrack checkout, which holds the firmware image and resource dump. They skip themselves without one.

## License

MIT or Apache-2.0, at your option.
