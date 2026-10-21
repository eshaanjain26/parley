// parley-micwatch: reports which apps are capturing from a microphone, once a second.
// Reads Core Audio's process list (macOS 14.4+). It never opens an audio stream, so it needs no
// microphone permission and hears nothing.
//
// stdout: one JSON object per line, {"apps":["us.zoom.xos", ...]} (bundle IDs, sorted, unique)
// stdin:  close it to stop.

import CoreAudio
import Foundation

func address(_ selector: AudioObjectPropertySelector) -> AudioObjectPropertyAddress {
    AudioObjectPropertyAddress(
        mSelector: selector, mScope: kAudioObjectPropertyScopeGlobal, mElement: kAudioObjectPropertyElementMain)
}

func processObjects() -> [AudioObjectID] {
    var addr = address(kAudioHardwarePropertyProcessObjectList)
    var size: UInt32 = 0
    guard AudioObjectGetPropertyDataSize(AudioObjectID(kAudioObjectSystemObject), &addr, 0, nil, &size) == noErr,
          size > 0
    else { return [] }
    var ids = [AudioObjectID](repeating: 0, count: Int(size) / MemoryLayout<AudioObjectID>.size)
    let status = ids.withUnsafeMutableBufferPointer { buf in
        AudioObjectGetPropertyData(AudioObjectID(kAudioObjectSystemObject), &addr, 0, nil, &size, buf.baseAddress!)
    }
    return status == noErr ? ids : []
}

func isCapturing(_ process: AudioObjectID) -> Bool {
    var addr = address(kAudioProcessPropertyIsRunningInput)
    var running: UInt32 = 0
    var size = UInt32(MemoryLayout<UInt32>.size)
    let status = withUnsafeMutablePointer(to: &running) {
        AudioObjectGetPropertyData(process, &addr, 0, nil, &size, $0)
    }
    return status == noErr && running != 0
}

func bundleID(_ process: AudioObjectID) -> String? {
    var addr = address(kAudioProcessPropertyBundleID)
    var value: Unmanaged<CFString>?
    var size = UInt32(MemoryLayout<Unmanaged<CFString>?>.size)
    let status = withUnsafeMutablePointer(to: &value) {
        AudioObjectGetPropertyData(process, &addr, 0, nil, &size, $0)
    }
    // Core Audio returns the string retained (+1).
    guard status == noErr, let id = value?.takeRetainedValue() as String?, !id.isEmpty else { return nil }
    return id
}

let stdout = FileHandle.standardOutput
signal(SIGPIPE, SIG_IGN)

DispatchQueue.global().async {
    while !FileHandle.standardInput.availableData.isEmpty {}
    exit(0)
}

while true {
    let apps = Set(processObjects().filter(isCapturing).compactMap(bundleID)).sorted()
    if let data = try? JSONSerialization.data(withJSONObject: ["apps": apps]) {
        try? stdout.write(contentsOf: data + Data([0x0A]))
    }
    Thread.sleep(forTimeInterval: 1)
}
