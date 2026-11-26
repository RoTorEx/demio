import AppKit
import ApplicationServices
import Carbon

struct InputFailure: Error {
    let message: String
    var offRoute = false
}

enum NativeInput {
    static func run(_ arguments: [String]) throws {
        guard AXIsProcessTrusted(), CGPreflightPostEventAccess() else {
            throw InputFailure(message: "Allow Accessibility for the app running Demio in System Settings.")
        }
        if arguments == ["check"] { return }
        guard arguments.count >= 3 else {
            throw InputFailure(message: "Invalid native input command.")
        }
        let operation = arguments[0]
        let text = arguments[2]
        guard !text.isEmpty, !text.unicodeScalars.contains(where: { CharacterSet.controlCharacters.contains($0) }) else {
            throw InputFailure(message: "Native input requires one line of text.")
        }
        switch operation {
        case "address", "close-tab":
            guard arguments.count == 3, let pid = pid_t(arguments[1]) else {
                throw InputFailure(message: "Invalid rehearsal Chrome process.")
            }
            let target = try ActiveWindow(pid: pid, bundle: "com.google.Chrome")
            if operation == "address" {
                let address = try target.find("Chrome address bar") { element, role in
                    role == "AXTextField" && names(of: element).contains(where: {
                        $0.localizedCaseInsensitiveContains("address and search bar")
                    })
                }
                try target.click(address)
                try target.key(CGKeyCode(kVK_ANSI_A), flags: .maskCommand, focused: address)
                try target.type(text, focused: address)
            } else {
                try target.closeTab(titled: text)
            }
        case "terminal":
            guard arguments.count == 4 else {
                throw InputFailure(message: "The rehearsal Terminal bounds are missing.")
            }
            let pid = NSWorkspace.shared.frontmostApplication?.processIdentifier ?? 0
            let target = try ActiveWindow(pid: pid, bundle: "com.apple.Terminal")
            try target.verifyTerminalBounds(arguments[3])
            if arguments[1] == "click" {
                let prompt = try target.find("Terminal prompt") { _, role in role == "AXTextArea" }
                try target.click(prompt)
            }
            let input = try target.focusedElement()
            try target.key(CGKeyCode(kVK_ANSI_U), flags: .maskControl, focused: input)
            try target.type(text, focused: input)
        default:
            throw InputFailure(message: "Unknown native input command.")
        }
    }
}

private func attribute(_ element: AXUIElement, _ name: String) -> CFTypeRef? {
    var value: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, name as CFString, &value) == .success else { return nil }
    return value
}

private func elementAttribute(_ element: AXUIElement, _ name: String) -> AXUIElement? {
    guard let value = attribute(element, name), CFGetTypeID(value) == AXUIElementGetTypeID() else { return nil }
    return (value as! AXUIElement)
}

private func names(of element: AXUIElement) -> [String] {
    [kAXTitleAttribute, kAXDescriptionAttribute].compactMap { attribute(element, $0) as? String }
}

private struct ActiveWindow {
    let pid: pid_t
    let app: AXUIElement
    let window: AXUIElement
    let source = CGEventSource(stateID: .privateState)

    init(pid: pid_t, bundle: String) throws {
        guard NSWorkspace.shared.frontmostApplication?.processIdentifier == pid,
              NSRunningApplication(processIdentifier: pid)?.bundleIdentifier == bundle else {
            throw InputFailure(message: "Return to the rehearsal window.", offRoute: true)
        }
        self.pid = pid
        let app = AXUIElementCreateApplication(pid)
        AXUIElementSetMessagingTimeout(app, 0.5)
        guard let window = elementAttribute(app, kAXFocusedWindowAttribute) else {
            throw InputFailure(message: "The rehearsal window is unavailable.", offRoute: true)
        }
        self.app = app
        self.window = window
    }

    func verify(focused expected: AXUIElement? = nil) throws {
        guard NSWorkspace.shared.frontmostApplication?.processIdentifier == pid,
              let current = elementAttribute(app, kAXFocusedWindowAttribute), CFEqual(window, current) else {
            throw InputFailure(message: "Return to the rehearsal window.", offRoute: true)
        }
        if let expected {
            guard let actual = elementAttribute(app, kAXFocusedUIElementAttribute), CFEqual(expected, actual) else {
                throw InputFailure(message: "Return to the rehearsal control before continuing.", offRoute: true)
            }
        }
    }

    func focusedElement() throws -> AXUIElement {
        guard let element = elementAttribute(app, kAXFocusedUIElementAttribute),
              attribute(element, kAXRoleAttribute) as? String == "AXTextArea" else {
            throw InputFailure(message: "Focus the rehearsal Terminal prompt.", offRoute: true)
        }
        return element
    }

    func find(_ label: String, in root: AXUIElement? = nil,
              matching: (AXUIElement, String) -> Bool) throws -> AXUIElement {
        var pending = [root ?? window]
        var index = 0
        while index < pending.count && index < 300 {
            let element = pending[index]
            index += 1
            let role = attribute(element, kAXRoleAttribute) as? String ?? ""
            if role == "AXWebArea" { continue }
            if matching(element, role) { return element }
            pending.append(contentsOf: attribute(element, kAXChildrenAttribute) as? [AXUIElement] ?? [])
        }
        throw InputFailure(message: "\(label) is unavailable in the rehearsal window.")
    }

