import Cocoa

/// A single terminal pane: owns a session, view, and PTY source.
class TerminalPane {
    let id: Int
    let view: TerminalMetalView
    var session: OpaquePointer?
    var ptySource: DispatchSourceRead?

    init(id: Int, frame: NSRect) {
        self.id = id
        self.view = TerminalMetalView(frame: frame)
        self.session = term_session_new(80, 24)

        guard let session = session else { return }

        // Apply config
        let size = CGFloat(term_session_font_size(session))
        if let familyPtr = term_session_font_family(session) {
            let family = String(cString: familyPtr)
            term_string_free(familyPtr)
            view.setupFont(family: family, size: size)
        }
        let bgRGB = term_session_theme_bg(session)
        view.themeBgColor = CGColor(
            red: CGFloat((bgRGB >> 16) & 0xFF) / 255.0,
            green: CGFloat((bgRGB >> 8) & 0xFF) / 255.0,
            blue: CGFloat(bgRGB & 0xFF) / 255.0,
            alpha: 1.0
        )

        let result = term_session_spawn_shell(session, nil)
        guard result == 0 else { return }

        view.session = session

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

    private func readPTY() {
        guard let session = session else { return }
        let bytesRead = term_session_read_pty(session)
        if bytesRead > 0 {
            view.scrollToBottom()
            view.setNeedsDisplay(view.bounds)
        } else if bytesRead < 0 {
            ptySource?.cancel()
            ptySource = nil
        }
    }

    func resize(cols: UInt32, rows: UInt32, width: UInt32, height: UInt32) {
        guard let session = session else { return }
        term_session_resize(session, cols, rows, width, height)
        view.cols = Int(cols)
        view.rows = Int(rows)
    }

    func cleanup() {
        view.stopDisplayLink()
        ptySource?.cancel()
        ptySource = nil
        if let session = session {
            term_session_free(session)
        }
        session = nil
    }

    deinit { cleanup() }
}
