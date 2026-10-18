//! Records two tracks at 16 kHz mono: the microphone ("you") and system audio ("them").
//! Each source pushes raw chunks over a channel to a `TrackWriter` thread.

use crate::dsp::{self, Resampler, TARGET_RATE};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

/// Live stats a track shares with the UI meter loop.
#[derive(Default)]
pub struct TrackStats {
    level_bits: AtomicU32,
    samples: AtomicU64,
}

impl TrackStats {
    pub fn level_dbfs(&self) -> f32 {
        let bits = self.level_bits.load(Ordering::Relaxed);
        if bits == 0 {
            -100.0
        } else {
            f32::from_bits(bits)
        }
    }

    pub fn seconds(&self) -> f64 {
        self.samples.load(Ordering::Relaxed) as f64 / TARGET_RATE as f64
    }
}

/// Resamples incoming chunks to 16 kHz and appends them to a 16-bit mono WAV.
pub struct TrackWriter {
    resampler: Resampler,
    wav: hound::WavWriter<std::io::BufWriter<std::fs::File>>,
    stats: Arc<TrackStats>,
}

impl TrackWriter {
    pub fn create(path: &Path, source_rate: u32, stats: Arc<TrackStats>) -> hound::Result<Self> {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: TARGET_RATE,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        Ok(Self {
            resampler: Resampler::new(source_rate, TARGET_RATE),
            wav: hound::WavWriter::create(path, spec)?,
            stats,
        })
    }

    pub fn push(&mut self, chunk: &[f32]) -> hound::Result<()> {
        let out = self.resampler.process(chunk);
        for s in &out {
            self.wav
                .write_sample((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)?;
        }
        self.stats
            .level_bits
            .store(dsp::rms_dbfs(chunk).to_bits(), Ordering::Relaxed);
        self.stats
            .samples
            .fetch_add(out.len() as u64, Ordering::Relaxed);
        Ok(())
    }

    /// Writes `samples` of silence at the 16 kHz output rate, e.g. to line a late-starting
    /// track up with one that started earlier.
    pub fn pad_silence(&mut self, samples: u64) -> hound::Result<()> {
        for _ in 0..samples {
            self.wav.write_sample(0i16)?;
        }
        self.stats.samples.fetch_add(samples, Ordering::Relaxed);
        Ok(())
    }

    pub fn finish(self) -> hound::Result<()> {
        self.wav.finalize()
    }
}

/// Drains `rx` into a WAV until every sender is dropped.
fn spawn_writer(
    path: PathBuf,
    source_rate: u32,
    stats: Arc<TrackStats>,
    lead_in_samples: u64,
    rx: Receiver<Vec<f32>>,
) -> Result<JoinHandle<Result<(), String>>, String> {
    let mut writer =
        TrackWriter::create(&path, source_rate, stats).map_err(|e| format!("{path:?}: {e}"))?;
    writer
        .pad_silence(lead_in_samples)
        .map_err(|e| e.to_string())?;
    Ok(std::thread::spawn(move || {
        for chunk in rx {
            writer.push(&chunk).map_err(|e| e.to_string())?;
        }
        writer.finish().map_err(|e| e.to_string())
    }))
}

/// The microphone stream lives on its own thread because cpal streams aren't `Send`.
struct MicSource {
    stop: Sender<()>,
    thread: JoinHandle<()>,
    writer: JoinHandle<Result<(), String>>,
}

fn start_mic(path: PathBuf, stats: Arc<TrackStats>) -> Result<MicSource, String> {
    let (ready_tx, ready_rx) = mpsc::channel::<Result<u32, String>>();
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let (data_tx, data_rx) = mpsc::channel::<Vec<f32>>();

    let thread = std::thread::spawn(move || {
        let stream = match open_mic_stream(data_tx) {
            Ok((stream, rate)) => {
                let _ = ready_tx.send(Ok(rate));
                stream
            }
            Err(e) => {
                let _ = ready_tx.send(Err(e));
                return;
            }
        };
        let _ = stop_rx.recv();
        drop(stream);
    });

    let rate = ready_rx
        // The first start can sit behind the macOS microphone permission prompt.
        .recv_timeout(Duration::from_secs(60))
        .map_err(|_| {
            "microphone did not start (check System Settings > Privacy > Microphone)".to_string()
        })??;
    let writer = spawn_writer(path, rate, stats, 0, data_rx)?;
    Ok(MicSource {
        stop: stop_tx,
        thread,
        writer,
    })
}

fn open_mic_stream(tx: Sender<Vec<f32>>) -> Result<(cpal::Stream, u32), String> {
    let device = cpal::default_host()
        .default_input_device()
        .ok_or("no microphone found")?;
    let supported = device
        .default_input_config()
        .map_err(|e| format!("microphone config: {e}"))?;
    let channels = supported.channels() as usize;
    let rate = supported.sample_rate();
    let config = supported.config();
    let on_error = |e| eprintln!("mic stream error: {e}");
    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => device.build_input_stream(
            config,
            move |data: &[f32], _: &_| {
                let _ = tx.send(dsp::downmix(data, channels));
            },
            on_error,
            None,
        ),
        cpal::SampleFormat::I16 => device.build_input_stream(
            config,
            move |data: &[i16], _: &_| {
                let f: Vec<f32> = data.iter().map(|s| *s as f32 / i16::MAX as f32).collect();
                let _ = tx.send(dsp::downmix(&f, channels));
            },
            on_error,
            None,
        ),
        other => return Err(format!("unsupported microphone sample format {other:?}")),
    }
    .map_err(|e| format!("could not open microphone: {e}"))?;
    stream
        .play()
        .map_err(|e| format!("could not start microphone: {e}"))?;
    Ok((stream, rate))
}

