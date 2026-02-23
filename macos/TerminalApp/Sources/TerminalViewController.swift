import Cocoa

/// View controller managing one or more terminal panes with split support.
class TerminalViewController: NSViewController {
    private var panes: [TerminalPane] = []
    private var activePaneIndex: Int = 0
    private var splitView: NSSplitView!
    private var nextPaneId: Int = 1
    private var lastConfigGen: UInt64 = 0

    override func loadView() {
        splitView = NSSplitView(frame: NSRect(x: 0, y: 0, width: 800, height: 600))
        splitView.isVertical = true
        splitView.dividerStyle = .thin
        self.view = splitView
    }

    override func viewDidLoad() {
        super.viewDidLoad()
        addPane()
    }

    override func viewDidLayout() {
        super.viewDidLayout()
        for pane in panes {
            updatePaneSize(pane)
        }
    }

    // MARK: - Pane Management

    private func addPane() {
        let pane = TerminalPane(id: nextPaneId, frame: splitView.bounds)
        nextPaneId += 1
        panes.append(pane)
        splitView.addSubview(pane.view)
        activePaneIndex = panes.count - 1

        // Start polling for title updates on active pane
        setupTitlePolling(pane)
    }

    func splitVertical() {
        splitView.isVertical = true
        addPane()
        splitView.adjustSubviews()
        for pane in panes { updatePaneSize(pane) }
        view.window?.makeFirstResponder(panes[activePaneIndex].view)
    }

    func splitHorizontal() {
        splitView.isVertical = false
        addPane()
        splitView.adjustSubviews()
        for pane in panes { updatePaneSize(pane) }
        view.window?.makeFirstResponder(panes[activePaneIndex].view)
    }

    func closeActivePane() {
        guard panes.count > 1 else { return }
        let pane = panes[activePaneIndex]
        pane.view.removeFromSuperview()
        pane.cleanup()
        panes.remove(at: activePaneIndex)
        activePaneIndex = min(activePaneIndex, panes.count - 1)
        splitView.adjustSubviews()
        view.window?.makeFirstResponder(panes[activePaneIndex].view)
        for pane in panes { updatePaneSize(pane) }
    }

    func focusNextPane() {
        guard panes.count > 1 else { return }
        activePaneIndex = (activePaneIndex + 1) % panes.count
        view.window?.makeFirstResponder(panes[activePaneIndex].view)
        highlightActivePane()
    }

    private func updatePaneSize(_ pane: TerminalPane) {
        let bounds = pane.view.bounds
        let cw = pane.view.cellWidth
        let ch = pane.view.cellHeight
        guard cw > 0 && ch > 0 else { return }
        let cols = UInt32(bounds.width / cw)
        let rows = UInt32(bounds.height / ch)
        if cols > 0 && rows > 0 {
            pane.resize(cols: cols, rows: rows, width: UInt32(bounds.width), height: UInt32(bounds.height))
        }
    }

    private func highlightActivePane() {
        for (i, pane) in panes.enumerated() {
            pane.view.layer?.borderWidth = panes.count > 1 ? 1 : 0
            pane.view.layer?.borderColor = (i == activePaneIndex)
                ? NSColor(white: 0.4, alpha: 1.0).cgColor
                : NSColor(white: 0.15, alpha: 1.0).cgColor
        }
    }

    // MARK: - Title

    private func setupTitlePolling(_ pane: TerminalPane) {
        // Use a timer to poll title + config
        Timer.scheduledTimer(withTimeInterval: 0.5, repeats: true) { [weak self, weak pane] _ in
            guard let self = self, let pane = pane, let session = pane.session else { return }

            // Title
            var title = ""
            let dirPtr = term_session_working_dir(session)
            if dirPtr != nil {
                let dir = String(cString: dirPtr!)
                term_string_free(dirPtr!)
                if !dir.isEmpty { title = (dir as NSString).lastPathComponent }
            }
            if title.isEmpty {
                let titlePtr = term_session_title(session)
                if titlePtr != nil {
                    let t = String(cString: titlePtr!)
                    term_string_free(titlePtr!)
                    if !t.isEmpty { title = t }
                }
            }
            let exitCode = term_session_last_exit_code(session)
            if exitCode > 0 { title += " ✘ \(exitCode)" }
            if !title.isEmpty { self.view.window?.title = title }

            // Config hot-reload
            let gen = term_session_poll_config(session)
            if gen > 0 && gen != self.lastConfigGen {
                self.lastConfigGen = gen
                self.applyConfig(pane)
            }
        }
    }

    private func applyConfig(_ pane: TerminalPane) {
        guard let session = pane.session else { return }
        let size = CGFloat(term_session_font_size(session))
        if let familyPtr = term_session_font_family(session) {
            let family = String(cString: familyPtr)
            term_string_free(familyPtr)
            pane.view.setupFont(family: family, size: size)
        }
        let bgRGB = term_session_theme_bg(session)
        pane.view.themeBgColor = CGColor(
            red: CGFloat((bgRGB >> 16) & 0xFF) / 255.0,
            green: CGFloat((bgRGB >> 8) & 0xFF) / 255.0,
            blue: CGFloat(bgRGB & 0xFF) / 255.0,
            alpha: 1.0
        )
        updatePaneSize(pane)
    }

    func fontChanged() {
        for pane in panes { updatePaneSize(pane) }
    }

    func cleanup() {
        for pane in panes { pane.cleanup() }
        panes.removeAll()
    }

    deinit { cleanup() }
}
