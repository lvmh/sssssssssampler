// ─── Audio Analysis ──────────────────────────────────────────────────────────
//
// RMS computation, transient detection, auto-gain normalization, signal
// classification, sub-bass detection, and stereo width for animation parameters.

/// Computes RMS (root mean square) amplitude from sample buffer
pub fn compute_rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }

    let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
    (sum_sq / samples.len() as f32).sqrt()
}

/// Signal classification: how the visual system adapts to different audio types
/// 0 = Percussive (drums, transient-heavy)
/// 1 = Tonal (melodic, harmonic)
/// 2 = Ambient (pads, drones, low variation)
pub const SIGNAL_PERCUSSIVE: u8 = 0;
pub const SIGNAL_TONAL: u8 = 1;
pub const SIGNAL_AMBIENT: u8 = 2;

/// Analyzes audio samples for animation parameters
#[derive(Clone, Debug)]
pub struct AudioAnalyzer {
    /// Ring buffer of RMS values (3 frames)
    rms_history: [f32; 3],
    /// Current write position in history
    history_pos: usize,
    /// Average RMS for transient detection threshold
    average_rms: f32,
    /// Previous frame RMS for rising-edge detection
    prev_rms: f32,
    /// Whether a transient is currently active
    pub transient_active: bool,
    // ── Auto-gain normalization ──
    /// Adaptive gain (converges so normalized RMS ≈ 0.25)
    auto_gain: f32,
    /// RMS after auto-gain (for transient scaling)
    pub normalized_rms: f32,
    // ── Signal classification ──
    /// Rolling transient occurrence rate (0-1)
    transient_density: f32,
    /// Rolling RMS frame-to-frame variation (0-1)
    rms_variation: f32,
    /// Classified signal type: 0=Percussive, 1=Tonal, 2=Ambient
    signal_class: u8,
    // ── V6: Stereo + sub-bass ──
    /// Per-channel smoothed RMS
    rms_l: f32,
    rms_r: f32,
    /// Single-pole LPF state for sub-bass extraction (~200Hz cutoff)
    sub_bass_lpf: f32,
    /// Smoothed sub-bass energy
    sub_bass_energy: f32,
}

impl AudioAnalyzer {
    pub fn new() -> Self {
        Self {
            rms_history: [0.0; 3],
            history_pos: 0,
            average_rms: 0.0,
            prev_rms: 0.0,
            transient_active: false,
            auto_gain: 1.0,
            normalized_rms: 0.0,
            transient_density: 0.0,
            rms_variation: 0.0,
            signal_class: SIGNAL_TONAL,
            rms_l: 0.0,
            rms_r: 0.0,
            sub_bass_lpf: 0.0,
            sub_bass_energy: 0.0,
        }
    }

    /// Update analyzer with new sample data (mono compatibility)
    pub fn update(&mut self, samples: &[f32]) {
        self.update_core(compute_rms(samples));
        // Mono: treat as centered
        let rms = self.smoothed_rms();
        self.rms_l += (rms - self.rms_l) * 0.15;
        self.rms_r += (rms - self.rms_r) * 0.15;
    }

    /// Update with stereo sample buffers + sub-bass extraction
    pub fn update_stereo(&mut self, samples_l: &[f32], samples_r: &[f32], sample_rate: f32) {
        let rms_l = compute_rms(samples_l);
        let rms_r = compute_rms(samples_r);

        // Combined mono RMS for existing pipeline
        let mono: Vec<f32> = samples_l.iter().zip(samples_r.iter())
            .map(|(l, r)| (l + r) * 0.5)
            .collect();
        let rms = compute_rms(&mono);
        self.update_core(rms);

        // Smooth per-channel RMS
        self.rms_l += (rms_l - self.rms_l) * 0.15;
        self.rms_r += (rms_r - self.rms_r) * 0.15;

        // Sub-bass extraction: single-pole LPF at ~200Hz applied to RMS envelope
        // alpha = dt × 2π × fc, where dt = buffer_size / sample_rate
        let dt = if sample_rate > 0.0 { samples_l.len() as f32 / sample_rate } else { 0.02 };
        let alpha = (dt * 2.0 * std::f32::consts::PI * 200.0).min(1.0);
        self.sub_bass_lpf += (rms - self.sub_bass_lpf) * alpha;
        self.sub_bass_energy += (self.sub_bass_lpf - self.sub_bass_energy) * 0.1;
    }

    /// Core update logic shared by mono and stereo paths
    fn update_core(&mut self, rms: f32) {
        // Add to history
        self.rms_history[self.history_pos] = rms;
        self.history_pos = (self.history_pos + 1) % 3;

        // Update average for transient threshold
        self.average_rms = self.rms_history.iter().sum::<f32>() / 3.0;

        // Detect transient: spike > 2x average AND rising from previous frame
        let spike = rms > self.average_rms * 2.0;
        let rising = rms > self.prev_rms * 1.2;
        self.transient_active = spike && rising;

        // ── Auto-gain normalization ──
        let smoothed = self.smoothed_rms();
        if smoothed > 0.0001 {
            let target_gain = 0.25 / smoothed;
            self.auto_gain += (target_gain - self.auto_gain) * 0.01;
            self.auto_gain = self.auto_gain.clamp(0.5, 8.0);
        }
        self.normalized_rms = smoothed * self.auto_gain;

        // ── Signal classification ──
        let transient_val = if self.transient_active { 1.0f32 } else { 0.0 };
        self.transient_density += (transient_val - self.transient_density) * 0.02;

        let rms_delta = (rms - self.prev_rms).abs();
        self.rms_variation += (rms_delta - self.rms_variation) * 0.02;

        self.signal_class = if self.transient_density > 0.15 {
            SIGNAL_PERCUSSIVE
        } else if self.rms_variation < 0.005 {
            SIGNAL_AMBIENT
        } else {
            SIGNAL_TONAL
        };

        self.prev_rms = rms;
    }

