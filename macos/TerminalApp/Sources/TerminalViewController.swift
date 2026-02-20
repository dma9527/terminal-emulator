import Cocoa

/// View controller for a single terminal session.
/// Bridges libterm C API with AppKit rendering.
class TerminalViewController: NSViewController {
    private var session: OpaquePointer?
    private var displayLink: CVDisplayLink?
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

        // Create terminal session via FFI
        session = term_session_new(defaultCols, defaultRows)
        guard let session = session else { return }

        // Spawn shell
        let result = term_session_spawn_shell(session, nil)
        guard result == 0 else {
            NSLog("Failed to spawn shell")
            return
        }

        terminalView.session = session

        // Monitor PTY for output using GCD
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

    private func readPTY() {
        guard let session = session else { return }
        let bytesRead = term_session_read_pty(session)
        if bytesRead > 0 {
            terminalView.setNeedsDisplay(terminalView.bounds)
            updateTitle()
        } else if bytesRead < 0 {
            // PTY closed — shell exited
            ptySource?.cancel()
            ptySource = nil
        }
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
        let cellWidth: CGFloat = 8.4  // TODO: derive from font metrics
        let cellHeight: CGFloat = 16.0

        let cols = UInt32(bounds.width / cellWidth)
        let rows = UInt32(bounds.height / cellHeight)

        if cols > 0 && rows > 0 {
            term_session_resize(session, cols, rows,
                                UInt32(bounds.width), UInt32(bounds.height))
            terminalView.cols = Int(cols)
            terminalView.rows = Int(rows)
            terminalView.setNeedsDisplay(terminalView.bounds)
        }
    }

    override func keyDown(with event: NSEvent) {
        guard let session = session else { return }

        var bytes: [UInt8]?

        if let chars = event.characters {
            if event.modifierFlags.contains(.control) {
                // Ctrl+key
                if let scalar = chars.unicodeScalars.first,
                   scalar.value >= 0x61 && scalar.value <= 0x7a {
                    bytes = [UInt8(scalar.value - 0x60)]
                }
            } else {
                switch event.keyCode {
                case 36: bytes = [0x0d]  // Return
                case 51: bytes = [0x7f]  // Backspace
                case 48: bytes = [0x09]  // Tab
                case 53: bytes = [0x1b]  // Escape
                case 126: bytes = Array("\u{1b}[A".utf8)  // Up
                case 125: bytes = Array("\u{1b}[B".utf8)  // Down
                case 124: bytes = Array("\u{1b}[C".utf8)  // Right
                case 123: bytes = Array("\u{1b}[D".utf8)  // Left
                default:
                    bytes = Array(chars.utf8)
                }
            }
        }

        if let bytes = bytes, !bytes.isEmpty {
            bytes.withUnsafeBufferPointer { buf in
                _ = term_session_write_pty(session, buf.baseAddress!, UInt32(buf.count))
            }
        }
    }

    override var acceptsFirstResponder: Bool { true }

    func cleanup() {
        ptySource?.cancel()
        ptySource = nil
        if let session = session {
            term_session_free(session)
        }
        session = nil
    }

    deinit {
        cleanup()
    }
}
