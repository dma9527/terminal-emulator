import Cocoa
import QuartzCore

/// Terminal view using Metal GPU rendering via libterm's wgpu backend.
/// Falls back to CoreGraphics if GPU init fails.
class TerminalMetalView: NSView, CALayerDelegate {
    var session: OpaquePointer?
    var cols: Int = 80
    var rows: Int = 24

    private var metalLayer: CAMetalLayer?
    private var gpuReady = false
    private var displayLink: CVDisplayLink?
    var themeBgColor: CGColor = NSColor.black.cgColor

    /// Scroll offset: 0 = bottom (live), positive = scrolled back N lines
    private var scrollOffset: Int = 0

    // Font metrics — computed from actual font
    private(set) var cellWidth: CGFloat = 1
    private(set) var cellHeight: CGFloat = 1
    private var fontAscent: CGFloat = 0
    private var ctFont: CTFont!
    private var fontSize: CGFloat = 15.0
    private var fontFamily: String = "Menlo"

    override var isFlipped: Bool { true }
    override var acceptsFirstResponder: Bool { true }

    override init(frame: NSRect) {
        super.init(frame: frame)
        setupFont(family: fontFamily, size: fontSize)
    }

    required init?(coder: NSCoder) { fatalError() }

    /// Compute cell dimensions from real font metrics.
    func setupFont(family: String, size: CGFloat) {
        fontFamily = family
        fontSize = size
        ctFont = CTFontCreateWithName(family as CFString, size, nil)

        // Cell width = advance width of 'M' (em-width for monospace)
        let chars: [UniChar] = [0x4D] // 'M'
        var glyphBuf: CGGlyph = 0
        CTFontGetGlyphsForCharacters(ctFont, chars, &glyphBuf, 1)
        var advances = CGSize.zero
        CTFontGetAdvancesForGlyphs(ctFont, .horizontal, &glyphBuf, &advances, 1)
        cellWidth = ceil(advances.width)

        // Cell height = ascent + descent + leading
        let ascent = CTFontGetAscent(ctFont)
        let descent = CTFontGetDescent(ctFont)
        let leading = CTFontGetLeading(ctFont)
        fontAscent = ceil(ascent)
        cellHeight = ceil(ascent + descent + leading)

        // Ensure minimum sizes
        if cellWidth < 1 { cellWidth = ceil(size * 0.6) }
        if cellHeight < 1 { cellHeight = ceil(size * 1.2) }
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        if window != nil {
            window?.makeFirstResponder(self)
            initGPU()
            if displayLink == nil { startDisplayLink() }
        } else {
            stopDisplayLink()
        }
    }

    private func initGPU() {
        guard let session = session else { return }
        gpuReady = false
    }

    private func startDisplayLink() {
        var dl: CVDisplayLink?
        CVDisplayLinkCreateWithActiveCGDisplays(&dl)
        guard let dl = dl else { return }

        CVDisplayLinkSetOutputCallback(dl, { (_, _, _, _, _, userInfo) -> CVReturn in
            guard let userInfo = userInfo else { return kCVReturnSuccess }
            let view = Unmanaged<TerminalMetalView>.fromOpaque(userInfo).takeUnretainedValue()
            DispatchQueue.main.async { [weak view] in view?.renderFrame() }
            return kCVReturnSuccess
        }, Unmanaged.passUnretained(self).toOpaque())

        CVDisplayLinkStart(dl)
        displayLink = dl
    }

    func stopDisplayLink() {
        if let dl = displayLink {
            CVDisplayLinkStop(dl)
            displayLink = nil
        }
    }

    private func renderFrame() {
        guard let session = session else { return }
        if gpuReady {
            let scale = window?.backingScaleFactor ?? 2.0
            let w = UInt32(bounds.width * scale)
            let h = UInt32(bounds.height * scale)
            metalLayer?.drawableSize = CGSize(width: CGFloat(w), height: CGFloat(h))
            term_session_render_gpu(session, w, h)
        } else {
            setNeedsDisplay(bounds)
        }
    }

    override func setFrameSize(_ newSize: NSSize) {
        super.setFrameSize(newSize)
        guard let session = session else { return }
        if gpuReady {
            let scale = window?.backingScaleFactor ?? 2.0
            let w = UInt32(newSize.width * scale)
            let h = UInt32(newSize.height * scale)
            term_session_resize_gpu(session, w, h)
        }
    }

    // MARK: - Scrolling

