use std::collections::VecDeque;
use std::sync::mpsc::Receiver;
use tokio::sync::watch;

const SAMPLE_RATE: f32 = 44100.0;
const WINDOW_SAMPLES: usize = 1024; // ~23ms of mono audio per energy window
const HISTORY_SIZE: usize = 10; // local average over ~230ms
const ONSET_THRESHOLD: f32 = 1.4; // energy must be 1.4x the local average
const MIN_ENERGY: f32 = 1e-4; // ignore near-silence
const MIN_ONSET_GAP_S: f32 = 0.15; // debounce: ignore onsets closer than 150ms
const ONSET_HISTORY_S: f32 = 8.0; // keep onsets from the last 8 seconds
const MIN_ONSETS: usize = 8; // need at least this many onsets for a reliable estimate
const UPDATE_INTERVAL_S: f32 = 3.0; // compute a new raw estimate every 3s
const MEDIAN_WINDOW: usize = 7; // median filter over this many raw estimates
const STABILITY_THRESHOLD: f32 = 1.0; // only update display if BPM shifts by more than this
const BPM_MIN: f32 = 60.0;
const BPM_MAX: f32 = 180.0;

pub struct BpmDetector {
    rx: Receiver<Vec<f64>>,
    bpm_tx: watch::Sender<Option<f32>>,
}

impl BpmDetector {
    pub fn new(rx: Receiver<Vec<f64>>, bpm_tx: watch::Sender<Option<f32>>) -> Self {
        Self { rx, bpm_tx }
    }

    pub fn run(self) {
        let mut mono_buf: Vec<f32> = Vec::with_capacity(WINDOW_SAMPLES);
        let mut energy_history: VecDeque<f32> = VecDeque::with_capacity(HISTORY_SIZE + 1);
        let mut onset_times: VecDeque<f32> = VecDeque::new();
        let mut last_onset_time: f32 = -1.0;
        let mut sample_clock: f32 = 0.0; // cumulative mono samples processed
        let mut last_update_time: f32 = 0.0;
        let mut raw_estimates: VecDeque<f32> = VecDeque::with_capacity(MEDIAN_WINDOW + 1);
        let mut last_emitted: Option<f32> = None;

        loop {
            let chunk = match self.rx.recv() {
                Ok(c) => c,
                Err(_) => break, // sender dropped — player shut down
            };

            // Interleaved stereo f64 → mono f32
            for pair in chunk.chunks(2) {
                let mono = match pair {
                    [l, r] => (*l + *r) as f32 * 0.5,
                    [l] => *l as f32,
                    _ => continue,
                };
                mono_buf.push(mono);

                if mono_buf.len() >= WINDOW_SAMPLES {
                    let window_time = sample_clock / SAMPLE_RATE;

                    // RMS² energy of this window
                    let energy =
                        mono_buf.iter().map(|s| s * s).sum::<f32>() / WINDOW_SAMPLES as f32;

                    energy_history.push_back(energy);
                    if energy_history.len() > HISTORY_SIZE {
                        energy_history.pop_front();
                    }

                    let local_avg =
                        energy_history.iter().sum::<f32>() / energy_history.len() as f32;

                    // Onset: sharp energy spike above local average
                    if energy > ONSET_THRESHOLD * local_avg
                        && energy > MIN_ENERGY
                        && window_time - last_onset_time > MIN_ONSET_GAP_S
                    {
                        onset_times.push_back(window_time);
                        last_onset_time = window_time;

                        // Evict old onsets outside the analysis window
                        while onset_times
                            .front()
                            .is_some_and(|&t| window_time - t > ONSET_HISTORY_S)
                        {
                            onset_times.pop_front();
                        }
                    }

                    sample_clock += WINDOW_SAMPLES as f32;
                    mono_buf.clear();

                    // Periodically compute a raw estimate and feed the median filter
                    if window_time - last_update_time >= UPDATE_INTERVAL_S
                        && onset_times.len() >= MIN_ONSETS
                    {
                        if let Some(raw) = estimate_bpm(&onset_times) {
                            raw_estimates.push_back(raw);
                            if raw_estimates.len() > MEDIAN_WINDOW {
                                raw_estimates.pop_front();
                            }

                            let median = median_bpm(&raw_estimates);

                            // Only push to the watch channel if the value changed meaningfully.
                            let should_emit = match last_emitted {
                                None => true,
                                Some(prev) => (median - prev).abs() > STABILITY_THRESHOLD,
                            };
                            if should_emit {
                                last_emitted = Some(median);
                                let _ = self.bpm_tx.send(Some(median));
                            }
                        }
                        last_update_time = window_time;
                    }
                }
            }
        }
    }
}

