import Cocoa

let app = NSApplication.shared
app.setActivationPolicy(.regular)

// Suppress harmless IMK mach port warnings
setenv("OS_ACTIVITY_DT_MODE", "", 1)

let delegate = AppDelegate()
app.delegate = delegate
app.run()
