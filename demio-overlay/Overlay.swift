import AppKit
import Carbon

let buildVersion = "0.2.1"
let session = URL(fileURLWithPath: CommandLine.arguments[1])
let owner = Int32(CommandLine.arguments[2]) ?? 0
func send(_ command: String) { print(command); fflush(stdout) }

struct PresenterFrame: Decodable {
    let status: String
    let channel: String
    let scenarioID: String
    let step: String
    let title: String
    let say: String
    let wait: String
    let detail: String
    let generation: UInt64
    let primary: String?
    let canPrevious: Bool
    let reviewing: Bool?

    enum CodingKeys: String, CodingKey {
        case status, channel, step, title, say, wait, detail, generation, primary, reviewing
        case scenarioID = "scenario_id"
        case canPrevious = "can_previous"
    }

    var heading: String { "\(channel) (\(scenarioID))" }
    var stepHeading: String { "Step: \(title) (\(step.replacingOccurrences(of: "/", with: " of ")))" }
    var offRoute: Bool { status == "Route changed" || status == "Restoring route" }
    var showsTranscript: Bool { !offRoute || reviewing == true }
    var statusLabel: String {
        switch status {
        case "Introduction": "Ready to start"
        case "Route changed": "Off route"
        default: status
        }
    }
    var statusColor: NSColor {
        switch status {
        case "Ready", "Finished": .systemGreen
        case "Preparing", "Working", "Waiting", "Checking route", "Restoring route": .systemBlue
        case "Awaiting confirmation": .systemYellow
        case "Route changed", "Step failed", "Preparation failed", "Preview failed",
             "Outcome unknown", "Session expired": .systemRed
        default: .labelColor
        }
    }
}

final class HoverButton: NSButton {
    var onHover: ((Bool) -> Void)?
    private var hoverArea: NSTrackingArea?

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let hoverArea { removeTrackingArea(hoverArea) }
        let area = NSTrackingArea(rect: .zero, options: [.mouseEnteredAndExited, .activeAlways, .inVisibleRect],
                                  owner: self, userInfo: nil)
        hoverArea = area
        addTrackingArea(area)
    }

    override func mouseEntered(with event: NSEvent) { onHover?(true) }
    override func mouseExited(with event: NSEvent) { onHover?(false) }
}