/// Inter-onset interval histogram → BPM.
fn estimate_bpm(onsets: &VecDeque<f32>) -> Option<f32> {
    // Collect valid inter-onset intervals
    let iois: Vec<f32> = onsets
        .iter()
        .zip(onsets.iter().skip(1))
        .map(|(a, b)| b - a)
        .filter(|&ioi| {
            let bpm = 60.0 / ioi;
            bpm >= BPM_MIN && bpm <= BPM_MAX
        })
        .collect();

    if iois.len() < 4 {
        return None;
    }

    // Histogram in 10ms bins over the valid IOI range
    const BIN_MS: f32 = 0.01;
    let ioi_min = 60.0 / BPM_MAX;
    let ioi_max = 60.0 / BPM_MIN;
    let num_bins = ((ioi_max - ioi_min) / BIN_MS) as usize + 1;
    let mut histogram = vec![0u32; num_bins];

    for ioi in &iois {
        let bin = ((ioi - ioi_min) / BIN_MS) as usize;
        if bin < num_bins {
            histogram[bin] += 1;
        }
    }

    let (peak_bin, _) = histogram
        .iter()
        .enumerate()
        .max_by_key(|&(_, count)| count)?;

    let peak_ioi = ioi_min + peak_bin as f32 * BIN_MS;
    let mut bpm = 60.0 / peak_ioi;

    // Fold into 90–150 BPM range to avoid octave errors (e.g. 64 vs 128 BPM)
    while bpm < 90.0 {
        bpm *= 2.0;
    }
    while bpm > 150.0 {
        bpm /= 2.0;
    }

    Some(bpm)
}

/// Median of a small window of BPM estimates — robust against outliers.
fn median_bpm(estimates: &VecDeque<f32>) -> f32 {
    let mut sorted: Vec<f32> = estimates.iter().copied().collect();
    sorted.sort_by(f32::total_cmp);
    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 0 {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        sorted[mid]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_onsets(bpm: f32, count: usize) -> VecDeque<f32> {
        let period = 60.0 / bpm;
        (0..count).map(|i| i as f32 * period).collect()
    }

    #[test]
    fn detects_128_bpm() {
        let onsets = make_onsets(128.0, 20);
        let result = estimate_bpm(&onsets).expect("should detect BPM");
        assert!((result - 128.0).abs() < 2.0, "got {result}");
    }

    #[test]
    fn detects_120_bpm() {
        let onsets = make_onsets(120.0, 20);
        let result = estimate_bpm(&onsets).expect("should detect BPM");
        assert!((result - 120.0).abs() < 2.0, "got {result}");
    }

    #[test]
    fn folds_64_bpm_to_128() {
        // 64 BPM is an octave below 128 — should be doubled into range
        let onsets = make_onsets(64.0, 20);
        let result = estimate_bpm(&onsets).expect("should detect BPM");
        assert!((result - 128.0).abs() < 2.0, "got {result}");
    }

    #[test]
    fn returns_none_for_too_few_onsets() {
        let onsets = make_onsets(128.0, 3);
        assert!(estimate_bpm(&onsets).is_none());
    }
}
