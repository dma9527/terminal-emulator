import Cocoa

class AppDelegate: NSObject, NSApplicationDelegate {
    var windowController: TerminalWindowController?

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
        appMenu.addItem(withTitle: "Quit Terminal", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")

        // File menu
        let fileMenuItem = NSMenuItem()
        mainMenu.addItem(fileMenuItem)
        let fileMenu = NSMenu(title: "File")
        fileMenuItem.submenu = fileMenu
        fileMenu.addItem(withTitle: "New Tab", action: #selector(TerminalWindowController.addNewTab), keyEquivalent: "t")
        fileMenu.addItem(withTitle: "Close Window", action: #selector(NSWindow.performClose(_:)), keyEquivalent: "w")

        // Create initial window
        windowController = TerminalWindowController()
        windowController?.showWindow(nil)

        // Bring to front
        NSApp.activate(ignoringOtherApps: true)
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        return true
    }
}
