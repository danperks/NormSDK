import NormLink
import NormSDK
import XCTest

/// Runs against `tools/emulator/bridge.py` on 127.0.0.1:7333 (or
/// `NORMSDK_BRIDGE=host:port`); skipped when no bridge is listening.
final class EmulatorBridgeTests: XCTestCase {
    private var link: NormLink!

    override func setUp() async throws {
        let address = ProcessInfo.processInfo.environment["NORMSDK_BRIDGE"] ?? "127.0.0.1:7333"
        let parts = address.split(separator: ":")
        let transport = BridgeTransport(host: String(parts[0]), port: UInt16(parts[1]) ?? 7333)
        link = NormLink(transport: transport)
        do {
            try await link.start()
        } catch {
            throw XCTSkip("no emulator bridge at \(address): \(error.localizedDescription)")
        }
    }

    override func tearDown() {
        link?.stop()
    }

    private func response(_ request: Request) async throws -> Response {
        let event = await link.send(request)
        guard case let .completed(_, response) = event else {
            throw XCTSkip("unexpected \(String(describing: event))")
        }
        return response
    }

    func testIdentityAndBattery() async throws {
        guard case let .version(text, parsed) = try await response(.getDeviceVersion) else { return XCTFail() }
        XCTAssertEqual(text, "A0.2R0.4T0.0H0.0B0.1")
        XCTAssertEqual(parsed?.isPatched, false)
        guard case let .watchId(id) = try await response(.getWatchId) else { return XCTFail() }
        XCTAssertEqual(id, "FCP03A18050801000000")
        guard case let .battery(status) = try await response(.getBattery(newMode: true)) else { return XCTFail() }
        XCTAssertGreaterThan(status.level, 0)
    }

    func testBrightnessRoundTrip() async throws {
        let ack = try await response(.setBrightness(level: 2))
        XCTAssertEqual(ack, .ack(command: 0x07, status: 0))
        guard case let .brightness(level, _) = try await response(.getBrightness) else { return XCTFail() }
        XCTAssertEqual(level, 2)
        _ = try await response(.setBrightness(level: 4))
    }

    func testUnhandledCommandIsReported() async throws {
        let ack = try await response(.setLanguage(language: 0))
        XCTAssertEqual(ack, .ack(command: 0x09, status: 2))
    }
}
