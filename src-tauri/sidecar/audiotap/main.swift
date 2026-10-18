// parley-audiotap: captures all system audio output through a Core Audio process tap
// (macOS 14.2+) and writes mono 32-bit float little-endian PCM to stdout.
//
// Protocol:
//   stderr: one JSON object per line. First line is {"event":"ready","sampleRate":N}
//           or {"event":"error",...}.
//   stdout: raw mono f32le samples at sampleRate.
//   stdin:  close it (or send SIGTERM) to stop. The tap and aggregate device are torn down on exit.

import AudioToolbox
import CoreAudio
import Foundation

func emit(_ fields: [String: Any]) {
    guard let data = try? JSONSerialization.data(withJSONObject: fields) else { return }
    FileHandle.standardError.write(data + Data([0x0A]))
}

func fail(_ message: String, _ status: OSStatus = 0) -> Never {
    emit(["event": "error", "message": message, "status": Int(status)])
    exit(1)
}

func address(_ selector: AudioObjectPropertySelector) -> AudioObjectPropertyAddress {
    AudioObjectPropertyAddress(
        mSelector: selector, mScope: kAudioObjectPropertyScopeGlobal, mElement: kAudioObjectPropertyElementMain)
}

func getProperty<T: BitwiseCopyable>(_ object: AudioObjectID, _ selector: AudioObjectPropertySelector, _ value: inout T) -> OSStatus {
    var addr = address(selector)
    var size = UInt32(MemoryLayout<T>.size)
    return withUnsafeMutablePointer(to: &value) { AudioObjectGetPropertyData(object, &addr, 0, nil, &size, $0) }
}

func defaultOutputDeviceUID() -> String {
    var device = AudioObjectID(kAudioObjectUnknown)
    var status = getProperty(AudioObjectID(kAudioObjectSystemObject), kAudioHardwarePropertyDefaultOutputDevice, &device)
    guard status == noErr else { fail("no default output device", status) }
    // Core Audio returns the UID string retained (+1), so take ownership of it.
    var uid: Unmanaged<CFString>?
    var addr = address(kAudioDevicePropertyDeviceUID)
    var size = UInt32(MemoryLayout<Unmanaged<CFString>?>.size)
    status = withUnsafeMutablePointer(to: &uid) { AudioObjectGetPropertyData(device, &addr, 0, nil, &size, $0) }
    guard status == noErr, let uid else { fail("could not read output device UID", status) }
    return uid.takeRetainedValue() as String
}

// Tap every process's output, mixed to stereo, without muting what the user hears.
let tapDescription = CATapDescription(stereoGlobalTapButExcludeProcesses: [])
tapDescription.uuid = UUID()
tapDescription.isPrivate = true
tapDescription.muteBehavior = .unmuted
tapDescription.name = "Parley System Audio"

var tapID = AudioObjectID(kAudioObjectUnknown)
var status = AudioHardwareCreateProcessTap(tapDescription, &tapID)
guard status == noErr else { fail("could not create process tap (is system audio recording allowed?)", status) }

var format = AudioStreamBasicDescription()
status = getProperty(tapID, kAudioTapPropertyFormat, &format)
guard status == noErr else { fail("could not read tap format", status) }
guard format.mFormatID == kAudioFormatLinearPCM, format.mFormatFlags & kAudioFormatFlagIsFloat != 0,
      format.mBitsPerChannel == 32
else { fail("unsupported tap format") }
let channels = Int(format.mChannelsPerFrame)
let nonInterleaved = format.mFormatFlags & kAudioFormatFlagIsNonInterleaved != 0

let outputUID = defaultOutputDeviceUID()
let aggregateDescription: [String: Any] = [
    kAudioAggregateDeviceNameKey: "Parley Tap",
    kAudioAggregateDeviceUIDKey: UUID().uuidString,
    kAudioAggregateDeviceMainSubDeviceKey: outputUID,
    kAudioAggregateDeviceIsPrivateKey: true,
    kAudioAggregateDeviceIsStackedKey: false,
    kAudioAggregateDeviceTapAutoStartKey: true,
    kAudioAggregateDeviceSubDeviceListKey: [[kAudioSubDeviceUIDKey: outputUID]],
    kAudioAggregateDeviceTapListKey: [
        [kAudioSubTapDriftCompensationKey: true, kAudioSubTapUIDKey: tapDescription.uuid.uuidString]
    ],
]

