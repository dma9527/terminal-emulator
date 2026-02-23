import Cocoa

/// Window controller managing tabs. Each tab contains a TerminalViewController.
class TerminalWindowController: NSWindowController, NSWindowDelegate {
    private var tabGroup: [TerminalViewController] = []

    convenience init() {
        // Read window size from config
        let tempSession = term_session_new(80, 24)!
        let w = CGFloat(term_session_window_width(tempSession))
        let h = CGFloat(term_session_window_height(tempSession))
        let bgRGB = term_session_theme_bg(tempSession)
        term_session_free(tempSession)

        let bgColor = NSColor(
            red: CGFloat((bgRGB >> 16) & 0xFF) / 255.0,
            green: CGFloat((bgRGB >> 8) & 0xFF) / 255.0,
            blue: CGFloat(bgRGB & 0xFF) / 255.0,
            alpha: 1.0
        )

        let window = NSWindow(
            contentRect: NSRect(x: 200, y: 200, width: w, height: h),
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )
        self.init(window: window)

        window.delegate = self
        window.title = "Terminal"
        window.tabbingMode = .preferred
        window.minSize = NSSize(width: 400, height: 300)
        window.isReleasedWhenClosed = false

        // Dark appearance
        window.appearance = NSAppearance(named: .darkAqua)
        window.backgroundColor = bgColor
        window.titlebarAppearsTransparent = true

        let termVC = TerminalViewController()
        window.contentViewController = termVC
        tabGroup.append(termVC)
    }

    @objc func addNewTab() {
        guard let currentWindow = self.window else { return }

        let newWindow = NSWindow(
            contentRect: currentWindow.frame,
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )
        newWindow.appearance = NSAppearance(named: .darkAqua)
        newWindow.backgroundColor = NSColor.black
        newWindow.titlebarAppearsTransparent = true

        let termVC = TerminalViewController()
        newWindow.contentViewController = termVC
        tabGroup.append(termVC)

        currentWindow.addTabbedWindow(newWindow, ordered: .above)
        newWindow.makeKeyAndOrderFront(nil)
    }

    func windowWillClose(_ notification: Notification) {
        // Cleanup terminal sessions
        for vc in tabGroup {
            vc.cleanup()
        }
    }
}
