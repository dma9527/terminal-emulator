import Cocoa

/// View controller for a single terminal session.
class TerminalViewController: NSViewController {
    private var session: OpaquePointer?
    private var ptySource: DispatchSourceRead?
    private var terminalView: TerminalMetalView!

    private let defaultCols: UInt32 = 80
    private let defaultRows: UInt32 = 24

    override func loadView() {
        terminalView = TerminalMetalView(frame: NSRect(x: 0, y: 0, width: 800, height: 600))
        self.view = terminalView
    }

    override func viewDidLoad() {
        super.viewDidLoad()

        session = term_session_new(defaultCols, defaultRows)
        guard let session = session else { return }

        // Apply font config from config.toml
        let size = CGFloat(term_session_font_size(session))
        if let familyPtr = term_session_font_family(session) {
            let family = String(cString: familyPtr)
            term_string_free(familyPtr)
            terminalView.setupFont(family: family, size: size)
        }

        // Apply theme colors
        let bgRGB = term_session_theme_bg(session)
        let bgColor = NSColor(
            red: CGFloat((bgRGB >> 16) & 0xFF) / 255.0,
            green: CGFloat((bgRGB >> 8) & 0xFF) / 255.0,
            blue: CGFloat(bgRGB & 0xFF) / 255.0,
            alpha: 1.0
        )
        terminalView.themeBgColor = bgColor.cgColor
        view.window?.backgroundColor = bgColor

        let result = term_session_spawn_shell(session, nil)
        guard result == 0 else {
            NSLog("Failed to spawn shell")
            return
        }

        terminalView.session = session

        let fd = term_session_pty_fd(session)
        if fd >= 0 {
            let source = DispatchSource.makeReadSource(fileDescriptor: fd, queue: .main)
            source.setEventHandler { [weak self] in
                self?.readPTY()
            }
            source.resume()
            ptySource = source
        }
    }

    override func viewDidLayout() {
        super.viewDidLayout()
        updateTerminalSize()
    }

    private var lastConfigGen: UInt64 = 0

    private func readPTY() {
        guard let session = session else { return }
        let bytesRead = term_session_read_pty(session)
        if bytesRead > 0 {
            terminalView.setNeedsDisplay(terminalView.bounds)
            updateTitle()
        } else if bytesRead < 0 {
            ptySource?.cancel()
            ptySource = nil
        }

        // Poll config hot-reload
        let gen = term_session_poll_config(session)
        if gen > 0 && gen != lastConfigGen {
            lastConfigGen = gen
            applyConfig()
        }
    }

    private func applyConfig() {
        guard let session = session else { return }
        let size = CGFloat(term_session_font_size(session))
        if let familyPtr = term_session_font_family(session) {
            let family = String(cString: familyPtr)
            term_string_free(familyPtr)
            terminalView.setupFont(family: family, size: size)
        }
        let bgRGB = term_session_theme_bg(session)
        terminalView.themeBgColor = CGColor(
            red: CGFloat((bgRGB >> 16) & 0xFF) / 255.0,
            green: CGFloat((bgRGB >> 8) & 0xFF) / 255.0,
            blue: CGFloat(bgRGB & 0xFF) / 255.0,
            alpha: 1.0
        )
        updateTerminalSize()
        terminalView.setNeedsDisplay(terminalView.bounds)
    }

    private func updateTitle() {
        guard let session = session else { return }
        let titlePtr = term_session_title(session)
        if let titlePtr = titlePtr {
            let title = String(cString: titlePtr)
            term_string_free(titlePtr)
            if !title.isEmpty {
                view.window?.title = title
            }
        }
    }

    private func updateTerminalSize() {
        guard let session = session else { return }
        let bounds = view.bounds
        let cw = terminalView.cellWidth
        let ch = terminalView.cellHeight

        guard cw > 0 && ch > 0 else { return }

        let cols = UInt32(bounds.width / cw)
        let rows = UInt32(bounds.height / ch)

        if cols > 0 && rows > 0 {
            term_session_resize(session, cols, rows,
                                UInt32(bounds.width), UInt32(bounds.height))
            terminalView.cols = Int(cols)
            terminalView.rows = Int(rows)
            terminalView.setNeedsDisplay(terminalView.bounds)
        }
    }

    func fontChanged() {
        updateTerminalSize()
    }

    func cleanup() {
        ptySource?.cancel()
        ptySource = nil
        if let session = session {
            term_session_free(session)
        }
        session = nil
    }

    deinit { cleanup() }
}
