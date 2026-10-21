//! Splits a 16 kHz track into utterances for engines that transcribe whole chunks (whisper.cpp).
//! An adaptive energy VAD finds speech; while someone is talking we hand out the growing
//! utterance every `partial_every` for a live partial, and close it after a pause.

use crate::dsp::TARGET_RATE;

const FRAME: usize = (TARGET_RATE as usize) * 30 / 1000; // 30 ms

#[derive(Debug, Clone)]
pub struct SegmenterConfig {
    pub partial_every_ms: u64,
    pub end_silence_ms: u64,
    pub max_utterance_ms: u64,
    pub pre_roll_ms: u64,
    /// Utterances with less voiced audio than this are dropped as noise blips.
    pub min_voiced_ms: u64,
}

impl Default for SegmenterConfig {
    fn default() -> Self {
        Self {
            partial_every_ms: 1_500,
            end_silence_ms: 700,
            max_utterance_ms: 20_000,
            pre_roll_ms: 300,
            min_voiced_ms: 250,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Cut {
    /// Speech so far in an open utterance; a later Partial or Final with the same id replaces it.
    Partial {
        id: u64,
        start_ms: u64,
        audio: Vec<f32>,
    },
    Final {
        id: u64,
        start_ms: u64,
        audio: Vec<f32>,
    },
}

pub struct Segmenter {
    cfg: SegmenterConfig,
    floor_db: f32,
    pending: Vec<f32>,
    /// Samples of the open utterance (or pre-roll while idle).
    buf: Vec<f32>,
    /// Absolute sample index of `buf[0]`.
    buf_start: u64,
    total: u64,
    in_speech: bool,
    voiced_frames: u64,
    trailing_silence: u64,
    last_voiced_end: usize,
    since_partial: usize,
    partial_sent: bool,
    next_id: u64,
}

fn ms_to_samples(ms: u64) -> usize {
    (ms * TARGET_RATE as u64 / 1000) as usize
}

fn frame_db(frame: &[f32]) -> f32 {
    crate::dsp::rms_dbfs(frame)
}

impl Segmenter {
    pub fn new(cfg: SegmenterConfig) -> Self {
        Self {
            cfg,
            floor_db: -60.0,
            pending: Vec::new(),
            buf: Vec::new(),
            buf_start: 0,
            total: 0,
            in_speech: false,
            voiced_frames: 0,
            trailing_silence: 0,
            last_voiced_end: 0,
            since_partial: 0,
            partial_sent: false,
            next_id: 0,
        }
    }

    pub fn push(&mut self, samples: &[f32]) -> Vec<Cut> {
        self.pending.extend_from_slice(samples);
        let mut cuts = Vec::new();
        let whole = self.pending.len() / FRAME * FRAME;
        let frames: Vec<f32> = self.pending.drain(..whole).collect();
        for frame in frames.as_chunks::<FRAME>().0 {
            self.on_frame(frame, &mut cuts);
        }
        cuts
    }

    /// Closes any open utterance at the end of a recording.
    pub fn finish(&mut self) -> Vec<Cut> {
        let rest = std::mem::take(&mut self.pending);
        self.buf.extend_from_slice(&rest);
        self.total += rest.len() as u64;
        let mut cuts = Vec::new();
        if self.in_speech {
            self.close(&mut cuts);
        }
        cuts
    }

    fn is_voiced(&mut self, db: f32) -> bool {
        let voiced = db > (self.floor_db + 12.0).max(-50.0);
        // The floor follows quiet frames down quickly and creeps up slowly, so a steady
        // fan or hum stops counting as speech after a few seconds.
        if db < self.floor_db {
            self.floor_db = 0.7 * self.floor_db + 0.3 * db;
        } else if !voiced {
            self.floor_db += 0.05 * (db - self.floor_db);
        }
        voiced
    }

    fn on_frame(&mut self, frame: &[f32], cuts: &mut Vec<Cut>) {
        let voiced = self.is_voiced(frame_db(frame));
        self.buf.extend_from_slice(frame);
        self.total += FRAME as u64;

        if !self.in_speech {
            if voiced {
                self.in_speech = true;
                self.voiced_frames = 1;
                self.trailing_silence = 0;
                self.last_voiced_end = self.buf.len();
                self.since_partial = 0;
                self.partial_sent = false;
            } else {
                let keep = ms_to_samples(self.cfg.pre_roll_ms);
                if self.buf.len() > keep {
                    let drop = self.buf.len() - keep;
                    self.buf.drain(..drop);
                    self.buf_start += drop as u64;
                }
            }
            return;
        }

        if voiced {
            self.voiced_frames += 1;
            self.trailing_silence = 0;
            self.last_voiced_end = self.buf.len();
        } else {
            self.trailing_silence += 1;
        }
        self.since_partial += FRAME;

        let silence_ms = self.trailing_silence * 30;
        let length_ms = self.buf.len() as u64 * 1000 / TARGET_RATE as u64;
        let next_frame_overflows = length_ms + 30 > self.cfg.max_utterance_ms;
        if silence_ms >= self.cfg.end_silence_ms || next_frame_overflows {
            self.close(cuts);
        } else if self.since_partial >= ms_to_samples(self.cfg.partial_every_ms)
            && self.voiced_ms() >= self.cfg.min_voiced_ms
        {
            self.since_partial = 0;
            self.partial_sent = true;
            cuts.push(Cut::Partial {
                id: self.next_id,
                start_ms: self.start_ms(),
                audio: self.buf[..self.last_voiced_end].to_vec(),
            });
        }
    }

    fn voiced_ms(&self) -> u64 {
        self.voiced_frames * 30
    }

    fn start_ms(&self) -> u64 {
        self.buf_start * 1000 / TARGET_RATE as u64
    }

    fn close(&mut self, cuts: &mut Vec<Cut>) {
        // A partial already on screen must always get its final, even if short.
        if self.partial_sent || self.voiced_ms() >= self.cfg.min_voiced_ms {
            let end = (self.last_voiced_end + ms_to_samples(200)).min(self.buf.len());
            cuts.push(Cut::Final {
                id: self.next_id,
                start_ms: self.start_ms(),
                audio: self.buf[..end].to_vec(),
            });
            self.next_id += 1;
        }
        self.in_speech = false;
        self.voiced_frames = 0;
        self.trailing_silence = 0;
        self.partial_sent = false;
        self.buf_start = self.total;
        self.buf.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn silence(ms: u64) -> Vec<f32> {
        vec![0.0005; ms_to_samples(ms)]
    }

    fn tone(ms: u64) -> Vec<f32> {
        (0..ms_to_samples(ms))
            .map(|i| (i as f32 * 0.2).sin() * 0.3)
            .collect()
    }

    fn run(input: &[f32]) -> Vec<Cut> {
        let mut s = Segmenter::new(SegmenterConfig::default());
        let mut cuts = Vec::new();
        for chunk in input.chunks(1_000) {
            cuts.extend(s.push(chunk));
        }
        cuts.extend(s.finish());
        cuts
    }

    fn finals(cuts: &[Cut]) -> Vec<(u64, u64, usize)> {
        cuts.iter()
            .filter_map(|c| match c {
                Cut::Final {
                    id,
                    start_ms,
                    audio,
                } => Some((*id, *start_ms, audio.len())),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn silence_produces_nothing() {
        assert!(run(&silence(5_000)).is_empty());
    }

    #[test]
    fn two_utterances_separated_by_a_pause_become_two_finals_with_times() {
        let mut input = silence(1_000);
        input.extend(tone(1_000));
        input.extend(silence(1_500));
        input.extend(tone(800));
        input.extend(silence(1_000));
        let f = finals(&run(&input));
        assert_eq!(f.len(), 2, "{f:?}");
        assert_eq!(f[0].0, 0);
        assert_eq!(f[1].0, 1);
        // Starts include the 300 ms pre-roll.
        assert!((690..=720).contains(&f[0].1), "start {}", f[0].1);
        assert!((3_180..=3_220).contains(&f[1].1), "start {}", f[1].1);
        // Audio is the speech plus pre-roll and a 200 ms tail, not the whole pause.
        let secs = f[0].2 as f64 / 16_000.0;
        assert!((1.4..1.6).contains(&secs), "len {secs}");
    }

    #[test]
    fn long_speech_emits_growing_partials_before_its_final() {
        let mut input = silence(500);
        input.extend(tone(5_000));
        input.extend(silence(1_000));
        let cuts = run(&input);
        let partials: Vec<usize> = cuts
            .iter()
            .filter_map(|c| match c {
                Cut::Partial { id: 0, audio, .. } => Some(audio.len()),
                _ => None,
            })
            .collect();
        assert!(partials.len() >= 3, "{partials:?}");
        assert!(partials.windows(2).all(|w| w[1] > w[0]));
        assert!(matches!(cuts.last(), Some(Cut::Final { id: 0, .. })));
    }

    #[test]
    fn utterances_are_capped_at_the_max_length() {
        let mut input = tone(45_000);
        input.extend(silence(1_000));
        let f = finals(&run(&input));
        assert_eq!(f.len(), 3, "{f:?}");
        assert!(f.iter().all(|(_, _, n)| *n <= ms_to_samples(20_000)));
    }

    #[test]
    fn short_clicks_are_dropped() {
        let mut input = silence(1_000);
        input.extend(tone(90));
        input.extend(silence(1_500));
        assert!(finals(&run(&input)).is_empty());
    }

    #[test]
    fn steady_background_hum_stops_counting_as_speech() {
        let hum: Vec<f32> = (0..ms_to_samples(30_000))
            .map(|i| (i as f32 * 0.05).sin() * 0.003)
            .collect();
        let f = finals(&run(&hum));
        assert!(f.len() <= 1, "hum produced {} utterances", f.len());
    }

    #[test]
    fn open_utterance_is_closed_by_finish() {
        let mut input = silence(500);
        input.extend(tone(1_000));
        assert_eq!(finals(&run(&input)).len(), 1);
    }
}