var aggregateID = AudioObjectID(kAudioObjectUnknown)
status = AudioHardwareCreateAggregateDevice(aggregateDescription as CFDictionary, &aggregateID)
guard status == noErr else {
    AudioHardwareDestroyProcessTap(tapID)
    fail("could not create aggregate device", status)
}

// Pipe writes can block; keep them off the real-time audio thread.
let writeQueue = DispatchQueue(label: "parley.audiotap.write")
let stdout = FileHandle.standardOutput
let sampleRate = format.mSampleRate

// Core Audio only runs the IO proc while something is playing, so silent stretches arrive as
// gaps. Track the output position against the host clock and write zeros for any gap; this
// keeps the system track aligned in time with the microphone track.
var timebase = mach_timebase_info_data_t()
mach_timebase_info(&timebase)
func hostSeconds(_ ticks: UInt64) -> Double {
    Double(ticks) * Double(timebase.numer) / Double(timebase.denom) / 1_000_000_000
}
var startHostTime: UInt64 = 0
var framesWritten = 0  // only touched on writeQueue
let gapToleranceFrames = Int(sampleRate * 0.05)

func writeSilence(upTo hostTime: UInt64) {
    let expected = Int(hostSeconds(hostTime &- startHostTime) * sampleRate)
    let missing = expected - framesWritten
    guard missing > gapToleranceFrames else { return }
    let zeros = [Float](repeating: 0, count: missing)
    try? stdout.write(contentsOf: zeros.withUnsafeBufferPointer { Data(buffer: $0) })
    framesWritten += missing
}

var procID: AudioDeviceIOProcID?
status = AudioDeviceCreateIOProcIDWithBlock(&procID, aggregateID, nil) { _, inputData, inputTime, _, _ in
    let hasHostTime = inputTime.pointee.mFlags.contains(.hostTimeValid)
    let hostTime = hasHostTime ? inputTime.pointee.mHostTime : mach_absolute_time()
    let buffers = UnsafeMutableAudioBufferListPointer(UnsafeMutablePointer(mutating: inputData))
    guard let first = buffers.first, let firstData = first.mData else { return }
    var mono: [Float]
    if nonInterleaved {
        let frames = Int(first.mDataByteSize) / MemoryLayout<Float>.size
        mono = [Float](repeating: 0, count: frames)
        for buffer in buffers {
            guard let data = buffer.mData else { continue }
            let samples = data.assumingMemoryBound(to: Float.self)
            for i in 0..<frames { mono[i] += samples[i] }
        }
        let scale = 1 / Float(max(buffers.count, 1))
        for i in 0..<frames { mono[i] *= scale }
    } else {
        let total = Int(first.mDataByteSize) / MemoryLayout<Float>.size
        let frames = total / max(channels, 1)
        let samples = firstData.assumingMemoryBound(to: Float.self)
        mono = [Float](repeating: 0, count: frames)
        for f in 0..<frames {
            var sum: Float = 0
            for c in 0..<channels { sum += samples[f * channels + c] }
            mono[f] = sum / Float(channels)
        }
    }
    let bytes = mono.withUnsafeBufferPointer { Data(buffer: $0) }
    let frames = mono.count
    writeQueue.async {
        writeSilence(upTo: hostTime)
        try? stdout.write(contentsOf: bytes)
        framesWritten += frames
    }
}
guard status == noErr, let procID else { fail("could not create IO proc", status) }

func shutdown(_ code: Int32) -> Never {
    AudioDeviceStop(aggregateID, procID)
    AudioDeviceDestroyIOProcID(aggregateID, procID)
    AudioHardwareDestroyAggregateDevice(aggregateID)
    AudioHardwareDestroyProcessTap(tapID)
    let now = mach_absolute_time()
    writeQueue.sync { writeSilence(upTo: now) }
    exit(code)
}

startHostTime = mach_absolute_time()
status = AudioDeviceStart(aggregateID, procID)
guard status == noErr else { fail("could not start capture", status) }

emit(["event": "ready", "sampleRate": format.mSampleRate, "sourceChannels": channels])

signal(SIGTERM, SIG_IGN)
signal(SIGINT, SIG_IGN)
signal(SIGPIPE, SIG_IGN)
let signalSources = [SIGTERM, SIGINT].map { sig -> DispatchSourceSignal in
    let source = DispatchSource.makeSignalSource(signal: sig, queue: .main)
    source.setEventHandler { shutdown(0) }
    source.resume()
    return source
}

DispatchQueue.global().async {
    while !FileHandle.standardInput.availableData.isEmpty {}
    DispatchQueue.main.async { shutdown(0) }
}

withExtendedLifetime(signalSources) { dispatchMain() }
