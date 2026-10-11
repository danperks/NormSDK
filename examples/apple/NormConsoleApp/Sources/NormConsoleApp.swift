import SwiftUI

@main
struct NormConsoleApp: App {
    @StateObject private var model = ConsoleModel()

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environmentObject(model)
                .task { await model.autoConnectIfRequested() }
        }
    }
}