    override func scrollWheel(with event: NSEvent) {
        guard let session = session else { return }
        let delta = Int(-event.scrollingDeltaY.rounded())
        if delta == 0 { return }

        let maxScroll = Int(term_session_scrollback_len(session))
        scrollOffset = max(0, min(scrollOffset + delta, maxScroll))
        setNeedsDisplay(bounds)
    }

    /// Reset scroll to bottom (called when new output arrives).
    func scrollToBottom() {
        if scrollOffset > 0 {
            scrollOffset = 0
            setNeedsDisplay(bounds)
        }
    }

    // MARK: - Selection

    private var selectionStart: (row: Int, col: Int)?
    private var selectionEnd: (row: Int, col: Int)?
    private var isSelecting = false

    private func gridPosition(for event: NSEvent) -> (row: Int, col: Int) {
        let loc = convert(event.locationInWindow, from: nil)
        let col = Int(loc.x / cellWidth)
        let row = Int(loc.y / cellHeight)
        return (row: max(0, row), col: max(0, col))
    }

    override func mouseDown(with event: NSEvent) {
        let pos = gridPosition(for: event)
        selectionStart = pos
        selectionEnd = pos
        isSelecting = true
        setNeedsDisplay(bounds)
    }

    override func mouseDragged(with event: NSEvent) {
        guard isSelecting else { return }
        selectionEnd = gridPosition(for: event)
        setNeedsDisplay(bounds)
    }

    override func mouseUp(with event: NSEvent) {
        isSelecting = false
    }

    func copySelection() {
        guard let session = session,
              let start = selectionStart,
              let end = selectionEnd else { return }

        // Normalize: start before end
        let (sr, sc, er, ec): (Int, Int, Int, Int)
        if start.row < end.row || (start.row == end.row && start.col <= end.col) {
            (sr, sc, er, ec) = (start.row, start.col, end.row, end.col)
        } else {
            (sr, sc, er, ec) = (end.row, end.col, start.row, start.col)
        }

        let ptr = term_session_extract_text(session,
            UInt32(sr), UInt32(sc), UInt32(er), UInt32(ec))
        if let ptr = ptr {
            let text = String(cString: ptr)
            term_string_free(ptr)
            if !text.isEmpty {
                NSPasteboard.general.clearContents()
                NSPasteboard.general.setString(text, forType: .string)
            }
        }
        // Clear selection
        selectionStart = nil
        selectionEnd = nil
        setNeedsDisplay(bounds)
    }

    private func drawSelection(_ ctx: CGContext) {
        guard let start = selectionStart, let end = selectionEnd else { return }
        let (sr, sc, er, ec): (Int, Int, Int, Int)
        if start.row < end.row || (start.row == end.row && start.col <= end.col) {
            (sr, sc, er, ec) = (start.row, start.col, end.row, end.col)
        } else {
            (sr, sc, er, ec) = (end.row, end.col, start.row, start.col)
        }

        ctx.setFillColor(CGColor(red: 0.3, green: 0.5, blue: 0.8, alpha: 0.35))
        for row in sr...er {
            let colStart = (row == sr) ? sc : 0
            let colEnd = (row == er) ? ec : cols
            let x = CGFloat(colStart) * cellWidth
            let y = CGFloat(row) * cellHeight
            let w = CGFloat(colEnd - colStart) * cellWidth
            ctx.fill(CGRect(x: x, y: y, width: w, height: cellHeight))
        }
    }

    // MARK: - Keyboard

    /// Pending marked text from IME (e.g. pinyin composing)
    private var markedTextStr: String = ""
    private var imeMarkedRange: NSRange = NSRange(location: NSNotFound, length: 0)