    func closeTab(titled title: String) throws {
        func closeButton() throws -> AXUIElement {
            try find("Chrome tab close button") { element, role in
                role == "AXButton" && names(of: element).contains("Close \(title)")
            }
        }
        if let button = try? closeButton() {
            try click(button)
            return
        }
        let tab = try find("The rehearsal tab") { element, role in
            (role == "AXRadioButton" || role == "AXTab") &&
                names(of: element).contains(where: { $0 == title || $0.hasPrefix(title + ", ") })
        }
        try move(to: center(of: tab))
        Thread.sleep(forTimeInterval: 0.18)
        let button = try find("Chrome tab close button", in: tab) { element, role in
            role == "AXButton" && names(of: element).contains(where: {
                $0 == "Close \(title)" || $0 == "Close tab" || $0 == "Close"
            })
        }
        try click(button)
    }

    func bounds(of element: AXUIElement) throws -> CGRect {
        guard let position = attribute(element, kAXPositionAttribute), CFGetTypeID(position) == AXValueGetTypeID(),
              let size = attribute(element, kAXSizeAttribute), CFGetTypeID(size) == AXValueGetTypeID() else {
            throw InputFailure(message: "The rehearsal control has no visible bounds.")
        }
        var point = CGPoint.zero
        var dimensions = CGSize.zero
        guard AXValueGetValue(position as! AXValue, .cgPoint, &point),
              AXValueGetValue(size as! AXValue, .cgSize, &dimensions),
              dimensions.width > 0, dimensions.height > 0 else {
            throw InputFailure(message: "The rehearsal control is hidden.")
        }
        return CGRect(origin: point, size: dimensions)
    }

    func center(of element: AXUIElement) throws -> CGPoint {
        let frame = try bounds(of: element)
        return CGPoint(x: frame.midX, y: frame.midY)
    }

    func verifyTerminalBounds(_ text: String) throws {
        let expected = text.split(separator: ",").compactMap { Double($0.trimmingCharacters(in: .whitespaces)) }
        let frame = try bounds(of: window)
        let actual = [frame.minX, frame.minY, frame.maxX, frame.maxY]
        guard expected.count == actual.count,
              zip(expected, actual).allSatisfy({ pair in abs(pair.0 - Double(pair.1)) <= 2 }) else {
            throw InputFailure(message: "Return to the rehearsal Terminal window.", offRoute: true)
        }
    }

    func move(to destination: CGPoint) throws {
        let start = CGEvent(source: nil)?.location ?? destination
        for frame in 1...18 {
            try verify()
            let progress = Double(frame) / 18
            let eased = progress * progress * (3 - 2 * progress)
            let position = CGPoint(x: start.x + (destination.x - start.x) * eased,
                                   y: start.y + (destination.y - start.y) * eased)
            CGWarpMouseCursorPosition(position)
            CGEvent(mouseEventSource: source, mouseType: .mouseMoved,
                    mouseCursorPosition: position, mouseButton: .left)?.postToPid(pid)
            Thread.sleep(forTimeInterval: 0.026)
        }
    }

    func click(_ control: AXUIElement) throws {
        let position = try center(of: control)
        try move(to: position)
        try verify()
        guard try center(of: control) == position else {
            throw InputFailure(message: "The rehearsal control moved. Return to it before continuing.", offRoute: true)
        }
        var hit: AXUIElement?
        var hitPID: pid_t = 0
        let system = AXUIElementCreateSystemWide()
        guard AXUIElementCopyElementAtPosition(system, Float(position.x), Float(position.y), &hit) == .success,
              let hit, AXUIElementGetPid(hit, &hitPID) == .success, hitPID == pid else {
            throw InputFailure(message: "Uncover the rehearsal control before continuing.", offRoute: true)
        }
        for kind in [CGEventType.leftMouseDown, .leftMouseUp] {
            guard let event = CGEvent(mouseEventSource: source, mouseType: kind,
                                      mouseCursorPosition: position, mouseButton: .left) else {
                throw InputFailure(message: "Could not create a mouse click.")
            }
            event.setIntegerValueField(.mouseEventClickState, value: 1)
            event.post(tap: .cghidEventTap)
        }
        Thread.sleep(forTimeInterval: 0.15)
    }

    func key(_ code: CGKeyCode, text: String = "", flags: CGEventFlags = [],
             focused: AXUIElement) throws {
        try verify(focused: focused)
        let units = Array(text.utf16)
        for isDown in [true, false] {
            guard let event = CGEvent(keyboardEventSource: source, virtualKey: code, keyDown: isDown) else {
                throw InputFailure(message: "Could not create a keystroke.")
            }
            event.flags = flags
            if !units.isEmpty {
                units.withUnsafeBufferPointer {
                    event.keyboardSetUnicodeString(stringLength: $0.count, unicodeString: $0.baseAddress)
                }
            }
            event.postToPid(pid)
        }
    }

    func type(_ text: String, focused: AXUIElement) throws {
        for letter in text {
            try key(0, text: String(letter), focused: focused)
            Thread.sleep(forTimeInterval: 0.09)
        }
        try key(CGKeyCode(kVK_Return), focused: focused)
    }
}
