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
    /// Effective BPM from host transport (halved if >115 for half-time)
    pub bpm: f32,
    /// Whether the DAW transport is currently playing
    pub playing: bool,
    /// Visual energy: normalized 0–1, drives animation intensity
    pub energy: f32,
    /// Whether a transient (spike) is currently active
    pub transient: bool,
    /// Signal classification: 0=Percussive, 1=Tonal, 2=Ambient
    pub signal_class: u8,
    /// Auto-gained normalized RMS (for transient scaling)
    pub normalized_rms: f32,
    // ── V6: Stereo + sub-bass ──
    /// Sub-bass energy (LPF at ~200Hz, 0.0–1.0)
    pub sub_bass_energy: f32,
    /// Stereo width: 0.0 (mono) to 1.0 (full separation)
    pub stereo_width: f32,
    /// L-R balance: -1.0 (full L) to +1.0 (full R)
    pub lr_balance: f32,
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
            bpm: 120.0,
            playing: false,
            energy: 0.0,
            transient: false,
            signal_class: 1,
            normalized_rms: 0.0,
            sub_bass_energy: 0.0,
            stereo_width: 0.0,
            lr_balance: 0.0,
        }
    }
}

/// Audio feed processor: collects samples, analyzes them, updates shared params
pub struct AudioFeed {
    /// Audio analyzer for RMS and transient detection
    analyzer: AudioAnalyzer,
    /// Ring buffers for collecting L/R samples between analysis updates
    buffer_l: Vec<f32>,
    buffer_r: Vec<f32>,
    /// Shared animation parameters (updated by DSP, read by render)
    pub shared_params: Arc<Mutex<AnimationParams>>,
    /// Buffer capacity (number of samples to accumulate before analysis)
    buffer_capacity: usize,
    /// Host sample rate (set from lib.rs initialize)
    pub host_sr: f32,
}

impl AudioFeed {
    /// Create a new audio feed processor
    pub fn new(buffer_size: usize) -> Self {
        Self {
            analyzer: AudioAnalyzer::new(),
            buffer_l: Vec::with_capacity(buffer_size),
            buffer_r: Vec::with_capacity(buffer_size),
            shared_params: Arc::new(Mutex::new(AnimationParams::default())),
            buffer_capacity: buffer_size,
            host_sr: 44100.0,
        }
    }

    /// Add a sample to the buffer (stereo-aware)
    #[inline]
    pub fn push_sample(&mut self, sample: f32, channel: usize) {
        let abs = sample.abs();
        if channel == 0 {
            self.buffer_l.push(abs);
        } else {
            self.buffer_r.push(abs);
        }
        if self.buffer_l.len() >= self.buffer_capacity {
            self.analyze_and_update();
        }
    }

    /// Analyze accumulated samples and update animation parameters
    fn analyze_and_update(&mut self) {
        if self.buffer_l.is_empty() {
            return;
        }

        // Pad R buffer if mono input
        if self.buffer_r.is_empty() {
            self.buffer_r.clone_from(&self.buffer_l);
        }

        self.analyzer.update_stereo(&self.buffer_l, &self.buffer_r, self.host_sr);

        let rms = self.analyzer.smoothed_rms();
        let instability = sample_rate_to_instability(96_000.0);
        let quantization = bit_depth_to_quantization(12.0);
        let layer_count = amplitude_to_layer_count(rms);
        let region_offset = jitter_to_region_offset(0.01);
        let brightness = amplitude_to_brightness(rms);
        let motion_speed = amplitude_to_motion_speed(rms);

        let existing = self.shared_params.lock().ok()
            .map(|p| (p.bpm, p.playing))
            .unwrap_or((120.0, false));
        let transient = self.analyzer.transient_active();
        let normalized_energy = self.analyzer.normalized_energy();
        let norm_rms = self.analyzer.normalized_rms();
        let transient_scaled = if transient {
            0.3 * (1.0 + (1.0 - norm_rms.min(1.0)))
        } else { 0.0 };
        let energy = (normalized_energy + transient_scaled).clamp(0.0, 1.0);
        let anim_params = AnimationParams {
            instability,
            quantization,
            layer_count,
            region_offset,
            brightness,
            motion_speed,
            rms,
            bpm: existing.0,
            playing: existing.1,
            energy,
            transient,
            signal_class: self.analyzer.signal_class(),
            normalized_rms: norm_rms,
            sub_bass_energy: self.analyzer.sub_bass_energy(),
            stereo_width: self.analyzer.stereo_width(),
            lr_balance: self.analyzer.lr_balance(),
        };

        if let Ok(mut params) = self.shared_params.lock() {
            *params = anim_params;
        }

        self.buffer_l.clear();
        self.buffer_r.clear();
    }

