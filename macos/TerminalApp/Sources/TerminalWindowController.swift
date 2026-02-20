import Cocoa

/// Window controller managing tabs. Each tab contains a TerminalViewController.
class TerminalWindowController: NSWindowController, NSWindowDelegate {
    private var tabGroup: [TerminalViewController] = []

    convenience init() {
        let window = NSWindow(
            contentRect: NSRect(x: 200, y: 200, width: 800, height: 600),
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
        window.backgroundColor = NSColor.black
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
