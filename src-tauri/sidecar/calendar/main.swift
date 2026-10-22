// parley-calendar: reads the macOS Calendar (EventKit), which includes every Google, Exchange and
// iCloud account added to the Mac. Read-only: it never creates or edits events.
//
// Usage (one JSON value on stdout, exit 0; on failure {"error": "..."} and exit 1):
//   parley-calendar status            {"status": "fullAccess" | "notDetermined" | "denied" | "restricted" | "writeOnly"}
//   parley-calendar request           asks for full access (macOS shows its prompt once), then prints status
//   parley-calendar calendars         [{"id", "title", "source", "color"}]
//   parley-calendar events FROM TO    timed and all-day events between two Unix-ms times, sorted by start

import EventKit
import Foundation

let store = EKEventStore()

func emit(_ value: Any) {
    let data = try! JSONSerialization.data(withJSONObject: value, options: [.sortedKeys])
    FileHandle.standardOutput.write(data)
    FileHandle.standardOutput.write("\n".data(using: .utf8)!)
}

func fail(_ message: String) -> Never {
    emit(["error": message])
    exit(1)
}

func statusName() -> String {
    switch EKEventStore.authorizationStatus(for: .event) {
    case .fullAccess: return "fullAccess"
    case .writeOnly: return "writeOnly"
    case .denied: return "denied"
    case .restricted: return "restricted"
    case .notDetermined: return "notDetermined"
    @unknown default: return "denied"
    }
}

func hex(_ color: CGColor?) -> String {
    guard let c = color?.converted(to: CGColorSpace(name: CGColorSpace.sRGB)!, intent: .defaultIntent, options: nil),
          let p = c.components, p.count >= 3
    else { return "" }
    return String(format: "#%02X%02X%02X", Int(p[0] * 255), Int(p[1] * 255), Int(p[2] * 255))
}

func ms(_ date: Date) -> Int64 { Int64((date.timeIntervalSince1970 * 1000).rounded()) }

let args = CommandLine.arguments.dropFirst()
switch args.first {
case "status":
    emit(["status": statusName()])

case "request":
    let done = DispatchSemaphore(value: 0)
    store.requestFullAccessToEvents { _, _ in done.signal() }
    done.wait()
    emit(["status": statusName()])

case "calendars":
    guard statusName() == "fullAccess" else { fail("no calendar access") }
    emit(store.calendars(for: .event).map { cal in
        ["id": cal.calendarIdentifier, "title": cal.title, "source": cal.source?.title ?? "", "color": hex(cal.cgColor)]
    })

case "events":
    guard statusName() == "fullAccess" else { fail("no calendar access") }
    let rest = Array(args.dropFirst())
    guard rest.count == 2, let from = Double(rest[0]), let to = Double(rest[1]), to > from else {
        fail("usage: parley-calendar events FROM_MS TO_MS")
    }
    let predicate = store.predicateForEvents(
        withStart: Date(timeIntervalSince1970: from / 1000), end: Date(timeIntervalSince1970: to / 1000), calendars: nil)
    let events = store.events(matching: predicate).sorted { $0.startDate < $1.startDate }
    emit(events.map { e -> [String: Any] in
        let start = ms(e.startDate)
        let recurring = e.hasRecurrenceRules
        let attendees = (e.attendees ?? []).filter { !$0.isCurrentUser }.map { p -> [String: String] in
            let email = p.url.absoluteString.hasPrefix("mailto:") ? String(p.url.absoluteString.dropFirst(7)) : ""
            return ["name": p.name ?? "", "email": email]
        }
        return [
            // Occurrences of a repeating event share one identifier, so the start time tells them apart.
            "id": "\(e.calendarItemIdentifier):\(start)",
            "seriesId": recurring ? e.calendarItemIdentifier : NSNull(),
            "calendarId": e.calendar?.calendarIdentifier ?? "",
            "title": e.title ?? "",
            "startMs": start,
            "endMs": ms(e.endDate),
            "allDay": e.isAllDay,
            "attendees": attendees,
            "url": e.url?.absoluteString ?? "",
            "location": e.location ?? "",
            // Call links often live in the invite text; capped so one huge invite can't flood the app.
            "notes": String((e.notes ?? "").prefix(4000)),
        ]
    })

default:
    fail("usage: parley-calendar status | request | calendars | events FROM_MS TO_MS")
}