    /// Normalized energy with auto-gain and tanh compression
    pub fn normalized_energy(&self) -> f32 {
        let normalized = self.smoothed_rms() * self.auto_gain;
        let compressed = (normalized * 1.5).tanh();
        let floor = 0.08;
        floor + (1.0 - floor) * compressed
    }

    /// Get smoothed RMS using 3-frame average
    pub fn smoothed_rms(&self) -> f32 {
        self.rms_history.iter().sum::<f32>() / 3.0
    }

    /// Get current transient state
    pub fn transient_active(&self) -> bool {
        self.transient_active
    }

    /// Get signal classification (0=Percussive, 1=Tonal, 2=Ambient)
    pub fn signal_class(&self) -> u8 {
        self.signal_class
    }

    /// Get normalized RMS (after auto-gain)
    pub fn normalized_rms(&self) -> f32 {
        self.normalized_rms
    }

    /// Stereo width: 0.0 (mono) to 1.0 (full L-R separation)
    pub fn stereo_width(&self) -> f32 {
        let sum = self.rms_l + self.rms_r;
        if sum < 0.001 { return 0.0; }
        ((self.rms_l - self.rms_r).abs() / sum).clamp(0.0, 1.0)
    }

    /// L-R balance: -1.0 (full L) to +1.0 (full R), 0.0 = center
    pub fn lr_balance(&self) -> f32 {
        let sum = self.rms_l + self.rms_r;
        if sum < 0.001 { return 0.0; }
        ((self.rms_r - self.rms_l) / sum).clamp(-1.0, 1.0)
    }

    /// Sub-bass energy (smoothed, LPF at ~200Hz)
    pub fn sub_bass_energy(&self) -> f32 {
        self.sub_bass_energy
    }

    /// Average RMS (for tests)
    pub fn average_rms(&self) -> f32 {
        self.average_rms
    }
}

impl Default for AudioAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_rms() {
        let samples = vec![0.0, 0.5, 1.0, 0.5, 0.0];
        let rms = compute_rms(&samples);
        assert!(rms > 0.0);
        assert!(rms < 1.0);
    }

    #[test]
    fn test_rms_empty() {
        assert_eq!(compute_rms(&[]), 0.0);
    }

    #[test]
    fn test_transient_detection() {
        let mut analyzer = AudioAnalyzer::new();

        // Three frames of quiet signal
        analyzer.update(&[0.1, 0.1, 0.1]);
        analyzer.update(&[0.1, 0.1, 0.1]);
        analyzer.update(&[0.1, 0.1, 0.1]);

        // Now send a spike (should be > 2x average)
        analyzer.update(&[1.0, 1.0, 1.0]);
        assert!(analyzer.transient_active());
    }

    #[test]
    fn test_smoothed_rms() {
        let mut analyzer = AudioAnalyzer::new();
        analyzer.update(&[0.5, 0.5, 0.5]);
        analyzer.update(&[0.6, 0.6, 0.6]);
        analyzer.update(&[0.7, 0.7, 0.7]);

        let smoothed = analyzer.smoothed_rms();
        assert!(smoothed > 0.0);
    }

    #[test]
    fn test_auto_gain_convergence() {
        let mut analyzer = AudioAnalyzer::new();
        // Feed quiet signal — auto_gain should increase
        for _ in 0..200 {
            analyzer.update(&[0.01, 0.01, 0.01]);
        }
        assert!(analyzer.auto_gain > 1.0);
        assert!(analyzer.normalized_energy() > 0.08); // above floor
    }

    #[test]
    fn test_signal_classification() {
        let mut analyzer = AudioAnalyzer::new();
        // Steady signal → should classify as ambient or tonal
        for _ in 0..100 {
            analyzer.update(&[0.3, 0.3, 0.3]);
        }
        assert!(analyzer.signal_class() == SIGNAL_AMBIENT || analyzer.signal_class() == SIGNAL_TONAL);
    }

    #[test]
    fn test_stereo_width() {
        let mut analyzer = AudioAnalyzer::new();
        // Feed L-heavy signal
        for _ in 0..10 {
            analyzer.update_stereo(&[0.8, 0.8], &[0.1, 0.1], 48000.0);
        }
        assert!(analyzer.stereo_width() > 0.3);
        assert!(analyzer.lr_balance() < -0.3); // L-heavy → negative
    }

    #[test]
    fn test_sub_bass_energy() {
        let mut analyzer = AudioAnalyzer::new();
        // Feed signal — sub_bass should accumulate
        for _ in 0..50 {
            analyzer.update_stereo(&[0.5, 0.5], &[0.5, 0.5], 48000.0);
        }
        assert!(analyzer.sub_bass_energy() > 0.0);
    }
}
