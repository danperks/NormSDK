import XCTest
import NormSDK

final class NormSDKTests: XCTestCase {
    func testEncodesRequests() {
        let frames = encodeRequest(request: .getBattery(newMode: true))
        XCTAssertEqual(frames.map(\.data), [Data([0x6F, 0x08, 0x70, 0x01, 0x00, 0x01, 0x8F])])
        XCTAssertEqual(frames.first?.channel, .primary)
    }

    func testCatalogIsAnnotated() {
        let catalog = commandCatalog()
        XCTAssertEqual(catalog.count, 93)
        let battery = commandSpec(kind: .getBattery)
        XCTAssertEqual(battery.id, "battery.get")
        XCTAssertEqual(battery.command, 0x08)
        XCTAssertEqual(battery.firmware, .stock)
        XCTAssertEqual(commandSpec(kind: .setLanguage).firmware, .noStockHandler)
    }

    func testSessionMatchesFragmentedReplies() throws {
        let session = Session()
        session.setLinkReady(ready: true)
        let id = session.enqueue(request: .getDeviceVersion, priority: .normal)
        let writes = session.pollTransmit(nowMs: 0)
        XCTAssertEqual(writes.count, 1)
        XCTAssertTrue(writes[0].awaitsResponse)

        let reply = encodeFrame6f(command: 0x03, action: .checkResponse,
                                  payload: Data([0x01]) + Data("A0.2R0.4T0.0H0.0B0.1".utf8))
        var events: [SessionEvent] = []
        for chunk in chunkFrame(frame: reply, maxLen: 20) {
            events += session.handleNotification(channel: .primary, data: chunk, nowMs: 5)
        }
        guard case let .completed(requestId, response) = try XCTUnwrap(events.last) else {
            return XCTFail("expected completion, got \(events)")
        }
        XCTAssertEqual(requestId, id)
        guard case let .version(text, parsed) = response else { return XCTFail("\(response)") }
        XCTAssertEqual(text, "A0.2R0.4T0.0H0.0B0.1")
        XCTAssertEqual(parsed?.apolloTag, "B0.1")
        XCTAssertEqual(parsed?.isPatched, false)
    }

    func testAcksAndTimeouts() {
        let session = Session()
        session.setLinkReady(ready: true)
        let set = session.enqueue(request: .setBrightness(level: 3), priority: .normal)
        let get = session.enqueue(request: .getCapabilities, priority: .normal)
        _ = session.pollTransmit(nowMs: 0)
        let ack = encodeFrame6f(command: 0x01, action: .setResponse, payload: Data([0x07, 0x00]))
        let events = session.handleNotification(channel: .primary, data: ack, nowMs: 1)
        XCTAssertEqual(events.last, .completed(requestId: set, response: .ack(command: 0x07, status: 0)))

        _ = session.pollTransmit(nowMs: 2)
        XCTAssertEqual(session.nextDeadlineMs(), 2_002)
        let timeout = session.tick(nowMs: 2_002)
        XCTAssertEqual(timeout, [.timedOut(requestId: get, partial: .capabilities(value: capabilities(raw: 0)))])
    }

    func testWatchInitiatedEvents() {
        let decoder = FrameDecoder()
        let frame = encodeFrame6f(command: 0xD2, action: .set, payload: Data([0x00]))
        let packet = decoder.feed(channel: .primary, data: frame)[0]
        XCTAssertEqual(classifyPacket(packet: packet), .findPhone(ringing: true))
        XCTAssertEqual(encodeRequest(request: .findPhoneStartAck)[0].channel, .remote)
    }

    func testGattProfile() {
        XCTAssertEqual(gattProfile().commandWrite, "00008001-0000-1000-8000-00805F9B34FB")
        XCTAssertTrue(isNormDeviceName(name: "Norm2#00042"))
    }
}