    override func keyDown(with event: NSEvent) {
        guard let session = session else { return }

        // Ctrl+key — handle directly, don't send to IME
        if event.modifierFlags.contains(.control), let chars = event.charactersIgnoringModifiers {
            if let scalar = chars.unicodeScalars.first,
               scalar.value >= 0x61 && scalar.value <= 0x7a {
                writePTY([UInt8(scalar.value - 0x60)])
                return
            } else if chars == "c" || chars == "C" {
                writePTY([0x03])
                return
            }
        }

        // If IME is composing, let it handle everything (including Return)
        if hasMarkedText() {
            interpretKeyEvents([event])
            return
        }

        // Opt+Up/Down — prompt navigation
        if event.modifierFlags.contains(.option) {
            if let session = self.session {
                switch event.keyCode {
                case 126: // Opt+Up — previous prompt
                    let viewRow = UInt32(max(0, Int(rows) - 1 - scrollOffset))
                    let target = term_session_prev_prompt(session, viewRow)
                    if target >= 0 {
                        let gridRows = Int(rows)
                        scrollOffset = max(0, gridRows - 1 - Int(target))
                        let sbLen = Int(term_session_scrollback_len(session))
                        scrollOffset = min(scrollOffset, sbLen)
                        setNeedsDisplay(bounds)
                    }
                    return
                case 125: // Opt+Down — next prompt or back to bottom
                    let viewRow = UInt32(max(0, Int(rows) - 1 - scrollOffset))
                    let target = term_session_next_prompt(session, viewRow)
                    if target >= 0 && scrollOffset > 0 {
                        let gridRows = Int(rows)
                        scrollOffset = max(0, gridRows - 1 - Int(target))
                        setNeedsDisplay(bounds)
                    } else {
                        scrollToBottom()
                    }
                    return
                default: break
                }
            }
        }

        // Special keys — handle directly (only when IME is NOT composing)
        let appMode = term_session_cursor_keys_app(session) != 0
        switch event.keyCode {
        case 36:  writePTY([0x0d]); return
        case 51:  writePTY([0x7f]); return
        case 48:  writePTY([0x09]); return
        case 53:  writePTY([0x1b]); return
        case 126: writePTY(Array((appMode ? "\u{1b}OA" : "\u{1b}[A").utf8)); return
        case 125: writePTY(Array((appMode ? "\u{1b}OB" : "\u{1b}[B").utf8)); return
        case 124: writePTY(Array((appMode ? "\u{1b}OC" : "\u{1b}[C").utf8)); return
        case 123: writePTY(Array((appMode ? "\u{1b}OD" : "\u{1b}[D").utf8)); return
        case 115: writePTY(Array((appMode ? "\u{1b}OH" : "\u{1b}[H").utf8)); return
        case 119: writePTY(Array((appMode ? "\u{1b}OF" : "\u{1b}[F").utf8)); return
        case 116: writePTY(Array("\u{1b}[5~".utf8)); return
        case 121: writePTY(Array("\u{1b}[6~".utf8)); return
        case 117: writePTY(Array("\u{1b}[3~".utf8)); return
        default: break
        }

        // All other keys → go through IME via interpretKeyEvents
        interpretKeyEvents([event])
    }

    // Suppress unhandled selectors from interpretKeyEvents (prevents beep + garbage)
    override func doCommand(by selector: Selector) {
        // Intentionally empty
    }

    private func writePTY(_ bytes: [UInt8]) {
        guard let session = session, !bytes.isEmpty else { return }
        bytes.withUnsafeBufferPointer { buf in
            _ = term_session_write_pty(session, buf.baseAddress!, UInt32(buf.count))
        }
    }

    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        guard let session = session else { return super.performKeyEquivalent(with: event) }

