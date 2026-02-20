import Cocoa

@main
class AppDelegate: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        // Create main menu
        let mainMenu = NSMenu()
        NSApp.mainMenu = mainMenu

        // App menu
        let appMenuItem = NSMenuItem()
        mainMenu.addItem(appMenuItem)
        let appMenu = NSMenu()
        appMenuItem.submenu = appMenu
        appMenu.addItem(withTitle: "About Terminal", action: #selector(NSApplication.orderFrontStandardAboutPanel(_:)), keyEquivalent: "")
        appMenu.addItem(NSMenuItem.separator())
        appMenu.addItem(withTitle: "Preferences…", action: #selector(showPreferences), keyEquivalent: ",")
        appMenu.addItem(NSMenuItem.separator())
        appMenu.addItem(withTitle: "Quit Terminal", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")

        // File menu
        let fileMenuItem = NSMenuItem()
        mainMenu.addItem(fileMenuItem)
        let fileMenu = NSMenu(title: "File")
        fileMenuItem.submenu = fileMenu
        fileMenu.addItem(withTitle: "New Tab", action: #selector(newTab), keyEquivalent: "t")
        fileMenu.addItem(withTitle: "Close Tab", action: #selector(closeTab), keyEquivalent: "w")

        // Shell menu
        let shellMenuItem = NSMenuItem()
        mainMenu.addItem(shellMenuItem)
        let shellMenu = NSMenu(title: "Shell")
        shellMenuItem.submenu = shellMenu
        shellMenu.addItem(withTitle: "Split Horizontally", action: #selector(splitHorizontal), keyEquivalent: "d")
        shellMenu.addItem(withTitle: "Split Vertically", action: #selector(splitVertical), keyEquivalent: "D")

        // View menu
        let viewMenuItem = NSMenuItem()
        mainMenu.addItem(viewMenuItem)
        let viewMenu = NSMenu(title: "View")
        viewMenuItem.submenu = viewMenu
        viewMenu.addItem(withTitle: "Increase Font Size", action: #selector(increaseFontSize), keyEquivalent: "+")
        viewMenu.addItem(withTitle: "Decrease Font Size", action: #selector(decreaseFontSize), keyEquivalent: "-")

        // Create initial window
        let window = TerminalWindowController()
        window.showWindow(nil)
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        return true
    }

    @objc func showPreferences() {
        // TODO: Phase 2 preferences window
    }

    @objc func newTab() {
        NSApp.keyWindow?.windowController?.performSelector(onMainThread: #selector(TerminalWindowController.addNewTab), with: nil, waitUntilDone: false)
    }

    @objc func closeTab() {}
    @objc func splitHorizontal() {}
    @objc func splitVertical() {}
    @objc func increaseFontSize() {}
    @objc func decreaseFontSize() {}
}
