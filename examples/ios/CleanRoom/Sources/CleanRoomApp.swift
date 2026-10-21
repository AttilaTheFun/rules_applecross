import SwiftUI

@main
struct CleanRoomApp: App {
    var body: some Scene {
        WindowGroup {
            ContentView()
        }
    }
}

// @State is a SwiftUI macro: on Linux the clean-room SwiftUIMacros plugin
// expands it, on a Mac Xcode's. Both forms of its expansion appear here.
struct ContentView: View {
    @State private var taps = 0
    @State var label = "Built by clean-room Apple tools"

    var body: some View {
        Button("\(label) (\(taps))") { taps += 1 }
            .padding()
    }
}
