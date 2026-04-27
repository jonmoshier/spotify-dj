use rustfft::{FftPlanner, num_complex::Complex};
use std::sync::mpsc::Receiver;
use tokio::sync::watch;

const SAMPLE_RATE: f32 = 44100.0;
const FFT_SIZE: usize = 2048; // ~46 ms window
const HOP_SIZE: usize = FFT_SIZE / 2; // 50% overlap → ~43 fps
pub const BAND_COUNT: usize = 20;
const FREQ_MIN: f32 = 40.0;
const FREQ_MAX: f32 = 16_000.0;
// dB range mapped onto the 0..1 visualization scale.
const DB_FLOOR: f32 = -60.0;
const DB_CEIL: f32 = -10.0;

pub struct FftAnalyzer {
    rx: Receiver<Vec<f64>>,
    bands_tx: watch::Sender<Vec<f32>>,
}

impl FftAnalyzer {
    pub fn new(rx: Receiver<Vec<f64>>, bands_tx: watch::Sender<Vec<f32>>) -> Self {
        Self { rx, bands_tx }
    }

    pub fn run(self) {
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);
        let window = hann_window(FFT_SIZE);
        let band_edges = log_band_edges();
        let mut buffer: Vec<f32> = Vec::with_capacity(FFT_SIZE * 2);
        let mut scratch: Vec<Complex<f32>> = vec![Complex::new(0.0, 0.0); FFT_SIZE];

        // recv() returns Err once all senders are dropped (player shutdown).
        while let Ok(chunk) = self.rx.recv() {
            // Interleaved stereo f64 → mono f32
            for pair in chunk.chunks(2) {
                let mono = match pair {
                    [l, r] => ((*l + *r) * 0.5) as f32,
                    [l] => *l as f32,
                    _ => continue,
                };
                buffer.push(mono);
            }

            while buffer.len() >= FFT_SIZE {
                for (i, sample) in buffer[..FFT_SIZE].iter().enumerate() {
                    scratch[i] = Complex::new(sample * window[i], 0.0);
                }
                fft.process(&mut scratch);
                let bands = compute_bands(&scratch, &band_edges);
                let _ = self.bands_tx.send(bands);
                buffer.drain(..HOP_SIZE);
            }
        }
    }
}

fn hann_window(size: usize) -> Vec<f32> {
    (0..size)
        .map(|i| {
            let x = i as f32 / (size - 1) as f32;
            0.5 * (1.0 - (2.0 * std::f32::consts::PI * x).cos())
        })
        .collect()
}

/// Logarithmically-spaced band edges (Hz). Length is BAND_COUNT + 1.
fn log_band_edges() -> Vec<f32> {
    let log_min = FREQ_MIN.ln();
    let log_max = FREQ_MAX.ln();
    (0..=BAND_COUNT)
        .map(|i| {
            let t = i as f32 / BAND_COUNT as f32;
            (log_min + t * (log_max - log_min)).exp()
        })
        .collect()
}

fn compute_bands(spectrum: &[Complex<f32>], edges: &[f32]) -> Vec<f32> {
    let bin_hz = SAMPLE_RATE / FFT_SIZE as f32;
    let nyquist_bin = FFT_SIZE / 2;
    // Hann window coherent gain ≈ 0.5; scaling by FFT_SIZE/2 normalizes a full-scale
    // sinusoid to magnitude 1.0.
    let scale = (FFT_SIZE as f32) * 0.5;

    let mut bands = vec![0.0f32; BAND_COUNT];
    for (i, band) in bands.iter_mut().enumerate() {
        let lo_bin = ((edges[i] / bin_hz).floor() as usize).max(1);
        let hi_bin = ((edges[i + 1] / bin_hz).ceil() as usize)
            .min(nyquist_bin)
            .max(lo_bin + 1);

        let mut peak = 0.0f32;
        for bin in &spectrum[lo_bin..hi_bin] {
            let mag = bin.norm() / scale;
            if mag > peak {
                peak = mag;
            }
        }

        let db = 20.0 * (peak + 1e-9).log10();
        *band = ((db - DB_FLOOR) / (DB_CEIL - DB_FLOOR)).clamp(0.0, 1.0);
    }
    bands
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthesize_tone(freq: f32, samples: usize) -> Vec<f32> {
        (0..samples)
            .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / SAMPLE_RATE).sin())
            .collect()
    }

    fn run_fft(samples: &[f32]) -> Vec<f32> {
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);
        let window = hann_window(FFT_SIZE);
        let mut scratch: Vec<Complex<f32>> = (0..FFT_SIZE)
            .map(|i| Complex::new(samples[i] * window[i], 0.0))
            .collect();
        fft.process(&mut scratch);
        compute_bands(&scratch, &log_band_edges())
    }

    #[test]
    fn band_edges_span_min_to_max() {
        let edges = log_band_edges();
        assert_eq!(edges.len(), BAND_COUNT + 1);
        assert!((edges[0] - FREQ_MIN).abs() < 0.01);
        assert!((edges[BAND_COUNT] - FREQ_MAX).abs() < 0.01);
    }

    #[test]
    fn low_tone_lights_up_low_band() {
        let tone = synthesize_tone(80.0, FFT_SIZE);
        let bands = run_fft(&tone);
        let peak = bands
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .unwrap()
            .0;
        assert!(peak < BAND_COUNT / 2, "80Hz should peak in lower half");
    }

    #[test]
    fn high_tone_lights_up_high_band() {
        let tone = synthesize_tone(8_000.0, FFT_SIZE);
        let bands = run_fft(&tone);
        let peak = bands
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .unwrap()
            .0;
        assert!(peak > BAND_COUNT / 2, "8kHz should peak in upper half");
    }

    #[test]
    fn silence_yields_zero_bands() {
        let bands = run_fft(&vec![0.0; FFT_SIZE]);
        assert!(bands.iter().all(|&b| b < 0.01));
    }
}
