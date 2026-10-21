// parley-mic: captures the default microphone through Apple's voice-processing I/O, which
// cancels the Mac's own speaker output (the call audio) out of the mic signal. Without it, a
// meeting on laptop speakers is transcribed twice: once from system audio, once from the mic.
//
// Same protocol as parley-audiotap:
//   stderr: JSON lines, first {"event":"ready","sampleRate":N} or {"event":"error",...}
//   stdout: raw mono f32le samples at sampleRate
//   stdin:  close it (or send SIGTERM) to stop

import AVFoundation
import Foundation

func emit(_ fields: [String: Any]) {
    guard let data = try? JSONSerialization.data(withJSONObject: fields) else { return }
    FileHandle.standardError.write(data + Data([0x0A]))
}

func fail(_ message: String) -> Never {
    emit(["event": "error", "message": message])
    exit(1)
}

let engine = AVAudioEngine()
let input = engine.inputNode

do {
    try input.setVoiceProcessingEnabled(true)
} catch {
    fail("could not enable echo cancellation: \(error.localizedDescription)")
}
// Voice processing normally turns other apps down while the mic is live. Parley records the
// call through system audio, so keep other audio at full volume.
input.voiceProcessingOtherAudioDuckingConfiguration = AVAudioVoiceProcessingOtherAudioDuckingConfiguration(
    enableAdvancedDucking: false, duckingLevel: .min)

let format = input.outputFormat(forBus: 0)
guard format.sampleRate > 0, format.channelCount > 0 else { fail("no microphone input available") }

let writeQueue = DispatchQueue(label: "parley.mic.write")
let stdout = FileHandle.standardOutput

input.installTap(onBus: 0, bufferSize: 1024, format: format) { buffer, _ in
    guard let channels = buffer.floatChannelData else { return }
    let frames = Int(buffer.frameLength)
    let count = Int(buffer.format.channelCount)
    let stride = buffer.stride
    var mono = [Float](repeating: 0, count: frames)
    for c in 0..<count {
        let samples = channels[c]
        for i in 0..<frames { mono[i] += samples[i * stride] }
    }
    if count > 1 {
        let scale = 1 / Float(count)
        for i in 0..<frames { mono[i] *= scale }
    }
    let bytes = mono.withUnsafeBufferPointer { Data(buffer: $0) }
    writeQueue.async { try? stdout.write(contentsOf: bytes) }
}

engine.prepare()
do {
    try engine.start()
} catch {
    fail("could not start microphone: \(error.localizedDescription)")
}

emit(["event": "ready", "sampleRate": format.sampleRate, "echoCancellation": true])

func shutdown() -> Never {
    input.removeTap(onBus: 0)
    engine.stop()
    writeQueue.sync {}
    exit(0)
}

signal(SIGTERM, SIG_IGN)
signal(SIGINT, SIG_IGN)
signal(SIGPIPE, SIG_IGN)
let signalSources = [SIGTERM, SIGINT].map { sig -> DispatchSourceSignal in
    let source = DispatchSource.makeSignalSource(signal: sig, queue: .main)
    source.setEventHandler { shutdown() }
    source.resume()
    return source
}

DispatchQueue.global().async {
    while !FileHandle.standardInput.availableData.isEmpty {}
    DispatchQueue.main.async { shutdown() }
}

withExtendedLifetime(signalSources) { dispatchMain() }
