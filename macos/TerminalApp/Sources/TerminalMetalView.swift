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
        window?.makeFirstResponder(self)
        if let scale = window?.backingScaleFactor {
            metalLayer?.contentsScale = scale
        }
        initGPU()
        startDisplayLink()
    }

    private func initGPU() {
        guard let session = session else { return }
        // GPU rendering disabled until Rust atlas font size matches Swift metrics.
        // CG fallback provides correct font rendering for now.
        // TODO: pass font size to Rust via FFI, rebuild atlas at correct size
        gpuReady = false
    }

    private func startDisplayLink() {
        var dl: CVDisplayLink?
        CVDisplayLinkCreateWithActiveCGDisplays(&dl)
        guard let dl = dl else { return }

        CVDisplayLinkSetOutputCallback(dl, { (_, _, _, _, _, userInfo) -> CVReturn in
            let view = Unmanaged<TerminalMetalView>.fromOpaque(userInfo!).takeUnretainedValue()
            DispatchQueue.main.async { view.renderFrame() }
            return kCVReturnSuccess
        }, Unmanaged.passUnretained(self).toOpaque())

        CVDisplayLinkStart(dl)
        displayLink = dl
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

    // MARK: - Keyboard

    override func keyDown(with event: NSEvent) {
        guard let session = session else { return }

        var bytes: [UInt8]?
        let appMode = term_session_cursor_keys_app(session) != 0

        if event.modifierFlags.contains(.control), let chars = event.charactersIgnoringModifiers {
            if let scalar = chars.unicodeScalars.first,
               scalar.value >= 0x61 && scalar.value <= 0x7a {
                bytes = [UInt8(scalar.value - 0x60)]
            } else if chars == "c" || chars == "C" {
                bytes = [0x03]
            }
        } else {
            switch event.keyCode {
            case 36:  bytes = [0x0d]
            case 51:  bytes = [0x7f]
            case 48:  bytes = [0x09]
            case 53:  bytes = [0x1b]
            case 126: bytes = Array((appMode ? "\u{1b}OA" : "\u{1b}[A").utf8)
            case 125: bytes = Array((appMode ? "\u{1b}OB" : "\u{1b}[B").utf8)
            case 124: bytes = Array((appMode ? "\u{1b}OC" : "\u{1b}[C").utf8)
            case 123: bytes = Array((appMode ? "\u{1b}OD" : "\u{1b}[D").utf8)
            case 115: bytes = Array((appMode ? "\u{1b}OH" : "\u{1b}[H").utf8)
            case 119: bytes = Array((appMode ? "\u{1b}OF" : "\u{1b}[F").utf8)
            case 116: bytes = Array("\u{1b}[5~".utf8)
            case 121: bytes = Array("\u{1b}[6~".utf8)
            case 117: bytes = Array("\u{1b}[3~".utf8)
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

    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        if event.modifierFlags.contains(.command) {
            return super.performKeyEquivalent(with: event)
        }
        return false
    }

    // MARK: - CoreGraphics Fallback

    override func draw(_ dirtyRect: NSRect) {
        guard !gpuReady else { return }
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
                    .font: ctFont as Any,
                    .foregroundColor: NSColor(cgColor: fgColor) ?? NSColor.white,
                ]
                let attrStr = NSAttributedString(string: String(Character(scalar)), attributes: attrs)
                let line = CTLineCreateWithAttributedString(attrStr)

                ctx.saveGState()
                // Baseline = top of cell + ascent (in flipped coords, need to flip for CG)
                let baselineY = y + fontAscent
                ctx.translateBy(x: x, y: baselineY)
                ctx.scaleBy(x: 1.0, y: -1.0)
                ctx.textPosition = .zero
                CTLineDraw(line, ctx)
                ctx.restoreGState()
            }
        }

        // Cursor
        if term_session_cursor_visible(session) != 0 {
            var cursorRow: UInt32 = 0
            var cursorCol: UInt32 = 0
            term_session_cursor_pos(session, &cursorRow, &cursorCol)
            let cursorX = CGFloat(cursorCol) * cellWidth
            let cursorY = CGFloat(cursorRow) * cellHeight
            ctx.setFillColor(CGColor(red: 0.8, green: 0.8, blue: 0.8, alpha: 0.7))
            ctx.fill(CGRect(x: cursorX, y: cursorY, width: cellWidth, height: cellHeight))
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