final class Controller: NSObject, NSApplicationDelegate, NSWindowDelegate, NSPopoverDelegate {
    private static let defaultFontSize: CGFloat = 12
    private static let transcriptNotes = try? NSRegularExpression(pattern: #"<[^<>\r\n]+>"#)
    private let minimumFontSize: CGFloat = 9
    private let maximumFontSize: CGFloat = 48
    private let initialContentSize = NSSize(width: 360, height: 288)
    private let titleFont = NSFont.systemFont(ofSize: 15, weight: .bold)
    private let statusFont = NSFont.systemFont(ofSize: 12, weight: .semibold)
    var notes: NSPanel!
    var back: NSButton!
    var start: NSButton!
    var next: NSButton!
    var stateButton: NSButton!
    var stop: NSButton!
    let notesTop = NSBox()
    let separator = NSBox()
    var frame: PresenterFrame?
    let cue = NSTextView()
    let title = NSTextField(wrappingLabelWithString: "")
    let step = NSTextField(wrappingLabelWithString: "")
    let status = NSTextField(labelWithString: "")
    let settingsPopover = NSPopover()
    let helpPopover = NSPopover()
    var settingsButton: NSButton!
    let helpButton = HoverButton()
    var last = Data()
    var keys: [EventHotKeyRef?] = []
    private var sessionUpdates: DispatchSourceFileSystemObject?
    private var ownerExit: DispatchSourceProcess?
    var fontSize = Controller.defaultFontSize
    var textAlignment: NSTextAlignment = .left
    let fontLabel = NSTextField(labelWithString: "")
    var smaller: NSButton!
    var larger: NSButton!
    var alignmentPicker: NSSegmentedControl!
    func settings(_ arguments: [String] = []) throws -> [String: Any] {
        let cli = URL(fileURLWithPath: CommandLine.arguments[0]).resolvingSymlinksInPath().deletingLastPathComponent().appendingPathComponent("demio")
        let process = Process()
        process.executableURL = cli
        process.arguments = ["--overlay-settings"] + arguments
        let output = Pipe()
        process.standardOutput = output
        try process.run()
        let data = output.fileHandleForReading.readDataToEndOfFile()
        process.waitUntilExit()
        guard process.terminationStatus == 0 else {
            throw NSError(domain: "DemioConfig", code: Int(process.terminationStatus))
        }
        return (try JSONSerialization.jsonObject(with: data)) as? [String: Any] ?? [:]
    }
    func restoreSettings() {
        do {
            let state = try settings()
            if let size = state["font_size"] as? Double, size.isFinite {
                fontSize = min(maximumFontSize, max(minimumFontSize, size))
            }
            switch state["alignment"] as? String { case "center": textAlignment = .center; case "right": textAlignment = .right; default: textAlignment = .left }
        } catch { fputs("Could not load presenter settings from config.toml\n", stderr) }
    }
    func saveSettings() {
        let alignment = textAlignment == .center ? "center" : textAlignment == .right ? "right" : "left"
        do { _ = try settings([String(Double(fontSize)), alignment]) }
        catch { fputs("Could not save presenter settings in config.toml\n", stderr) }
    }
    func applyTextStyle() {
        fontLabel.stringValue = "\(Int(fontSize)) pt"
        smaller?.isEnabled = fontSize > minimumFontSize
        larger?.isEnabled = fontSize < maximumFontSize
        alignmentPicker?.selectedSegment = textAlignment == .center ? 1 : textAlignment == .right ? 2 : 0
        renderCue()
    }
    func renderCue() {
        guard let frame else { return }
        let content = NSMutableAttributedString(string: "")
        let headingFont = NSFont.systemFont(ofSize: 12, weight: .bold)
        let infoFont = NSFont.systemFont(ofSize: 12)
        func append(_ text: String, font: NSFont, color: NSColor = .labelColor,
                    alignment: NSTextAlignment = .left, before: CGFloat = 0, after: CGFloat = 0) {
            let paragraph = NSMutableParagraphStyle()
            paragraph.alignment = alignment
            paragraph.paragraphSpacingBefore = before
            paragraph.paragraphSpacing = after
            content.append(NSAttributedString(string: text, attributes: [
                .font: font, .foregroundColor: color, .paragraphStyle: paragraph,
            ]))
        }
        append("Info\n", font: headingFont, after: 4)
        if !frame.detail.isEmpty {
            append(frame.detail + "\n", font: infoFont)
        }
        if !frame.offRoute && !frame.wait.isEmpty {
            append("Wait: \(frame.wait)\n", font: infoFont, color: .systemYellow)
        }
        if frame.showsTranscript && !frame.say.isEmpty {
            append("Transcript\n", font: headingFont, before: 12, after: 4)
            let narrationFont = NSFont.systemFont(ofSize: fontSize)
            let start = content.length
            append(frame.say, font: narrationFont, alignment: textAlignment)
            let length = (frame.say as NSString).length
            let notes = Self.transcriptNotes?.matches(in: frame.say, range: NSRange(location: 0, length: length)) ?? []
            if !notes.isEmpty {
                let noteFont = NSFontManager.shared.convert(narrationFont, toHaveTrait: .italicFontMask)
                for note in notes {
                    content.addAttributes([.font: noteFont, .foregroundColor: NSColor.secondaryLabelColor],
                        range: NSRange(location: start + note.range.location, length: note.range.length))
                }
            }
        }
        if !cue.attributedString().isEqual(to: content) {
            cue.textStorage?.setAttributedString(content)
        }
    }
    @objc func decreaseFont() { fontSize = max(minimumFontSize, fontSize-2); applyTextStyle(); saveSettings() }
    @objc func increaseFont() { fontSize = min(maximumFontSize, fontSize+2); applyTextStyle(); saveSettings() }
    @objc func changeAlignment() {
        let alignments: [NSTextAlignment] = [.left, .center, .right]
        guard alignments.indices.contains(alignmentPicker.selectedSegment) else { return }
        textAlignment = alignments[alignmentPicker.selectedSegment]
        applyTextStyle(); saveSettings()
    }
    func buildSettings() {
        smaller = NSButton(title: "A−", target: self, action: #selector(decreaseFont))
        larger = NSButton(title: "A+", target: self, action: #selector(increaseFont))
        for button in [smaller!, larger!] {
            button.bezelStyle = .rounded
            button.controlSize = .small
        }
        smaller.setAccessibilityLabel("Decrease narration size")
        larger.setAccessibilityLabel("Increase narration size")
        fontLabel.font = .monospacedDigitSystemFont(ofSize: 11, weight: .regular)
        alignmentPicker = NSSegmentedControl(labels: ["Left", "Center", "Right"], trackingMode: .selectOne,
                                             target: self, action: #selector(changeAlignment))
        alignmentPicker.controlSize = .small
        alignmentPicker.setAccessibilityLabel("Narration alignment")
        let sizeLabel = NSTextField(labelWithString: "Font size")
        let alignmentLabel = NSTextField(labelWithString: "Alignment")
        for label in [sizeLabel, alignmentLabel] { label.font = .systemFont(ofSize: 11, weight: .medium) }
        let size = NSStackView(views: [sizeLabel, smaller, fontLabel, larger])
        let alignment = NSStackView(views: [alignmentLabel, alignmentPicker])
        let controls = NSStackView(views: [size, alignment])
        controls.orientation = .vertical
        controls.alignment = .leading
        controls.spacing = 8
        controls.translatesAutoresizingMaskIntoConstraints = false
        let controller = NSViewController()
        let contentSize = NSSize(width: 240, height: 76)
        controller.view = NSView(frame: NSRect(origin: .zero, size: contentSize))
        controller.preferredContentSize = contentSize
        controller.view.addSubview(controls)
        NSLayoutConstraint.activate([
            controls.leadingAnchor.constraint(equalTo: controller.view.leadingAnchor, constant: 12),
            controls.trailingAnchor.constraint(lessThanOrEqualTo: controller.view.trailingAnchor, constant: -12),
            controls.topAnchor.constraint(equalTo: controller.view.topAnchor, constant: 10),
            controls.bottomAnchor.constraint(equalTo: controller.view.bottomAnchor, constant: -10),
        ])
        settingsPopover.contentViewController = controller
        settingsPopover.contentSize = contentSize
        settingsPopover.behavior = .transient
        settingsPopover.animates = false
        settingsPopover.delegate = self
        applyTextStyle()
    }
    func buildHelp() {
        let legend = NSTextField(wrappingLabelWithString: """
        F7   Back to the previous step
        F9   Forward when available
        Use Fn with F7/F9 if needed.
        """)
        legend.font = .systemFont(ofSize: 11)
        legend.translatesAutoresizingMaskIntoConstraints = false
        let controller = NSViewController()
        let size = NSSize(width: 220, height: 65)
        controller.view = NSView(frame: NSRect(origin: .zero, size: size))
        controller.view.addSubview(legend)
        NSLayoutConstraint.activate([
            legend.leadingAnchor.constraint(equalTo: controller.view.leadingAnchor, constant: 12),
            legend.trailingAnchor.constraint(equalTo: controller.view.trailingAnchor, constant: -12),
            legend.topAnchor.constraint(equalTo: controller.view.topAnchor, constant: 10),
            legend.bottomAnchor.constraint(lessThanOrEqualTo: controller.view.bottomAnchor, constant: -10),
        ])
        helpPopover.contentViewController = controller
        helpPopover.contentSize = size
        helpPopover.behavior = .transient
        helpPopover.animates = false
        helpPopover.delegate = self
    }
    func toggle(_ popover: NSPopover, from button: NSButton, closing other: NSPopover) {
        if popover.isShown {
            popover.close()
        } else {
            other.close()
            popover.show(relativeTo: button.bounds, of: button, preferredEdge: .minY)
            popover.contentViewController?.view.window?.sharingType = .none
        }
    }
    @objc func toggleSettings() { toggle(settingsPopover, from: settingsButton, closing: helpPopover) }
    func hoverHelp(_ entered: Bool) {
        if !entered {
            helpPopover.close()
        } else if !helpPopover.isShown {
            toggle(helpPopover, from: helpButton, closing: settingsPopover)
        }
    }
    func popoverDidShow(_ notification: Notification) {
        (notification.object as? NSPopover)?.contentViewController?.view.window?.sharingType = .none
    }
    func button(_ label: String, symbol: String, action: Selector?, iconOnly: Bool = false,
                control: NSButton = NSButton()) -> NSButton {
        control.title = label
        control.target = self
        control.action = action
        control.bezelStyle = .rounded
        control.image = NSImage(systemSymbolName: symbol, accessibilityDescription: nil)
        control.imagePosition = iconOnly ? .imageOnly : .imageLeading
        control.setAccessibilityLabel(label)
        control.refusesFirstResponder = true
        if iconOnly {
            control.isBordered = false
            control.toolTip = label
            control.widthAnchor.constraint(equalToConstant: 25).isActive = true
            control.heightAnchor.constraint(equalToConstant: 23).isActive = true
        }
        return control
    }
    func panel(_ rect: NSRect, resizable: Bool) -> NSPanel {
        var style: NSWindow.StyleMask = [.titled, .nonactivatingPanel]
        if resizable { style.insert(.resizable) }
        let window = NSPanel(contentRect: rect, styleMask: style, backing: .buffered, defer: false)
        // Preserve the original presenter panel's exclusion from legacy capture.
        // ScreenCaptureKit recorders may ignore this flag on newer macOS versions.
        window.sharingType = .none
        window.titleVisibility = .hidden
        window.level = .floating
        window.isFloatingPanel = true
        window.hidesOnDeactivate = false
        window.isMovableByWindowBackground = true
        window.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        window.delegate = self
        let glass = NSVisualEffectView(frame: NSRect(origin: .zero, size: rect.size))
        glass.material = .hudWindow
        glass.blendingMode = .behindWindow
        glass.state = .active
        glass.autoresizingMask = [.width, .height]
        window.contentView = glass
        return window
    }
    @objc func publishForeground(_ notification: Notification? = nil) {
        let pid = NSWorkspace.shared.frontmostApplication?.processIdentifier ?? 0
        try? "\(pid)\n".write(to: session.appendingPathComponent("foreground-pid"),
                              atomically: true, encoding: .utf8)
    }
    func observeSession() {
        guard owner > 0, getppid() == owner else { NSApp.terminate(nil); return }
        let descriptor = open(session.path, O_EVTONLY | O_CLOEXEC)
        guard descriptor >= 0 else {
            fputs("Could not observe the Demio session.\n", stderr)
            NSApp.terminate(nil)
            return
        }
        // Atomic replacement of panel.json changes its parent directory.
        let updates = DispatchSource.makeFileSystemObjectSource(fileDescriptor: descriptor,
            eventMask: [.write, .delete, .rename], queue: .main)
        updates.setEventHandler { [weak self] in self?.refresh() }
        updates.setCancelHandler { close(descriptor) }
        sessionUpdates = updates
        updates.activate()
        let process = DispatchSource.makeProcessSource(identifier: owner, eventMask: .exit, queue: .main)
        process.setEventHandler { NSApp.terminate(nil) }
        ownerExit = process
        process.activate()
        refresh()
    }
    func applicationDidFinishLaunching(_ notification: Notification) {
        restoreSettings()
        NSWorkspace.shared.notificationCenter.addObserver(self, selector: #selector(publishForeground),
            name: NSWorkspace.didActivateApplicationNotification, object: nil)
        publishForeground()
        let screen = NSScreen.main?.visibleFrame ?? NSRect(x: 0, y: 0, width: 1280, height: 800)
        let origin = NSPoint(x: screen.maxX - initialContentSize.width - 25, y: screen.minY + 25)
        notes = panel(NSRect(origin: origin, size: initialContentSize), resizable: true)
        notes.contentMinSize = NSSize(width: 320, height: 242)
        let row = NSStackView()
        row.orientation = .horizontal
        row.distribution = .fillEqually
        row.detachesHiddenViews = false
        row.spacing = 8
        back = button("Back", symbol: "arrow.left", action: #selector(previous))
        start = button("Start", symbol: "play.fill", action: #selector(startScenario))
        start.setAccessibilityLabel("Start scenario")
        next = button("Forward", symbol: "arrow.right", action: #selector(forward))
        next.imagePosition = .imageTrailing
        stateButton = button("Confirm", symbol: "checkmark", action: #selector(performStateAction))
        stateButton.setAccessibilityLabel("Confirm completion")
        stop = button("Stop", symbol: "stop.fill", action: #selector(stopScenario))
        [back!, start!, next!].forEach(row.addArrangedSubview)
        let stateRow = NSStackView(views: [stateButton, stop])
        stateRow.distribution = .fillEqually
        stateRow.spacing = 8
        let body = NSStackView()
        body.orientation = .vertical
        body.alignment = .centerX
        body.spacing = 6
        body.translatesAutoresizingMaskIntoConstraints = false
        title.font = titleFont
        title.maximumNumberOfLines = 2
        title.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        title.setContentHuggingPriority(.defaultLow, for: .horizontal)
        step.font = .systemFont(ofSize: 12, weight: .medium)
        step.maximumNumberOfLines = 2
        step.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        status.font = statusFont
        status.lineBreakMode = .byTruncatingTail
        status.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        let metadata = NSStackView(views: [step, status])
        metadata.orientation = .vertical
        metadata.alignment = .leading
        metadata.spacing = 3
        settingsButton = button("Settings", symbol: "gearshape", action: #selector(toggleSettings), iconOnly: true)
        _ = button("Keyboard shortcuts", symbol: "info.circle", action: nil, iconOnly: true, control: helpButton)
        helpButton.toolTip = nil
        helpButton.onHover = { [weak self] entered in self?.hoverHelp(entered) }
        let header = NSStackView(views: [title, settingsButton, helpButton])
        header.distribution = .fill
        header.alignment = .centerY
        header.spacing = 4
        buildSettings()
        buildHelp()
        let scroll = NSScrollView()
        scroll.hasVerticalScroller = true
        scroll.drawsBackground = false
        cue.isEditable = false
        cue.isSelectable = true
        cue.drawsBackground = false
        cue.textColor = .labelColor
        cue.textContainerInset = NSSize(width: 0, height: 4)
        cue.textContainer?.lineFragmentPadding = 0
        cue.isVerticallyResizable = true
        cue.isHorizontallyResizable = false
        cue.autoresizingMask = [.width]
        cue.textContainer?.widthTracksTextView = true
        cue.textContainer?.containerSize = NSSize(width: initialContentSize.width - 30, height: CGFloat.greatestFiniteMagnitude)
        scroll.documentView = cue
        notesTop.boxType = .separator
        separator.boxType = .separator
        for view in [header, metadata, notesTop, scroll, separator, row, stateRow] as [NSView] { body.addArrangedSubview(view) }
        notes.contentView!.addSubview(body)
        NSLayoutConstraint.activate([
            body.leadingAnchor.constraint(equalTo: notes.contentView!.leadingAnchor, constant: 12),
            body.trailingAnchor.constraint(equalTo: notes.contentView!.trailingAnchor, constant: -12),
            body.topAnchor.constraint(equalTo: notes.contentView!.topAnchor, constant: 10),
            body.bottomAnchor.constraint(equalTo: notes.contentView!.bottomAnchor, constant: -10),
            header.widthAnchor.constraint(equalTo: body.widthAnchor),
            metadata.widthAnchor.constraint(equalTo: body.widthAnchor),
            step.widthAnchor.constraint(equalTo: metadata.widthAnchor),
            status.widthAnchor.constraint(equalTo: metadata.widthAnchor),
            notesTop.widthAnchor.constraint(equalTo: body.widthAnchor),
            scroll.widthAnchor.constraint(equalTo: body.widthAnchor),
            separator.widthAnchor.constraint(equalTo: body.widthAnchor),
            row.widthAnchor.constraint(equalTo: body.widthAnchor),
            stateRow.widthAnchor.constraint(equalToConstant: 240),
            scroll.heightAnchor.constraint(greaterThanOrEqualToConstant: 50),
        ])
        notes.orderFrontRegardless()
        installKeys()
        observeSession()
        if CommandLine.arguments.contains("--smoke") {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { self.smoke() }
        }
    }
    func smoke() {
        func attributes(at text: String) -> [NSAttributedString.Key: Any]? {
            let range = (cue.string as NSString).range(of: text)
            guard range.location != NSNotFound else { return nil }
            return cue.textStorage?.attributes(at: range.location, effectiveRange: nil)
        }
        func narrationSize() -> CGFloat? {
            (attributes(at: frame?.say ?? "")?[.font] as? NSFont)?.pointSize
        }
        func snapshot(_ view: NSView, as name: String) {
            guard let bitmap = view.bitmapImageRepForCachingDisplay(in: view.bounds) else { return }
            view.cacheDisplay(in: view.bounds, to: bitmap)
            if let png = bitmap.representation(using: .png, properties: [:]) {
                try? png.write(to: session.appendingPathComponent(name + ".png"))
            }
        }
        guard notes.sharingType == .none else { exit(11) }
        guard let foreground = try? String(contentsOf: session.appendingPathComponent("foreground-pid"), encoding: .utf8),
              Int(foreground.trimmingCharacters(in: .whitespacesAndNewlines)) != nil else { exit(18) }
        let buttons = [back!, start!, next!, stateButton!, stop!, settingsButton!, helpButton]
        guard NSApp.windows.filter({ $0.isVisible }).count == 1,
              buttons.allSatisfy({ $0.window === notes && $0.image != nil }), cue.window === notes,
              notes.contentView?.bounds.size == initialContentSize, narrationSize() == fontSize,
              notes.styleMask.contains(.resizable), notes.titleVisibility == .hidden,
              let infoRange = cue.string.range(of: "Info\n"),
              let transcriptRange = cue.string.range(of: "Transcript\n"),
              infoRange.lowerBound < transcriptRange.lowerBound,
              let initialFrame = frame,
              title.stringValue == "Native check (native1)",
              step.stringValue == initialFrame.stepHeading, status.stringValue == "Status: \(initialFrame.statusLabel)",
              title.attributedStringValue.attribute(.foregroundColor, at: 0, effectiveRange: nil) as? NSColor == .labelColor,
              title.attributedStringValue.attribute(.foregroundColor, at: title.attributedStringValue.length - 1, effectiveRange: nil) as? NSColor == .secondaryLabelColor,
              attributes(at: "Info")?[.font] as? NSFont == .systemFont(ofSize: 12, weight: .bold),
              let transcriptStyle = attributes(at: "Transcript")?[.paragraphStyle] as? NSParagraphStyle,
              transcriptStyle.paragraphSpacingBefore >= 10,
              attributes(at: initialFrame.say)?[.font] as? NSFont == .systemFont(ofSize: fontSize),
              attributes(at: initialFrame.detail)?[.font] as? NSFont == .systemFont(ofSize: 12) else { exit(3) }
        let original = notes.frame
        notes.setFrame(NSRect(x: original.minX+30, y: original.minY+20, width: 600, height: 360), display: true)
        notes.contentView?.layoutSubtreeIfNeeded()
        guard notes.frame.width == 600, notes.frame.height == 360 else { exit(4) }
        notes.setContentSize(notes.contentMinSize)
        notes.contentView?.layoutSubtreeIfNeeded()
        guard let content = notes.contentView else { exit(12) }
        for control in buttons {
            let rect = control.convert(control.bounds, to: content)
            guard content.bounds.contains(rect), rect.width > 0, rect.height > 0 else {
                fputs("Presenter control \(control.title) \(rect) is outside \(content.bounds).\n", stderr)
                exit(12)
            }
        }
        let center = start.convert(start.bounds, to: content)
        let backRect = back.convert(back.bounds, to: content)
        let nextRect = next.convert(next.bounds, to: content)
        let stopRect = stop.convert(stop.bounds, to: content)
        let stateRect = stateButton.convert(stateButton.bounds, to: content)
        let rule = separator.convert(separator.bounds, to: content)
        let gearRect = settingsButton.convert(settingsButton.bounds, to: content)
        let helpRect = helpButton.convert(helpButton.bounds, to: content)
        func textLeft(_ field: NSTextField) -> CGFloat {
            field.convert(NSPoint(x: field.alignmentRectInsets.left, y: 0), to: content).x
        }
        let headingX = textLeft(title)
        let cueX = cue.convert(cue.textContainerOrigin, to: content).x
        let stepX = textLeft(step)
        let statusX = textLeft(status)
        guard backRect.maxX < center.minX, center.maxX < nextRect.minX,
              abs(center.midX - content.bounds.midX) < 2,
              stopRect.maxY < center.minY, stateRect.maxX < stopRect.minX,
              abs(stopRect.midY - stateRect.midY) < 2, rule.minY > center.maxY,
              gearRect.maxX < helpRect.minX,
              content.bounds.maxX - helpRect.maxX < 16,
              abs(headingX - cueX) < 1, abs(headingX - stepX) < 1, abs(headingX - statusX) < 1 else {
            fputs("Presenter positions: \([backRect, center, nextRect, stopRect, gearRect, helpRect]) in \(content.bounds).\n", stderr)
            fputs("Text left edges: \([headingX, stepX, statusX, cueX]), separator: \(rule).\n", stderr)
            exit(12)
        }
        notes.setContentSize(initialContentSize)
        notes.contentView?.layoutSubtreeIfNeeded()
        snapshot(content, as: "presenter")
        settingsButton.performClick(nil)
        guard settingsPopover.isShown, let settingsContent = settingsPopover.contentViewController?.view,
              let settingsWindow = settingsContent.window else {
            fputs("Settings button did not open an attached popover.\n", stderr)
            exit(16)
        }
        guard settingsWindow.sharingType == .none else {
            fputs("Settings window did not request capture exclusion.\n", stderr)
            exit(16)
        }
        settingsContent.layoutSubtreeIfNeeded()
        for control in [smaller!, larger!, alignmentPicker!] as [NSControl] {
            let rect = control.convert(control.bounds, to: settingsContent)
            guard control.window === settingsWindow, settingsContent.bounds.contains(rect),
                  rect.width > 0, rect.height > 0 else {
                fputs("Settings control \(rect) is outside \(settingsContent.bounds).\n", stderr)
                exit(16)
            }
        }
        snapshot(settingsWindow.contentView?.superview ?? settingsContent, as: "settings")
        let originalFont = fontSize
        larger.performClick(nil)
        guard fontSize == min(maximumFontSize, originalFont+2), narrationSize() == fontSize,
              attributes(at: initialFrame.detail)?[.font] as? NSFont == .systemFont(ofSize: 12) else { exit(5) }
        smaller.performClick(nil)
        guard fontSize == originalFont else { exit(6) }
        fontSize = minimumFontSize; applyTextStyle(); smaller.performClick(nil)
        guard fontSize == minimumFontSize, !smaller.isEnabled else { exit(9) }
        fontSize = maximumFontSize; applyTextStyle(); larger.performClick(nil)
        guard fontSize == maximumFontSize, !larger.isEnabled else { exit(10) }
        fontSize = originalFont; applyTextStyle()
        for (index, expected) in [NSTextAlignment.left, .center, .right].enumerated() {
            alignmentPicker.selectedSegment = index
            alignmentPicker.sendAction(alignmentPicker.action!, to: self)
            guard let say = frame?.say,
                  let cueStyle = attributes(at: say)?[.paragraphStyle] as? NSParagraphStyle,
                  let infoStyle = attributes(at: "Info")?[.paragraphStyle] as? NSParagraphStyle,
                  cueStyle.alignment == expected, infoStyle.alignment == .left else { exit(7) }
        }
        fontSize = 30; applyTextStyle(); saveSettings()
        fontSize = 22; textAlignment = .left; restoreSettings()
        guard fontSize == 30, textAlignment == .right else { exit(8) }
        alignmentPicker.selectedSegment = 0; changeAlignment()
        helpButton.performClick(nil)
        guard !helpPopover.isShown, !helpButton.trackingAreas.isEmpty,
              let enter = NSEvent.enterExitEvent(with: .mouseEntered, location: .zero,
                  modifierFlags: [], timestamp: 0, windowNumber: notes.windowNumber,
                  context: nil, eventNumber: 0, trackingNumber: 0, userData: nil),
              let leave = NSEvent.enterExitEvent(with: .mouseExited, location: .zero,
                  modifierFlags: [], timestamp: 0, windowNumber: notes.windowNumber,
                  context: nil, eventNumber: 0, trackingNumber: 0, userData: nil) else { exit(17) }
        helpButton.mouseEntered(with: enter)
        guard !settingsPopover.isShown, helpPopover.isShown,
              let helpContent = helpPopover.contentViewController?.view,
              let helpWindow = helpContent.window, helpWindow.sharingType == .none,
              let legend = helpContent.subviews.first as? NSTextField,
              legend.stringValue.contains("F7"), legend.stringValue.contains("F9"),
              legend.stringValue.split(separator: "\n").count == 3 else { exit(17) }
        helpButton.mouseExited(with: leave)
        guard !helpPopover.isShown, notes.sharingType == .none else { exit(17) }
        let retainedFrame = notes.frame
        let stateSize = stateButton.frame.size
        let states: [(String, String?, Bool)] = [
            ("Introduction", "start", false), ("Preparing", nil, false), ("Waiting", nil, false),
            ("Awaiting confirmation", "confirm", false), ("Step failed", "confirm", false),
            ("Outcome unknown", nil, false), ("Checking route", nil, false),
            ("Route changed", "restore", false), ("Restoring route", nil, false),
            ("Ready", "next", true), ("Finished", nil, true),
        ]
        for (index, (status, command, canGoBack)) in states.enumerated() {
            var state: [String: Any] = ["status": status, "channel": "Native check", "scenario_id": "native1", "step": "3/5",
                "title": "Updated step", "say": "Updated narration stays with the controls.",
                "detail": "Current step detail", "wait": "", "generation": index + 1, "can_previous": canGoBack]
            if let command { state["primary"] = command }
            guard let data = try? JSONSerialization.data(withJSONObject: state) else { exit(13) }
            do { try data.write(to: session.appendingPathComponent("panel.json")) } catch { exit(13) }
            refresh()
            content.layoutSubtreeIfNeeded()
            let offRoute = status == "Route changed" || status == "Restoring route"
            guard back.isEnabled == canGoBack, start.isEnabled == (command == "start"), start.title == "Start",
                  stateButton.isEnabled == (command == "confirm" || command == "restore"),
                  stateButton.frame.size == stateSize, buttons.allSatisfy({ !$0.isHidden }),
                  next.isEnabled == (command == "next"), stop.isEnabled,
                  cue.string.contains("Updated narration") == !offRoute, cue.string.contains("Current step detail"),
                  let frame, self.status.stringValue == "Status: \(frame.statusLabel)",
                  self.status.attributedStringValue.attribute(.foregroundColor, at: 0, effectiveRange: nil) as? NSColor == .labelColor,
                  self.status.attributedStringValue.attribute(.foregroundColor, at: "Status: ".utf16.count, effectiveRange: nil) as? NSColor == frame.statusColor,
                  attributes(at: "Info")?[.foregroundColor] as? NSColor == .labelColor,
                  offRoute || narrationSize() == fontSize,
                  notes.frame == retainedFrame else { exit(13) }
            if command == "start" {
                forward()
                start.performClick(nil)
            }
            if status == "Awaiting confirmation" || command == "restore" {
                forward()
                if command == "restore" { snapshot(content, as: "off-route") }
                stateButton.performClick(nil)
            }
        }
        guard buttons.allSatisfy({ $0.refusesFirstResponder }) else { exit(14) }
        let tipState: [String: Any] = ["status": "Ready", "channel": "Native check", "scenario_id": "native1", "step": "4/5",
            "title": "Tip", "say": "Narration.\n\n<Presenter note>\nAnother paragraph.", "wait": "Copilot's reply",
            "detail": "", "generation": states.count + 1, "primary": "next", "can_previous": true]
        guard let tipData = try? JSONSerialization.data(withJSONObject: tipState) else { exit(15) }
        do { try tipData.write(to: session.appendingPathComponent("panel.json"), options: .atomic) } catch { exit(15) }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) {
            guard self.cue.string.contains("Wait: Copilot's reply"),
                  attributes(at: "Wait: Copilot's reply")?[.foregroundColor] as? NSColor == NSColor.systemYellow,
                  attributes(at: "Wait: Copilot's reply")?[.font] as? NSFont == .systemFont(ofSize: 12),
                  attributes(at: "Narration.")?[.font] as? NSFont == .systemFont(ofSize: self.fontSize),
                  let noteFont = attributes(at: "<Presenter note>")?[.font] as? NSFont,
                  NSFontManager.shared.traits(of: noteFont).contains(.italicFontMask),
                  attributes(at: "<Presenter note>")?[.foregroundColor] as? NSColor == .secondaryLabelColor,
                  self.cue.string.contains("Narration.\n\n<Presenter note>\nAnother paragraph.") else { exit(15) }
            self.back.performClick(nil); self.next.performClick(nil); self.stop.performClick(nil)
            fputs("Native UI: session updates, button gating, notes, popovers, capture exclusion and preferences passed.\n", stderr)
            NSApp.terminate(nil)
        }
    }
    @objc func previous() {
        if let frame, frame.canPrevious { send("previous \(frame.generation)") }
    }
    @objc func startScenario() {
        if let frame, frame.primary == "start" { send("start \(frame.generation)") }
    }
    @objc func performStateAction() {
        if let frame, let command = frame.primary, command == "confirm" || command == "restore" {
            send("\(command) \(frame.generation)")
        }
    }
    @objc func stopScenario() { send("quit") }
    @objc func forward() {
        if let frame, frame.primary == "next" { send("next \(frame.generation)") }
    }
    func refresh() {
        if getppid() != owner { NSApp.terminate(nil); return }
        guard let data = try? Data(contentsOf: session.appendingPathComponent("panel.json")), data != last,
              let state = try? JSONDecoder().decode(PresenterFrame.self, from: data) else { return }
        let previous = frame
        let newStep = previous?.step != state.step || previous?.title != state.title
        let newCue = newStep || previous?.say != state.say || previous?.showsTranscript != state.showsTranscript
        last = data
        frame = state
        if previous?.channel != state.channel || previous?.scenarioID != state.scenarioID {
            let heading = NSMutableAttributedString(string: "\(state.channel) ", attributes: [
                .font: titleFont, .foregroundColor: NSColor.labelColor,
            ])
            heading.append(NSAttributedString(string: "(\(state.scenarioID))", attributes: [
                .font: titleFont, .foregroundColor: NSColor.secondaryLabelColor,
            ]))
            title.attributedStringValue = heading
            title.toolTip = state.heading
            notes.title = state.heading
        }
        if newStep {
            step.stringValue = state.stepHeading
            step.toolTip = state.stepHeading
        }
        if previous?.status != state.status {
            let line = NSMutableAttributedString(string: "Status: ", attributes: [
                .font: statusFont, .foregroundColor: NSColor.labelColor,
            ])
            line.append(NSAttributedString(string: state.statusLabel, attributes: [
                .font: statusFont, .foregroundColor: state.statusColor,
            ]))
            status.attributedStringValue = line
        }
        if newCue || previous?.detail != state.detail || previous?.wait != state.wait {
            renderCue()
        }
        if newCue {
            cue.scrollRangeToVisible(NSRange(location: 0, length: 0))
        }
        back.isEnabled = state.canPrevious
        start.isEnabled = state.primary == "start"
        next.isEnabled = state.primary == "next"
        stateButton.isEnabled = state.primary == "confirm" || state.primary == "restore"
        let (caption, symbol): (String, String) = switch (state.primary, state.status) {
        case ("restore", _): ("Check route", "arrow.clockwise")
        case (_, "Checking route"): ("Checking", "magnifyingglass")
        case (_, "Restoring route"): ("Restoring", "arrow.clockwise")
        case (_, "Waiting"): ("Waiting", "hourglass")
        case (_, "Working"), (_, "Preparing"): ("Working", "hourglass")
        case (_, "Outcome unknown"), (_, "Preparation failed"), (_, "Preview failed"), (_, "Session expired"):
            ("Blocked", "lock")
        default: ("Confirm", "checkmark")
        }
        if stateButton.title != caption {
            stateButton.title = caption
            stateButton.image = NSImage(systemSymbolName: symbol, accessibilityDescription: nil)
            stateButton.setAccessibilityLabel(caption == "Confirm" ? "Confirm completion" : caption)
        }
    }
    func installKeys() {
        let callback: EventHandlerUPP = { _, event, pointer in
            guard let event = event, let pointer = pointer else { return noErr }
            var key = EventHotKeyID()
            GetEventParameter(event, EventParamName(kEventParamDirectObject), EventParamType(typeEventHotKeyID), nil, MemoryLayout<EventHotKeyID>.size, nil, &key)
            let controller = Unmanaged<Controller>.fromOpaque(pointer).takeUnretainedValue()
            if key.id == 7 { controller.previous() }
            if key.id == 9 { controller.forward() }
            return noErr
        }
        var spec = EventTypeSpec(eventClass: OSType(kEventClassKeyboard), eventKind: UInt32(kEventHotKeyPressed))
        InstallEventHandler(GetApplicationEventTarget(), callback, 1, &spec, Unmanaged.passUnretained(self).toOpaque(), nil)
        for (id, code) in [(7, kVK_F7), (9, kVK_F9)] {
            var ref: EventHotKeyRef?
            RegisterEventHotKey(UInt32(code), 0, EventHotKeyID(signature: 0x444D494F, id: UInt32(id)), GetApplicationEventTarget(), 0, &ref)
            keys.append(ref)
        }
    }
}
@main
struct OverlayMain {
    static func main() {
        let arguments = CommandLine.arguments
        if arguments.contains("--version") {
            print("demio-overlay \(buildVersion)")
            return
        }
        if arguments.dropFirst().first == "--native-input" {
            do {
                try NativeInput.run(Array(arguments.dropFirst(2)))
            } catch let error as InputFailure {
                fputs(error.message + "\n", stderr)
                exit(error.offRoute ? 3 : 2)
            } catch {
                fputs(error.localizedDescription + "\n", stderr)
                exit(2)
            }
            return
        }
        guard arguments.count >= 3 else { exit(2) }
        let application = NSApplication.shared
        application.setActivationPolicy(.accessory)
        let controller = Controller()
        application.delegate = controller
        application.run()
    }
}
