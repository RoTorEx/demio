import AppKit
import Carbon

let buildVersion = "0.1.16"
if CommandLine.arguments.contains("--version") {
    print("demio-overlay \(buildVersion)")
    exit(0)
}
guard CommandLine.arguments.count >= 3 else { exit(2) }
let session = URL(fileURLWithPath: CommandLine.arguments[1])
let owner = Int32(CommandLine.arguments[2]) ?? 0
func send(_ command: String) { print(command); fflush(stdout) }

struct PresenterFrame: Decodable {
    let status: String
    let channel: String
    let step: String
    let title: String
    let role: String
    let say: String
    let wait: String
    let detail: String
    let generation: UInt64
    let primary: String?
    let canPrevious: Bool

    enum CodingKeys: String, CodingKey {
        case status, channel, step, title, role, say, wait, detail, generation, primary
        case canPrevious = "can_previous"
    }
}

final class Controller: NSObject, NSApplicationDelegate, NSWindowDelegate {
    private static let defaultFontSize: CGFloat = 11
    private let minimumFontSize: CGFloat = 9
    private let maximumFontSize: CGFloat = 48
    private let initialContentSize = NSSize(width: 360, height: 240)
    var notes: NSPanel!
    var back: NSButton!
    var waitRange: NSRange?
    var primary: NSButton!
    var stop: NSButton!
    var frame: PresenterFrame?
    let cue = NSTextView()
    let title = NSTextField(labelWithString: "Ready")
    var last = Data()
    var keys: [EventHotKeyRef?] = []
    var timer: Timer?
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
        let paragraph = NSMutableParagraphStyle()
        paragraph.alignment = textAlignment
        let attributes: [NSAttributedString.Key: Any] = [.font: NSFont.systemFont(ofSize: fontSize), .foregroundColor: NSColor.labelColor, .paragraphStyle: paragraph]
        cue.defaultParagraphStyle = paragraph
        cue.typingAttributes = attributes
        cue.textStorage?.setAttributes(attributes, range: NSRange(location: 0, length: (cue.string as NSString).length))
        if let tip = waitRange, NSMaxRange(tip) <= (cue.string as NSString).length {
            cue.textStorage?.addAttribute(.foregroundColor, value: NSColor.systemRed, range: tip)
        }
        fontLabel.stringValue = "\(Int(fontSize)) pt"
        smaller?.isEnabled = fontSize > minimumFontSize
        larger?.isEnabled = fontSize < maximumFontSize
    }
    @objc func decreaseFont() { fontSize = max(minimumFontSize, fontSize-2); applyTextStyle(); saveSettings() }
    @objc func increaseFont() { fontSize = min(maximumFontSize, fontSize+2); applyTextStyle(); saveSettings() }
    @objc func changeAlignment() {
        textAlignment = [.left, .center, .right][alignmentPicker.selectedSegment]
        applyTextStyle(); saveSettings()
    }
    func panel(_ rect: NSRect, title: String, resizable: Bool) -> NSPanel {
        var style: NSWindow.StyleMask = [.titled, .nonactivatingPanel]
        if resizable { style.insert(.resizable) }
        let window = NSPanel(contentRect: rect, styleMask: style, backing: .buffered, defer: false)
        // Preserve the original presenter panel's exclusion from legacy capture.
        // ScreenCaptureKit recorders may ignore this flag on newer macOS versions.
        window.sharingType = .none
        window.title = title
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
    func applicationDidFinishLaunching(_ notification: Notification) {
        restoreSettings()
        let screen = NSScreen.main?.visibleFrame
        let origin = NSPoint(x: (screen?.minX ?? 0) + 25, y: (screen?.maxY ?? 850) - initialContentSize.height - 80)
        notes = panel(NSRect(origin: origin, size: initialContentSize), title: "Demio", resizable: true)
        notes.contentMinSize = NSSize(width: 320, height: 200)
        let row = NSStackView()
        row.orientation = .horizontal
        row.distribution = .fill
        row.spacing = 10
        back = NSButton(title: "← Back", target: self, action: #selector(previous))
        primary = NSButton(title: "Start scenario", target: self, action: #selector(performPrimary))
        stop = NSButton(title: "Stop", target: self, action: #selector(stopScenario))
        for button in [back!, primary!, stop!] {
            button.bezelStyle = .rounded
            row.addArrangedSubview(button)
        }
        back.setContentHuggingPriority(.defaultHigh, for: .horizontal)
        stop.setContentHuggingPriority(.defaultHigh, for: .horizontal)
        let body = NSStackView()
        body.orientation = .vertical
        body.alignment = .leading
        body.spacing = 12
        body.translatesAutoresizingMaskIntoConstraints = false
        title.font = .systemFont(ofSize: 13, weight: .semibold)
        title.lineBreakMode = .byTruncatingTail
        let scroll = NSScrollView()
        scroll.hasVerticalScroller = true
        scroll.drawsBackground = false
        cue.isEditable = false
        cue.isSelectable = true
        cue.drawsBackground = false
        cue.font = .systemFont(ofSize: fontSize)
        cue.textColor = .labelColor
        cue.textContainerInset = NSSize(width: 4, height: 4)
        cue.isVerticallyResizable = true
        cue.isHorizontallyResizable = false
        cue.autoresizingMask = [.width]
        cue.textContainer?.widthTracksTextView = true
        cue.textContainer?.containerSize = NSSize(width: initialContentSize.width - 30, height: CGFloat.greatestFiniteMagnitude)
        scroll.documentView = cue
        let controls = NSStackView()
        controls.orientation = .horizontal
        controls.spacing = 8
        smaller = NSButton(title: "A−", target: self, action: #selector(decreaseFont))
        larger = NSButton(title: "A+", target: self, action: #selector(increaseFont))
        smaller.bezelStyle = .rounded; larger.bezelStyle = .rounded
        fontLabel.font = .monospacedDigitSystemFont(ofSize: 12, weight: .regular)
        alignmentPicker = NSSegmentedControl(labels: ["Left", "Center", "Right"], trackingMode: .selectOne, target: self, action: #selector(changeAlignment))
        alignmentPicker.selectedSegment = textAlignment == .center ? 1 : textAlignment == .right ? 2 : 0
        for view in [smaller!, fontLabel, larger!, alignmentPicker!] { controls.addArrangedSubview(view) }
        // With macOS keyboard navigation on, the first button (Back) would hold focus and Space would
        // send the presenter back a step. Only clicks and F7/F9 may navigate.
        for control in [back!, primary!, stop!, smaller!, larger!, alignmentPicker!] as [NSControl] { control.refusesFirstResponder = true }
        body.addArrangedSubview(title); body.addArrangedSubview(controls); body.addArrangedSubview(scroll); body.addArrangedSubview(row)
        notes.contentView!.addSubview(body)
        NSLayoutConstraint.activate([body.leadingAnchor.constraint(equalTo: notes.contentView!.leadingAnchor, constant: 15), body.trailingAnchor.constraint(equalTo: notes.contentView!.trailingAnchor, constant: -15), body.topAnchor.constraint(equalTo: notes.contentView!.topAnchor, constant: 12), body.bottomAnchor.constraint(equalTo: notes.contentView!.bottomAnchor, constant: -12), scroll.widthAnchor.constraint(equalTo: body.widthAnchor), row.widthAnchor.constraint(equalTo: body.widthAnchor), scroll.heightAnchor.constraint(greaterThanOrEqualToConstant: 60)])
        notes.orderFrontRegardless()
        installKeys()
        timer = Timer.scheduledTimer(withTimeInterval: 0.1, repeats: true) { [weak self] _ in self?.refresh() }
        refresh()
        if CommandLine.arguments.contains("--smoke") {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { self.smoke() }
        }
    }
    func smoke() {
        guard notes.sharingType == .none else { exit(11) }
        guard NSApp.windows.filter({ $0.isVisible }).count == 1,
              back.window === notes, primary.window === notes, stop.window === notes, cue.window === notes,
              notes.contentView?.bounds.size == initialContentSize, cue.font?.pointSize == fontSize,
              notes.styleMask.contains(.resizable), !cue.string.isEmpty else { exit(3) }
        let original = notes.frame
        notes.setFrame(NSRect(x: original.minX+30, y: original.minY+20, width: 600, height: 360), display: true)
        notes.contentView?.layoutSubtreeIfNeeded()
        guard notes.frame.width == 600, notes.frame.height == 360 else { exit(4) }
        notes.setContentSize(notes.contentMinSize)
        notes.contentView?.layoutSubtreeIfNeeded()
        guard let content = notes.contentView else { exit(12) }
        for control in [back!, primary!, stop!, smaller!, larger!, alignmentPicker!] as [NSControl] {
            let rect = control.convert(control.bounds, to: content)
            guard content.bounds.contains(rect), rect.width > 0, rect.height > 0 else { exit(12) }
        }
        notes.setFrame(NSRect(x: original.minX+30, y: original.minY+20, width: 600, height: 360), display: true)
        let originalFont = fontSize
        larger.performClick(nil)
        guard fontSize == min(maximumFontSize, originalFont+2), cue.font?.pointSize == fontSize else { exit(5) }
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
            guard let style = cue.textStorage?.attribute(.paragraphStyle, at: 0, effectiveRange: nil) as? NSParagraphStyle, style.alignment == expected else { exit(7) }
        }
        fontSize = 30; applyTextStyle(); saveSettings()
        fontSize = 22; textAlignment = .left; restoreSettings()
        guard fontSize == 30, textAlignment == .right else { exit(8) }
        alignmentPicker.selectedSegment = 0; changeAlignment()
        let retainedFrame = notes.frame
        let states: [(String, String?, Bool)] = [
            ("Introduction", "start", false), ("Preparing", nil, false), ("Waiting", nil, false),
            ("Awaiting confirmation", "confirm", false), ("Step failed", "confirm", false),
            ("Outcome unknown", nil, false), ("Ready", "next", true), ("Finished", nil, true),
        ]
        for (index, (status, command, canGoBack)) in states.enumerated() {
            var state: [String: Any] = ["status": status, "channel": "Native check", "step": "3/5",
                "role": "Approver", "title": "Updated step", "say": "Updated narration stays with the controls.",
                "detail": "Current step detail", "wait": "", "generation": index + 1, "can_previous": canGoBack]
            if let command { state["primary"] = command }
            guard let data = try? JSONSerialization.data(withJSONObject: state) else { exit(13) }
            do { try data.write(to: session.appendingPathComponent("panel.json")) } catch { exit(13) }
            refresh()
            guard back.isEnabled == canGoBack, primary.isEnabled == (command != nil), stop.isEnabled,
                  cue.string.contains("Updated narration"), cue.string.contains("Current step detail"),
                  cue.font?.pointSize == fontSize,
                  notes.frame == retainedFrame else { exit(13) }
            if command == "start" || status == "Awaiting confirmation" {
                forward()
                primary.performClick(nil)
            }
        }
        guard [back!, primary!, stop!, smaller!, larger!, alignmentPicker!].allSatisfy({ $0.refusesFirstResponder }) else { exit(14) }
        let tipState: [String: Any] = ["status": "Ready", "channel": "Native check", "step": "4/5",
            "role": "Presenter", "title": "Tip", "say": "Narration.", "wait": "Copilot's reply",
            "detail": "", "generation": 9, "primary": "next", "can_previous": true]
        guard let tipData = try? JSONSerialization.data(withJSONObject: tipState) else { exit(15) }
        do { try tipData.write(to: session.appendingPathComponent("panel.json")) } catch { exit(15) }
        refresh()
        guard let tip = waitRange, cue.string.hasSuffix("Wait: Copilot's reply"),
              cue.textStorage?.attribute(.foregroundColor, at: tip.location, effectiveRange: nil) as? NSColor == NSColor.systemRed else { exit(15) }
        back.performClick(nil); primary.performClick(nil); stop.performClick(nil)
        for (name, window) in [("presenter", notes!)] {
            if let view = window.contentView, let bitmap = view.bitmapImageRepForCachingDisplay(in: view.bounds) {
                view.cacheDisplay(in: view.bounds, to: bitmap)
                if let png = bitmap.representation(using: .png, properties: [:]) {
                    try? png.write(to: session.appendingPathComponent(name+".png"))
                }
            }
        }
        fputs("Native UI: one compact panel with sharing disabled, navigation controls, move/resize, font size, alignment and saved preferences passed.\n", stderr)
        NSApp.terminate(nil)
    }
    @objc func previous() {
        if let frame, frame.canPrevious { send("previous \(frame.generation)") }
    }
    @objc func performPrimary() {
        if let frame, let command = frame.primary { send("\(command) \(frame.generation)") }
    }
    @objc func stopScenario() { send("quit") }
    func forward() {
        if frame?.primary == "next" { performPrimary() }
    }
    func refresh() {
        if getppid() != owner { NSApp.terminate(nil); return }
        guard let data = try? Data(contentsOf: session.appendingPathComponent("panel.json")), data != last,
              let state = try? JSONDecoder().decode(PresenterFrame.self, from: data) else { return }
        last = data
        frame = state
        title.stringValue = "\(state.status) · \(state.step) · \(state.role)"
        notes.title = "Demio · \(state.channel)"
        var content = [state.detail, state.title, state.say].filter { !$0.isEmpty }.joined(separator: "\n\n")
        waitRange = nil
        if !state.wait.isEmpty {
            let tip = "Wait: \(state.wait)"
            content += "\n\n"
            waitRange = NSRange(location: (content as NSString).length, length: (tip as NSString).length)
            content += tip
        }
        if cue.string != content {
            cue.string = content
            applyTextStyle()
            cue.scrollRangeToVisible(NSRange(location: 0, length: 0))
        }
        back.isEnabled = state.canPrevious
        primary.isEnabled = state.primary != nil
        switch state.primary {
        case "start": primary.title = "Start scenario"
        case "confirm": primary.title = "Confirm completion"
        default: primary.title = "Next →"
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
let application = NSApplication.shared
application.setActivationPolicy(.accessory)
let controller = Controller()
application.delegate = controller
application.run()