        if event.modifierFlags.contains(.command) {
            // Cmd+Shift+Up/Down — reserved by macOS, don't use
            // Cmd+Home/End — scroll to top/bottom
            switch event.keyCode {
            case 115: // Home
                if let session = self.session {
                    scrollOffset = Int(term_session_scrollback_len(session))
                    setNeedsDisplay(bounds)
                }
                return true
            case 119: // End
                scrollToBottom()
                return true
            default: break
            }

            switch event.charactersIgnoringModifiers {
            case "c": // Cmd+C — copy selection
                copySelection()
                return true
            case "v": // Cmd+V — paste
                if let text = NSPasteboard.general.string(forType: .string) {
                    let bracketedPaste = term_session_bracketed_paste(session) != 0
                    var bytes: [UInt8]
                    if bracketedPaste {
                        bytes = Array("\u{1b}[200~".utf8) + Array(text.utf8) + Array("\u{1b}[201~".utf8)
                    } else {
                        bytes = Array(text.utf8)
                    }
                    bytes.withUnsafeBufferPointer { buf in
                        _ = term_session_write_pty(session, buf.baseAddress!, UInt32(buf.count))
                    }
                }
                return true
            case "=" where event.modifierFlags.contains(.command): // Cmd+= zoom in
                fontSize = min(fontSize + 1, 32)
                setupFont(family: fontFamily, size: fontSize)
                (self.window?.contentViewController as? TerminalViewController)?.fontChanged()
                return true
            case "-" where event.modifierFlags.contains(.command): // Cmd+- zoom out
                fontSize = max(fontSize - 1, 8)
                setupFont(family: fontFamily, size: fontSize)
                (self.window?.contentViewController as? TerminalViewController)?.fontChanged()
                return true
            default:
                break
            }
        }
        return super.performKeyEquivalent(with: event)
    }

    // MARK: - CoreGraphics Fallback

    override func draw(_ dirtyRect: NSRect) {
        guard !gpuReady else { return }
        guard let ctx = NSGraphicsContext.current?.cgContext,
              let session = session else { return }

        ctx.setFillColor(themeBgColor)
        ctx.fill(bounds)

        var gridCols: UInt32 = 0
        var gridRows: UInt32 = 0
        term_session_grid_size(session, &gridCols, &gridRows)

        let sbLen = Int(term_session_scrollback_len(session))
        let visibleRows = Int(gridRows)

        for screenRow in 0..<visibleRows {
            let logicalRow = screenRow - scrollOffset

            for col in 0..<Int(gridCols) {
                let codepoint: UInt32
                let fgRGB: UInt32
                let bgRGB: UInt32

                if logicalRow < 0 {
                    let sbRow = sbLen + logicalRow
                    guard sbRow >= 0 && sbRow < sbLen else { continue }
                    codepoint = term_session_scrollback_cell_char(session, UInt32(sbRow), UInt32(col))
                    fgRGB = term_session_scrollback_cell_fg(session, UInt32(sbRow), UInt32(col))
                    bgRGB = term_session_scrollback_cell_bg(session, UInt32(sbRow), UInt32(col))
                } else if logicalRow < visibleRows {
                    codepoint = term_session_cell_char(session, UInt32(logicalRow), UInt32(col))
                    fgRGB = term_session_cell_fg(session, UInt32(logicalRow), UInt32(col))
                    bgRGB = term_session_cell_bg(session, UInt32(logicalRow), UInt32(col))
                } else { continue }

                let x = CGFloat(col) * cellWidth
                let y = CGFloat(screenRow) * cellHeight

                if bgRGB != 0 {
                    ctx.setFillColor(colorFromRGB(bgRGB))
                    ctx.fill(CGRect(x: x, y: y, width: cellWidth, height: cellHeight))
                }

                guard codepoint > 0x20 && codepoint != 0 else { continue }
                guard let scalar = Unicode.Scalar(codepoint) else { continue }

                let fgColor = colorFromRGB(fgRGB)
                let attrs: [NSAttributedString.Key: Any] = [
                    .font: ctFont as Any,
                    .foregroundColor: NSColor(cgColor: fgColor) ?? NSColor.white,
                ]
                let attrStr = NSAttributedString(string: String(Character(scalar)), attributes: attrs)
                let line = CTLineCreateWithAttributedString(attrStr)

                ctx.saveGState()
                let baselineY = y + fontAscent
                ctx.translateBy(x: x, y: baselineY)
                ctx.scaleBy(x: 1.0, y: -1.0)
                ctx.textPosition = .zero
                CTLineDraw(line, ctx)
                ctx.restoreGState()
            }
        }

        // Cursor — only when not scrolled back
        if scrollOffset == 0 && term_session_cursor_visible(session) != 0 {
            var cursorRow: UInt32 = 0
            var cursorCol: UInt32 = 0
            term_session_cursor_pos(session, &cursorRow, &cursorCol)
            let cursorX = CGFloat(cursorCol) * cellWidth
            let cursorY = CGFloat(cursorRow) * cellHeight
            ctx.setFillColor(CGColor(red: 0.8, green: 0.8, blue: 0.8, alpha: 0.7))
            ctx.fill(CGRect(x: cursorX, y: cursorY, width: cellWidth, height: cellHeight))
        }

        // Selection highlight
        drawSelection(ctx)

        // Command decorations (duration + exit code)
        drawCommandDecorations(ctx)
    }

    private func drawCommandDecorations(_ ctx: CGContext) {
        guard let session = session else { return }
        let count = Int(term_session_command_count(session))
        guard count > 0 else { return }

        let viewWidth = bounds.width

        for i in 0..<count {
            let promptRow = term_session_command_prompt_row(session, UInt32(i))
            guard promptRow >= 0 else { continue }

            // Adjust for scroll offset
            let screenRow = Int(promptRow) + scrollOffset
            guard screenRow >= 0 && screenRow < rows else { continue }

            let exitCode = term_session_command_exit_code(session, UInt32(i))
            let durationMs = term_session_command_duration_ms(session, UInt32(i))

            // Build decoration string
            var parts: [String] = []
            if durationMs > 0 {
                if durationMs < 1000 {
                    parts.append("\(durationMs)ms")
                } else if durationMs < 60000 {
                    parts.append(String(format: "%.1fs", Double(durationMs) / 1000.0))
                } else {
                    let mins = durationMs / 60000
                    let secs = (durationMs % 60000) / 1000
                    parts.append("\(mins)m\(secs)s")
                }
            }

            if exitCode > 0 {
                parts.append("✘ \(exitCode)")
            } else if exitCode == 0 && durationMs > 0 {
                parts.append("✔")
            }

            guard !parts.isEmpty else { continue }
            let text = parts.joined(separator: " ")

            // Draw right-aligned
            let color: NSColor = exitCode > 0
                ? NSColor(red: 1.0, green: 0.3, blue: 0.3, alpha: 0.8)
                : NSColor(red: 0.5, green: 0.5, blue: 0.5, alpha: 0.6)

            let attrs: [NSAttributedString.Key: Any] = [
                .font: CTFontCreateWithName("Menlo" as CFString, fontSize * 0.85, nil) as Any,
                .foregroundColor: color,
            ]
            let attrStr = NSAttributedString(string: text, attributes: attrs)
            let line = CTLineCreateWithAttributedString(attrStr)
            let lineWidth = CTLineGetTypographicBounds(line, nil, nil, nil)

            let x = viewWidth - CGFloat(lineWidth) - 8
            let y = CGFloat(screenRow) * cellHeight

            ctx.saveGState()
            ctx.translateBy(x: x, y: y + fontAscent)
            ctx.scaleBy(x: 1.0, y: -1.0)
            ctx.textPosition = .zero
            CTLineDraw(line, ctx)
            ctx.restoreGState()

            // Red left border for failed commands
            if exitCode > 0 {
                ctx.setFillColor(CGColor(red: 1.0, green: 0.2, blue: 0.2, alpha: 0.6))
                ctx.fill(CGRect(x: 0, y: y, width: 3, height: cellHeight))
            }
        }
    }

    private func colorFromRGB(_ rgb: UInt32) -> CGColor {
        let r = CGFloat((rgb >> 16) & 0xFF) / 255.0
        let g = CGFloat((rgb >> 8) & 0xFF) / 255.0
        let b = CGFloat(rgb & 0xFF) / 255.0
        return CGColor(red: r, green: g, blue: b, alpha: 1.0)
    }

    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    deinit {
        if let dl = displayLink { CVDisplayLinkStop(dl) }
    }
}

