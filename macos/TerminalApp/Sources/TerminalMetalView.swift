import Cocoa

/// NSView that renders the terminal grid and handles keyboard input.
class TerminalMetalView: NSView {
    var session: OpaquePointer?
    var cols: Int = 80
    var rows: Int = 24

    private let cellWidth: CGFloat = 8.4
    private let cellHeight: CGFloat = 16.0
    private let fontSize: CGFloat = 14.0
    private lazy var font: CTFont = CTFontCreateWithName("Menlo" as CFString, fontSize, nil)

    override var isFlipped: Bool { true }
    override var acceptsFirstResponder: Bool { true }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        window?.makeFirstResponder(self)
    }

    override func keyDown(with event: NSEvent) {
        guard let session = session else { return }

        var bytes: [UInt8]?

        if event.modifierFlags.contains(.control), let chars = event.charactersIgnoringModifiers {
            if let scalar = chars.unicodeScalars.first,
               scalar.value >= 0x61 && scalar.value <= 0x7a {
                bytes = [UInt8(scalar.value - 0x60)]  // Ctrl+a = 0x01, etc.
            } else if chars == "c" || chars == "C" {
                bytes = [0x03]  // Ctrl+C
            }
        } else {
            switch event.keyCode {
            case 36:  bytes = [0x0d]                    // Return
            case 51:  bytes = [0x7f]                    // Backspace
            case 48:  bytes = [0x09]                    // Tab
            case 53:  bytes = [0x1b]                    // Escape
            case 126: bytes = Array("\u{1b}[A".utf8)    // Up
            case 125: bytes = Array("\u{1b}[B".utf8)    // Down
            case 124: bytes = Array("\u{1b}[C".utf8)    // Right
            case 123: bytes = Array("\u{1b}[D".utf8)    // Left
            case 115: bytes = Array("\u{1b}[H".utf8)    // Home
            case 119: bytes = Array("\u{1b}[F".utf8)    // End
            case 116: bytes = Array("\u{1b}[5~".utf8)   // PageUp
            case 121: bytes = Array("\u{1b}[6~".utf8)   // PageDown
            case 117: bytes = Array("\u{1b}[3~".utf8)   // Delete
            default:
                if let chars = event.characters {
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

    // Prevent system beep on key press
    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        if event.modifierFlags.contains(.command) {
            return super.performKeyEquivalent(with: event)
        }
        return false
    }

    override func draw(_ dirtyRect: NSRect) {
        guard let ctx = NSGraphicsContext.current?.cgContext,
              let session = session else { return }

        ctx.setFillColor(NSColor.black.cgColor)
        ctx.fill(bounds)

        var gridCols: UInt32 = 0
        var gridRows: UInt32 = 0
        term_session_grid_size(session, &gridCols, &gridRows)

        for row in 0..<Int(gridRows) {
            for col in 0..<Int(gridCols) {
                let codepoint = term_session_cell_char(session, UInt32(row), UInt32(col))
                let bgRGB = term_session_cell_bg(session, UInt32(row), UInt32(col))

                let x = CGFloat(col) * cellWidth
                let y = CGFloat(row) * cellHeight

                if bgRGB != 0 {
                    ctx.setFillColor(colorFromRGB(bgRGB))
                    ctx.fill(CGRect(x: x, y: y, width: cellWidth, height: cellHeight))
                }

                guard codepoint > 0x20 && codepoint != 0 else { continue }
                guard let scalar = Unicode.Scalar(codepoint) else { continue }

                let fgRGB = term_session_cell_fg(session, UInt32(row), UInt32(col))
                let fgColor = colorFromRGB(fgRGB)

                let attrs: [NSAttributedString.Key: Any] = [
                    .font: font as Any,
                    .foregroundColor: NSColor(cgColor: fgColor) ?? NSColor.white,
                ]
                let attrStr = NSAttributedString(string: String(Character(scalar)), attributes: attrs)
                let line = CTLineCreateWithAttributedString(attrStr)

                ctx.saveGState()
                let baselineY = y + cellHeight - 3.0
                ctx.translateBy(x: x, y: baselineY)
                ctx.scaleBy(x: 1.0, y: -1.0)
                ctx.textPosition = .zero
                CTLineDraw(line, ctx)
                ctx.restoreGState()
            }
        }

        // Cursor
        var cursorRow: UInt32 = 0
        var cursorCol: UInt32 = 0
        term_session_cursor_pos(session, &cursorRow, &cursorCol)

        let cursorX = CGFloat(cursorCol) * cellWidth
        let cursorY = CGFloat(cursorRow) * cellHeight
        ctx.setFillColor(CGColor(red: 0.8, green: 0.8, blue: 0.8, alpha: 0.7))
        ctx.fill(CGRect(x: cursorX, y: cursorY, width: cellWidth, height: cellHeight))
    }

    private func colorFromRGB(_ rgb: UInt32) -> CGColor {
        let r = CGFloat((rgb >> 16) & 0xFF) / 255.0
        let g = CGFloat((rgb >> 8) & 0xFF) / 255.0
        let b = CGFloat(rgb & 0xFF) / 255.0
        return CGColor(red: r, green: g, blue: b, alpha: 1.0)
    }

    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
}
