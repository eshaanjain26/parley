//! Audio helpers shared by every capture source. Everything here is pure and streaming:
//! feeding a signal in any chunk sizes gives the same output as feeding it at once.

use std::f64::consts::PI;

pub const TARGET_RATE: u32 = 16_000;

pub fn downmix(interleaved: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return interleaved.to_vec();
    }
    interleaved
        .chunks_exact(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect()
}

/// Root-mean-square level in dBFS, floored at -100.
pub fn rms_dbfs(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return -100.0;
    }
    let mean_sq = samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32;
    (10.0 * mean_sq.max(1e-10).log10()).max(-100.0)
}

/// Streaming sample-rate converter: windowed-sinc low-pass (to stop aliasing when
/// downsampling) followed by linear interpolation.
pub struct Resampler {
    step: f64,
    taps: Vec<f32>,
    history: Vec<f32>,
    // Positions come from integer counters, never from a running float, so chunk
    // boundaries can't drift and change which samples get picked.
    produced: u64,
    consumed: u64,
    prev: Option<f32>,
}

impl Resampler {
    pub fn new(in_rate: u32, out_rate: u32) -> Self {
        let step = in_rate as f64 / out_rate as f64;
        let taps = if in_rate > out_rate {
            lowpass_taps(0.45 * out_rate as f64 / in_rate as f64, 63)
        } else {
            vec![1.0]
        };
        Self {
            step,
            history: vec![0.0; taps.len() - 1],
            taps,
            produced: 0,
            consumed: 0,
            prev: None,
        }
    }

    pub fn process(&mut self, input: &[f32]) -> Vec<f32> {
        let filtered = self.filter(input);
        let len = filtered.len() as i64;
        let mut out = Vec::with_capacity((len as f64 / self.step) as usize + 1);
        loop {
            // Absolute position minus samples already consumed; index -1 is `prev`.
            let pos = self.produced as f64 * self.step - self.consumed as f64;
            let i = pos.floor() as i64;
            if i + 1 >= len {
                break;
            }
            let a = if i < 0 {
                self.prev
                    .expect("position before chunk start only happens after a chunk")
            } else {
                filtered[i as usize]
            };
            let b = filtered[(i + 1) as usize];
            out.push(a + (b - a) * (pos - i as f64) as f32);
            self.produced += 1;
        }
        if let Some(&last) = filtered.last() {
            self.prev = Some(last);
            self.consumed += len as u64;
        }
        out
    }

    fn filter(&mut self, input: &[f32]) -> Vec<f32> {
        if self.taps.len() == 1 {
            return input.to_vec();
        }
        let mut buf = std::mem::take(&mut self.history);
        buf.extend_from_slice(input);
        let n = self.taps.len();
        let out = buf
            .windows(n)
            .map(|w| w.iter().zip(&self.taps).map(|(x, t)| x * t).sum())
            .collect();
        self.history = buf[buf.len() - (n - 1)..].to_vec();
        out
    }
}

/// Windowed-sinc low-pass FIR. `cutoff` is a fraction of the input sample rate.
fn lowpass_taps(cutoff: f64, count: usize) -> Vec<f32> {
    let mid = (count - 1) as f64 / 2.0;
    let raw: Vec<f64> = (0..count)
        .map(|i| {
            let x = i as f64 - mid;
            let sinc = if x == 0.0 {
                2.0 * cutoff
            } else {
                (2.0 * PI * cutoff * x).sin() / (PI * x)
            };
            let blackman = 0.42 - 0.5 * (2.0 * PI * i as f64 / (count - 1) as f64).cos()
                + 0.08 * (4.0 * PI * i as f64 / (count - 1) as f64).cos();
            sinc * blackman
        })
        .collect();
    let gain: f64 = raw.iter().sum();
    raw.iter().map(|t| (t / gain) as f32).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(freq: f64, rate: u32, secs: f64) -> Vec<f32> {
        let n = (rate as f64 * secs) as usize;
        (0..n)
            .map(|i| (2.0 * PI * freq * i as f64 / rate as f64).sin() as f32 * 0.5)
            .collect()
    }

    fn zero_crossings(s: &[f32]) -> usize {
        s.windows(2)
            .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
            .count()
    }

    #[test]
    fn downmix_averages_channels() {
        assert_eq!(downmix(&[1.0, 0.0, 0.5, 0.5], 2), vec![0.5, 0.5]);
        assert_eq!(downmix(&[0.3, 0.6], 1), vec![0.3, 0.6]);
    }

    #[test]
    fn rms_of_full_scale_square_is_zero_db_and_silence_is_floored() {
        assert!((rms_dbfs(&[1.0, -1.0, 1.0, -1.0])).abs() < 1e-4);
        assert_eq!(rms_dbfs(&[0.0; 64]), -100.0);
        assert_eq!(rms_dbfs(&[]), -100.0);
    }

    #[test]
    fn downsampling_48k_keeps_length_and_pitch() {
        let out = Resampler::new(48_000, TARGET_RATE).process(&sine(440.0, 48_000, 1.0));
        assert!((out.len() as i64 - 16_000).abs() <= 2, "len {}", out.len());
        // A 440 Hz tone crosses zero about 880 times per second.
        let zc = zero_crossings(&out[200..]);
        assert!((850..=890).contains(&zc), "zero crossings {zc}");
    }

    #[test]
    fn tones_above_new_nyquist_are_filtered_out() {
        let out = Resampler::new(48_000, TARGET_RATE).process(&sine(11_000.0, 48_000, 0.5));
        assert!(
            rms_dbfs(&out[100..]) < -40.0,
            "level {}",
            rms_dbfs(&out[100..])
        );
    }

    #[test]
    fn chunked_processing_matches_one_shot() {
        let input = sine(523.0, 44_100, 0.4);
        let whole = Resampler::new(44_100, TARGET_RATE).process(&input);
        let mut r = Resampler::new(44_100, TARGET_RATE);
        let mut chunked = Vec::new();
        for chunk in input.chunks(37).chain(std::iter::once(&[][..])) {
            chunked.extend(r.process(chunk));
        }
        assert_eq!(whole.len(), chunked.len());
        for (a, b) in whole.iter().zip(&chunked) {
            assert!((a - b).abs() < 1e-5);
        }
    }

    #[test]
    fn same_rate_passes_samples_through() {
        let input: Vec<f32> = (0..100).map(|i| i as f32 / 100.0).collect();
        let out = Resampler::new(TARGET_RATE, TARGET_RATE).process(&input);
        assert_eq!(out.len(), 99);
        assert_eq!(out[..], input[..99]);
    }
}