    /// Update animation parameters with current DSP values
    pub fn update(
        &mut self,
        target_sample_rate: f32,
        bit_depth: f32,
        jitter: f32,
        host_bpm: f32,
        host_playing: bool,
    ) {
        let instability = sample_rate_to_instability(target_sample_rate);
        let quantization = bit_depth_to_quantization(bit_depth);
        let region_offset = jitter_to_region_offset(jitter);

        let rms = self.analyzer.smoothed_rms();
        let layer_count = amplitude_to_layer_count(rms);
        let brightness = amplitude_to_brightness(rms);
        let motion_speed = amplitude_to_motion_speed(rms);

        let effective_bpm = if host_bpm > 115.0 { host_bpm * 0.5 } else { host_bpm };

        let transient = self.analyzer.transient_active();
        let normalized_energy = self.analyzer.normalized_energy();
        let norm_rms = self.analyzer.normalized_rms();
        let transient_scaled = if transient {
            0.3 * (1.0 + (1.0 - norm_rms.min(1.0)))
        } else { 0.0 };
        let energy = (normalized_energy + transient_scaled).clamp(0.0, 1.0);

        let anim_params = AnimationParams {
            instability,
            quantization,
            layer_count,
            region_offset,
            brightness,
            motion_speed,
            rms,
            bpm: effective_bpm,
            playing: host_playing,
            energy,
            transient,
            signal_class: self.analyzer.signal_class(),
            normalized_rms: norm_rms,
            sub_bass_energy: self.analyzer.sub_bass_energy(),
            stereo_width: self.analyzer.stereo_width(),
            lr_balance: self.analyzer.lr_balance(),
        };

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
}

impl Default for AudioFeed {
    fn default() -> Self {
        Self::new(1024)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_params(feed: &AudioFeed) -> AnimationParams {
        feed.shared_params.lock().unwrap().clone()
    }

    #[test]
    fn test_audio_feed_creation() {
        let feed = AudioFeed::new(512);
        let params = get_params(&feed);
        assert_eq!(params.layer_count, 1.0);
        assert_eq!(params.brightness, 1.0);
    }

    #[test]
    fn test_push_sample() {
        let mut feed = AudioFeed::new(4);
        for _ in 0..4 {
            feed.push_sample(0.5, 0);
            feed.push_sample(0.5, 1);
        }

        let params = get_params(&feed);
        assert!(params.rms > 0.0);
    }

    #[test]
    fn test_update_params() {
        let mut feed = AudioFeed::new(512);
        feed.push_sample(0.1, 0);
        feed.push_sample(0.1, 1);

        feed.update(26_040.0, 12.0, 0.01, 120.0, true);

        let params = get_params(&feed);
        assert!(params.instability > 0.3);
        assert!(params.quantization > 0.0);
    }

    #[test]
    fn test_stereo_params() {
        let mut feed = AudioFeed::new(4);
        for _ in 0..8 {
            feed.push_sample(0.8, 0);
            feed.push_sample(0.1, 1);
        }
        let params = get_params(&feed);
        assert!(params.stereo_width > 0.0);
    }
}
