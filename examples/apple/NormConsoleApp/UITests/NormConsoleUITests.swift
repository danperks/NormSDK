import XCTest

/// Drives the app against `tools/emulator/bridge.py` on the host (the
/// simulator shares the host's loopback). Skipped when no bridge is running.
final class NormConsoleUITests: XCTestCase {
    func testSendsCommandsToTheEmulatedWatch() throws {
        let app = XCUIApplication()
        app.launchArguments = ["-bridge", "127.0.0.1:7333"]
        app.launch()

        let status = app.staticTexts["status"]
        let connected = NSPredicate(format: "label CONTAINS 'Connected'")
        let result = XCTWaiter.wait(for: [expectation(for: connected, evaluatedWith: status)], timeout: 10)
        if result != .completed {
            throw XCTSkip("no emulator bridge reachable: \(status.label)")
        }

        app.buttons["Commands"].tap()
        app.buttons["send.battery.get"].tap()
        app.buttons["send.identity.version"].firstMatch.tap()
        app.buttons["Log"].tap()

        let battery = app.staticTexts.containing(NSPredicate(format: "label CONTAINS 'Battery { status'")).firstMatch
        XCTAssertTrue(battery.waitForExistence(timeout: 10))
        let version = app.staticTexts.containing(NSPredicate(format: "label CONTAINS 'A0.2R0.4T0.0H0.0B0.1'")).firstMatch
        XCTAssertTrue(version.waitForExistence(timeout: 10))

        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