/// Parses the sidecar's first stderr line into its sample rate.
pub fn parse_ready_line(line: &str) -> Result<u32, String> {
    let v: serde_json::Value =
        serde_json::from_str(line).map_err(|e| format!("bad sidecar output {line:?}: {e}"))?;
    match v["event"].as_str() {
        Some("ready") => v["sampleRate"]
            .as_f64()
            .filter(|r| *r > 0.0)
            .map(|r| r.round() as u32)
            .ok_or_else(|| format!("sidecar ready without sample rate: {line}")),
        Some("error") => Err(v["message"]
            .as_str()
            .unwrap_or("system audio capture failed")
            .to_string()),
        _ => Err(format!("unexpected sidecar output: {line}")),
    }
}

struct SystemSource {
    child: Child,
    stdin: Option<ChildStdin>,
    reader: JoinHandle<()>,
    writer: JoinHandle<Result<(), String>>,
}

/// `align_to` is the track already running (the mic); the system track is padded with
/// silence for however long that track recorded before the sidecar became ready.
fn start_system(
    sidecar: &Path,
    path: PathBuf,
    stats: Arc<TrackStats>,
    align_to: &TrackStats,
) -> Result<SystemSource, String> {
    let mut child = Command::new(sidecar)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("could not start {sidecar:?}: {e}"))?;
    let stdin = child.stdin.take();
    let mut stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");

    let (line_tx, line_rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            if line_tx.send(line.clone()).is_err() {
                eprintln!("audiotap: {line}");
            }
        }
    });
    let rate = match line_rx.recv_timeout(Duration::from_secs(10)) {
        Ok(line) => parse_ready_line(&line),
        Err(_) => Err("system audio capture did not start".into()),
    };
    let rate = match rate {
        Ok(r) => r,
        Err(e) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(e);
        }
    };

    let (data_tx, data_rx) = mpsc::channel::<Vec<f32>>();
    let reader = std::thread::spawn(move || {
        let mut buf = vec![0u8; 16 * 1024];
        let mut carry: Vec<u8> = Vec::new();
        while let Ok(n) = stdout.read(&mut buf) {
            if n == 0 {
                break;
            }
            carry.extend_from_slice(&buf[..n]);
            let whole = carry.len() / 4 * 4;
            let samples = carry[..whole]
                .as_chunks::<4>()
                .0
                .iter()
                .map(|b| f32::from_le_bytes(*b))
                .collect();
            carry.drain(..whole);
            if data_tx.send(samples).is_err() {
                break;
            }
        }
    });
    let lead_in = align_to.samples.load(Ordering::Relaxed);
    let writer = spawn_writer(path, rate, stats, lead_in, data_rx)?;
    Ok(SystemSource {
        child,
        stdin,
        reader,
        writer,
    })
}

pub struct Recording {
    pub dir: PathBuf,
    pub mic_stats: Arc<TrackStats>,
    pub system_stats: Arc<TrackStats>,
    pub system_warning: Option<String>,
    mic: MicSource,
    system: Option<SystemSource>,
}

pub struct Finished {
    pub mic_path: PathBuf,
    pub system_path: Option<PathBuf>,
    pub duration_secs: f64,
}

