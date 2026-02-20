import Cocoa

/// NSView that renders the terminal grid using Core Graphics.
/// Phase 2 uses CG for correctness; Phase 3 can switch to Metal for perf.
class TerminalMetalView: NSView {
    var session: OpaquePointer?
    var cols: Int = 80
    var rows: Int = 24

    private let cellWidth: CGFloat = 8.4
    private let cellHeight: CGFloat = 16.0
    private let fontSize: CGFloat = 14.0
    private lazy var font: NSFont = NSFont(name: "Menlo", size: fontSize) ?? NSFont.monospacedSystemFont(ofSize: fontSize, weight: .regular)

    override var isFlipped: Bool { true }  // Top-left origin like terminal

    override func draw(_ dirtyRect: NSRect) {
        guard let ctx = NSGraphicsContext.current?.cgContext,
              let session = session else { return }

        // Background
        ctx.setFillColor(NSColor.black.cgColor)
        ctx.fill(bounds)

        var gridCols: UInt32 = 0
        var gridRows: UInt32 = 0
        term_session_grid_size(session, &gridCols, &gridRows)

        let attrFont = CTFontCreateWithName("Menlo" as CFString, fontSize, nil)

        for row in 0..<Int(gridRows) {
            for col in 0..<Int(gridCols) {
                let codepoint = term_session_cell_char(session, UInt32(row), UInt32(col))
                guard codepoint > 0x20 && codepoint != 0 else { continue }  // Skip spaces and null

                let bgRGB = term_session_cell_bg(session, UInt32(row), UInt32(col))
                let fgRGB = term_session_cell_fg(session, UInt32(row), UInt32(col))

                let x = CGFloat(col) * cellWidth
                let y = CGFloat(row) * cellHeight

                // Draw background if not default black
                if bgRGB != 0 {
                    let bgColor = colorFromRGB(bgRGB)
                    ctx.setFillColor(bgColor)
                    ctx.fill(CGRect(x: x, y: y, width: cellWidth, height: cellHeight))
                }

                // Draw character
                guard let scalar = Unicode.Scalar(codepoint) else { continue }
                let ch = Character(scalar)
                let str = String(ch) as CFString

                let fgColor = colorFromRGB(fgRGB)
                let attrs: [NSAttributedString.Key: Any] = [
                    .font: font,
                    .foregroundColor: NSColor(cgColor: fgColor) ?? NSColor.white,
                ]
                let attrStr = NSAttributedString(string: String(ch), attributes: attrs)
                let line = CTLineCreateWithAttributedString(attrStr)

                ctx.textPosition = CGPoint(x: x, y: y + fontSize)
                CTLineDraw(line, ctx)
            }
        }

        // Draw cursor
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
