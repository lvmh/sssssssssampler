// ─── Audio Feed Integration ────────────────────────────────────────────────────
//
// Wires audio analysis into DSP processing loop.
// Collects samples, analyzes them, and updates animation parameters.

use std::sync::{Arc, Mutex};
use crate::render::AudioAnalyzer;
use crate::{
    sample_rate_to_instability,
    bit_depth_to_quantization,
    amplitude_to_layer_count,
    jitter_to_region_offset,
    amplitude_to_brightness,
    amplitude_to_motion_speed,
};

/// Shared animation parameters that bridge DSP and render loops
#[derive(Clone, Debug)]
pub struct AnimationParams {
    /// Instability score (0.0–1.0) from sample rate degradation
    pub instability: f32,
    /// Quantization factor (0.0–1.0) from bit depth reduction
    pub quantization: f32,
    /// Layer count multiplier (1.0–5.0) from amplitude
    pub layer_count: f32,
    /// Region desynchronization offset (0.0–1.0) from jitter
    pub region_offset: f32,
    /// Brightness multiplier (0.5–2.0) from amplitude
    pub brightness: f32,
    /// Motion speed multiplier (0.5–3.0) from amplitude
    pub motion_speed: f32,
    /// Raw RMS level for additional visualizations
    pub rms: f32,
}

impl Default for AnimationParams {
    fn default() -> Self {
        Self {
            instability: 0.0,
            quantization: 0.0,
            layer_count: 1.0,
            region_offset: 0.01,
            brightness: 1.0,
            motion_speed: 1.0,
            rms: 0.0,
        }
    }
}

/// Audio feed processor: collects samples, analyzes them, updates shared params
pub struct AudioFeed {
    /// Audio analyzer for RMS and transient detection
    analyzer: AudioAnalyzer,
    /// Ring buffer for collecting samples between analysis updates
    buffer: Vec<f32>,
    /// Shared animation parameters (updated by DSP, read by render)
    pub shared_params: Arc<Mutex<AnimationParams>>,
    /// Buffer capacity (number of samples to accumulate before analysis)
    buffer_capacity: usize,
}

impl AudioFeed {
    /// Create a new audio feed processor
    ///
    /// # Arguments
    /// * `buffer_size` - Number of samples to accumulate before analysis (typically 512–2048)
    pub fn new(buffer_size: usize) -> Self {
        Self {
            analyzer: AudioAnalyzer::new(),
            buffer: Vec::with_capacity(buffer_size),
            shared_params: Arc::new(Mutex::new(AnimationParams::default())),
            buffer_capacity: buffer_size,
        }
    }

    /// Add a sample to the buffer
    #[inline]
    pub fn push_sample(&mut self, sample: f32) {
        self.buffer.push(sample.abs());
        if self.buffer.len() >= self.buffer_capacity {
            self.analyze_and_update();
        }
    }

    /// Analyze accumulated samples and update animation parameters
    fn analyze_and_update(&mut self) {
        if self.buffer.is_empty() {
            return;
        }

        // Analyze the buffer
        self.analyzer.update(&self.buffer);

        // Get smoothed RMS
        let rms = self.analyzer.smoothed_rms();

        // Compute animation parameters via remapping functions
        let instability = sample_rate_to_instability(96_000.0); // Will be set externally
        let quantization = bit_depth_to_quantization(12.0);     // Will be set externally
        let layer_count = amplitude_to_layer_count(rms);
        let region_offset = jitter_to_region_offset(0.01);      // Will be set externally
        let brightness = amplitude_to_brightness(rms);
        let motion_speed = amplitude_to_motion_speed(rms);

        let anim_params = AnimationParams {
            instability,
            quantization,
            layer_count,
            region_offset,
            brightness,
            motion_speed,
            rms,
        };

        // Update shared parameters
        if let Ok(mut params) = self.shared_params.lock() {
            *params = anim_params;
        }

        // Clear buffer for next cycle
        self.buffer.clear();
    }

    /// Update animation parameters with current DSP values
    /// Call this from the DSP process loop with the current parameter values
    pub fn update(
        &mut self,
        target_sample_rate: f32,
        bit_depth: f32,
        jitter: f32,
    ) {
        // Compute remapped parameters
        let instability = sample_rate_to_instability(target_sample_rate);
        let quantization = bit_depth_to_quantization(bit_depth);
        let region_offset = jitter_to_region_offset(jitter);

        // Get current RMS and compute amplitude-derived parameters
        let rms = self.analyzer.smoothed_rms();
        let layer_count = amplitude_to_layer_count(rms);
        let brightness = amplitude_to_brightness(rms);
        let motion_speed = amplitude_to_motion_speed(rms);

        let anim_params = AnimationParams {
            instability,
            quantization,
            layer_count,
            region_offset,
            brightness,
            motion_speed,
            rms,
        };

        // Update shared parameters
        if let Ok(mut params) = self.shared_params.lock() {
            *params = anim_params;
        }
    }

    /// Get a clone of current animation parameters
    pub fn get_params(&self) -> AnimationParams {
        self.shared_params
            .lock()
            .ok()
            .map(|p| p.clone())
            .unwrap_or_default()
    }

    /// Check if a transient is currently active
    pub fn is_transient_active(&self) -> bool {
        self.analyzer.transient_active()
    }

    /// Get the current average RMS level
    pub fn average_rms(&self) -> f32 {
        self.analyzer.average_rms()
    }
}

impl Default for AudioFeed {
    fn default() -> Self {
        Self::new(1024)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_feed_creation() {
        let feed = AudioFeed::new(512);
        let params = feed.get_params();
        assert_eq!(params.layer_count, 1.0);
        assert_eq!(params.brightness, 0.5);
    }

    #[test]
    fn test_push_sample() {
        let mut feed = AudioFeed::new(4);
        feed.push_sample(0.5);
        feed.push_sample(0.5);
        feed.push_sample(0.5);
        feed.push_sample(0.5); // Should trigger analysis

        let params = feed.get_params();
        assert!(params.rms > 0.0);
    }

    #[test]
    fn test_update_params() {
        let mut feed = AudioFeed::new(512);
        feed.push_sample(0.1);
        feed.push_sample(0.1);

        // Update with DSP values
        feed.update(26_040.0, 12.0, 0.01);

        let params = feed.get_params();
        // Instability should be high for low SR
        assert!(params.instability > 0.5);
        // Quantization should reflect 12-bit depth
        assert!(params.quantization > 0.0);
    }
}
