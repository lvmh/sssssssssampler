// ─── Audio Analysis ──────────────────────────────────────────────────────────
//
// RMS computation, transient detection, and smoothing for animation parameters.
// Provides real-time signal analysis for visualization.

/// Computes RMS (root mean square) amplitude from sample buffer
pub fn compute_rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }

    let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
    (sum_sq / samples.len() as f32).sqrt()
}

/// Analyzes audio samples for animation parameters
#[derive(Clone, Debug)]
pub struct AudioAnalyzer {
    /// Ring buffer of RMS values (3 frames)
    rms_history: [f32; 3],
    /// Current write position in history
    history_pos: usize,
    /// Average RMS for transient detection threshold
    average_rms: f32,
    /// Whether a transient is currently active
    pub transient_active: bool,
}

impl AudioAnalyzer {
    pub fn new() -> Self {
        Self {
            rms_history: [0.0; 3],
            history_pos: 0,
            average_rms: 0.0,
            transient_active: false,
        }
    }

    /// Update analyzer with new sample data
    pub fn update(&mut self, samples: &[f32]) {
        let rms = compute_rms(samples);

        // Add to history
        self.rms_history[self.history_pos] = rms;
        self.history_pos = (self.history_pos + 1) % 3;

        // Update average for transient threshold
        self.average_rms = self.rms_history.iter().sum::<f32>() / 3.0;

        // Detect transient: spike > 2x average
        self.transient_active = rms > self.average_rms * 2.0;
    }

    /// Get smoothed RMS using 3-frame exponential moving average
    pub fn smoothed_rms(&self) -> f32 {
        // Simple 3-frame average
        self.rms_history.iter().sum::<f32>() / 3.0
    }

    /// Get current transient state
    pub fn transient_active(&self) -> bool {
        self.transient_active
    }

    /// Get average RMS level
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
}