impl Recording {
    /// Starts both tracks. A system-audio failure (old macOS, permission denied) degrades
    /// to mic-only with a warning; a mic failure aborts.
    pub fn start(dir: PathBuf, sidecar: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(&dir).map_err(|e| format!("{dir:?}: {e}"))?;
        let mic_stats = Arc::new(TrackStats::default());
        let system_stats = Arc::new(TrackStats::default());
        let mic = start_mic(dir.join("mic.wav"), mic_stats.clone())?;
        let (system, system_warning) = match start_system(
            sidecar,
            dir.join("system.wav"),
            system_stats.clone(),
            &mic_stats,
        ) {
            Ok(s) => (Some(s), None),
            Err(e) => (None, Some(e)),
        };
        Ok(Self {
            dir,
            mic_stats,
            system_stats,
            system_warning,
            mic,
            system,
        })
    }

    pub fn stop(self) -> Result<Finished, String> {
        let _ = self.mic.stop.send(());
        let _ = self.mic.thread.join();
        self.mic
            .writer
            .join()
            .map_err(|_| "mic writer panicked")??;

        let system_path = match self.system {
            Some(mut s) => {
                // Closing stdin asks the sidecar to tear down its tap and exit.
                if let Some(mut stdin) = s.stdin.take() {
                    let _ = stdin.flush();
                }
                let exited = wait_with_timeout(&mut s.child, Duration::from_secs(3));
                if !exited {
                    let _ = s.child.kill();
                    let _ = s.child.wait();
                }
                let _ = s.reader.join();
                s.writer.join().map_err(|_| "system writer panicked")??;
                Some(self.dir.join("system.wav"))
            }
            None => None,
        };

        Ok(Finished {
            mic_path: self.dir.join("mic.wav"),
            system_path,
            duration_secs: self.mic_stats.seconds().max(self.system_stats.seconds()),
        })
    }
}

fn wait_with_timeout(child: &mut Child, timeout: Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if let Ok(Some(_)) = child.try_wait() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn track_writer_resamples_to_16k_wav_and_tracks_stats() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.wav");
        let stats = Arc::new(TrackStats::default());
        let mut w = TrackWriter::create(&path, 48_000, stats.clone()).unwrap();
        let tone: Vec<f32> = (0..48_000)
            .map(|i| (i as f32 * 440.0 * std::f32::consts::TAU / 48_000.0).sin() * 0.5)
            .collect();
        for chunk in tone.chunks(480) {
            w.push(chunk).unwrap();
        }
        w.finish().unwrap();

        let reader = hound::WavReader::open(&path).unwrap();
        assert_eq!(reader.spec().sample_rate, 16_000);
        assert_eq!(reader.spec().channels, 1);
        assert!((reader.len() as i64 - 16_000).abs() <= 2);
        assert!((stats.seconds() - 1.0).abs() < 0.01);
        // A 0.5-amplitude sine sits near -9 dBFS.
        assert!(
            (stats.level_dbfs() + 9.03).abs() < 0.5,
            "{}",
            stats.level_dbfs()
        );
    }

    #[test]
    fn pad_silence_prepends_zeros_and_counts_them() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("p.wav");
        let stats = Arc::new(TrackStats::default());
        let mut w = TrackWriter::create(&path, 16_000, stats.clone()).unwrap();
        w.pad_silence(8_000).unwrap();
        w.push(&vec![0.5; 1_601]).unwrap();
        w.finish().unwrap();

        let samples: Vec<i16> = hound::WavReader::open(&path)
            .unwrap()
            .into_samples()
            .map(Result::unwrap)
            .collect();
        assert_eq!(samples.len(), 8_000 + 1_600);
        assert!(samples[..8_000].iter().all(|s| *s == 0));
        assert!(samples[8_000..].iter().all(|s| *s > 16_000));
        assert!((stats.seconds() - 0.6).abs() < 1e-9);
    }

    #[test]
    fn fresh_stats_read_as_silence() {
        let s = TrackStats::default();
        assert_eq!(s.level_dbfs(), -100.0);
        assert_eq!(s.seconds(), 0.0);
    }

    #[test]
    fn ready_line_gives_sample_rate() {
        assert_eq!(
            parse_ready_line(r#"{"event":"ready","sampleRate":48000,"sourceChannels":2}"#),
            Ok(48_000)
        );
    }

    #[test]
    fn error_line_surfaces_the_sidecar_message() {
        let e = parse_ready_line(
            r#"{"event":"error","message":"could not create process tap","status":-1}"#,
        );
        assert_eq!(e, Err("could not create process tap".to_string()));
    }

    #[test]
    fn junk_lines_are_rejected() {
        assert!(parse_ready_line("hello").is_err());
        assert!(parse_ready_line(r#"{"event":"ready"}"#).is_err());
    }
}