// MARK: - NSTextInputClient (IME support)
extension TerminalMetalView: NSTextInputClient {
    func insertText(_ string: Any, replacementRange: NSRange) {
        let text: String
        if let s = string as? String { text = s }
        else if let s = string as? NSAttributedString { text = s.string }
        else { return }

        markedTextStr = ""
        imeMarkedRange = NSRange(location: NSNotFound, length: 0)
        writePTY(Array(text.utf8))
    }

    func setMarkedText(_ string: Any, selectedRange: NSRange, replacementRange: NSRange) {
        if let s = string as? String { markedTextStr = s }
        else if let s = string as? NSAttributedString { markedTextStr = s.string }
        imeMarkedRange = NSRange(location: 0, length: markedTextStr.utf16.count)
        setNeedsDisplay(bounds)
    }

    func unmarkText() {
        markedTextStr = ""
        imeMarkedRange = NSRange(location: NSNotFound, length: 0)
    }

    func selectedRange() -> NSRange {
        NSRange(location: NSNotFound, length: 0)
    }

    func markedRange() -> NSRange {
        imeMarkedRange
    }

    func hasMarkedText() -> Bool {
        !markedTextStr.isEmpty
    }

    func attributedSubstring(forProposedRange range: NSRange, actualRange: NSRangePointer?) -> NSAttributedString? {
        nil
    }

    func validAttributesForMarkedText() -> [NSAttributedString.Key] {
        []
    }

    func firstRect(forCharacterRange range: NSRange, actualRange: NSRangePointer?) -> NSRect {
        guard let session = session else { return .zero }
        var cursorRow: UInt32 = 0
        var cursorCol: UInt32 = 0
        term_session_cursor_pos(session, &cursorRow, &cursorCol)
        let x = CGFloat(cursorCol) * cellWidth
        let y = CGFloat(cursorRow) * cellHeight + cellHeight
        let screenPoint = window?.convertPoint(toScreen: convert(NSPoint(x: x, y: y), to: nil)) ?? .zero
        return NSRect(x: screenPoint.x, y: screenPoint.y, width: cellWidth, height: cellHeight)
    }

    func characterIndex(for point: NSPoint) -> Int {
        0
    }
}
