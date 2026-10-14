import AppKit
import Carbon

let buildVersion = "0.0.0"
if CommandLine.arguments.contains("--version") {
    print("demio-overlay \(buildVersion)")
    exit(0)
}
guard CommandLine.arguments.count >= 3 else { exit(2) }
let session = URL(fileURLWithPath: CommandLine.arguments[1])
let owner = Int32(CommandLine.arguments[2]) ?? 0
func send(_ command: String) { print(command); fflush(stdout) }
final class Controller: NSObject, NSApplicationDelegate, NSWindowDelegate {
    var remote: NSPanel!
    var notes: NSPanel!
    let cue = NSTextView()
    let title = NSTextField(labelWithString: "Ready")
    var last = Data()
    var keys: [EventHotKeyRef?] = []
    var timer: Timer?
    func panel(_ rect: NSRect, title: String, resizable: Bool) -> NSPanel {
        var style: NSWindow.StyleMask = [.titled, .nonactivatingPanel]
        if resizable { style.insert(.resizable) }
        let window = NSPanel(contentRect: rect, styleMask: style, backing: .buffered, defer: false)
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
        let top = NSScreen.main?.visibleFrame.maxY ?? 850
        remote = panel(NSRect(x: 25, y: top-110, width: 190, height: 54), title: "Demio", resizable: false)
        let row = NSStackView()
        row.orientation = .horizontal
        row.distribution = .fillEqually
        row.spacing = 10
        row.translatesAutoresizingMaskIntoConstraints = false
        let back = NSButton(title: "← Back", target: self, action: #selector(previous))
        let next = NSButton(title: "Next →", target: self, action: #selector(forward))
        back.bezelStyle = .rounded; next.bezelStyle = .rounded
        row.addArrangedSubview(back); row.addArrangedSubview(next)
        remote.contentView!.addSubview(row)
        NSLayoutConstraint.activate([row.leadingAnchor.constraint(equalTo: remote.contentView!.leadingAnchor, constant: 10), row.trailingAnchor.constraint(equalTo: remote.contentView!.trailingAnchor, constant: -10), row.centerYAnchor.constraint(equalTo: remote.contentView!.centerYAnchor)])
        notes = panel(NSRect(x: 25, y: top-400, width: 440, height: 250), title: "Demio · Speaker notes", resizable: true)
        notes.minSize = NSSize(width: 280, height: 160)
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
        cue.font = .systemFont(ofSize: 22)
        cue.textColor = .labelColor
        cue.textContainerInset = NSSize(width: 4, height: 4)
        cue.isVerticallyResizable = true
        cue.isHorizontallyResizable = false
        cue.autoresizingMask = [.width]
        cue.textContainer?.widthTracksTextView = true
        cue.textContainer?.containerSize = NSSize(width: 410, height: CGFloat.greatestFiniteMagnitude)
        scroll.documentView = cue
        body.addArrangedSubview(title); body.addArrangedSubview(scroll)
        notes.contentView!.addSubview(body)
        NSLayoutConstraint.activate([body.leadingAnchor.constraint(equalTo: notes.contentView!.leadingAnchor, constant: 15), body.trailingAnchor.constraint(equalTo: notes.contentView!.trailingAnchor, constant: -15), body.topAnchor.constraint(equalTo: notes.contentView!.topAnchor, constant: 12), body.bottomAnchor.constraint(equalTo: notes.contentView!.bottomAnchor, constant: -12), scroll.widthAnchor.constraint(equalTo: body.widthAnchor)])
        remote.orderFrontRegardless(); notes.orderFrontRegardless()
        installKeys()
        timer = Timer.scheduledTimer(withTimeInterval: 0.1, repeats: true) { [weak self] _ in self?.refresh() }
        refresh()
        if CommandLine.arguments.contains("--smoke") {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { self.smoke() }
        }
    }
    func smoke() {
        guard let buttons = (remote.contentView?.subviews.first as? NSStackView)?.arrangedSubviews as? [NSButton], buttons.count == 2,
              !remote.styleMask.contains(.resizable), notes.styleMask.contains(.resizable), !cue.string.isEmpty else { exit(3) }
        let original = notes.frame
        notes.setFrame(NSRect(x: original.minX+30, y: original.minY+20, width: 600, height: 360), display: true)
        notes.contentView?.layoutSubtreeIfNeeded()
        guard notes.frame.width == 600, notes.frame.height == 360 else { exit(4) }
        buttons[0].performClick(nil); buttons[1].performClick(nil)
        for (name, window) in [("remote", remote!), ("notes", notes!)] {
            if let view = window.contentView, let bitmap = view.bitmapImageRepForCachingDisplay(in: view.bounds) {
                view.cacheDisplay(in: view.bounds, to: bitmap)
                if let png = bitmap.representation(using: .png, properties: [:]) {
                    try? png.write(to: session.appendingPathComponent(name+".png"))
                }
            }
        }
        fputs("Native UI: two controls, readable notes, move and resize passed.\n", stderr)
        NSApp.terminate(nil)
    }
    @objc func previous() { send("previous") }
    @objc func forward() { send("next") }
    func refresh() {
        if getppid() != owner { NSApp.terminate(nil); return }
        guard let data = try? Data(contentsOf: session.appendingPathComponent("panel.json")), data != last,
              let state = (try? JSONSerialization.jsonObject(with: data)) as? [String: String] else { return }
        last = data
        title.stringValue = "\(state["status"] ?? "Ready") · \(state["step"] ?? "") · \(state["role"] ?? "")"
        notes.title = "Demio · \(state["channel"] ?? "Speaker notes")"
        let content = "\(state["title"] ?? "")\n\n\(state["say"] ?? "")"
        if cue.string != content {
            cue.string = content
            cue.scrollRangeToVisible(NSRange(location: 0, length: 0))
        }
        // Ignore repeated button presses while an operation is in flight.
        (remote.contentView?.subviews.first as? NSStackView)?.arrangedSubviews.forEach {
            ($0 as? NSButton)?.isEnabled = state["status"] == "Ready"
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
