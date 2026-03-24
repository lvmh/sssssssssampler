/// Non-linear parameter remapping for audio quality perception

/// Map sample rate to visual degradation intensity (0.0–1.0)
/// Returns "instability score": 0 at high SR, 1 at very low SR.
/// Calibrated for target_sr range 4,000–48,000 Hz.
pub fn sample_rate_to_instability(sr_hz: f32) -> f32 {
    // Zones:
    // 44kHz–48kHz: minimal (0.0–0.1)
    // 30kHz–44kHz: mild    (0.1–0.3)
    // 15kHz–30kHz: moderate (0.3–0.6)
    //  4kHz–15kHz: extreme  (0.6–1.0)
    let sr = sr_hz.clamp(4_000.0, 48_000.0);
    if sr >= 44_000.0 {
        0.05 + (48_000.0 - sr) / 80_000.0     // 0.05 at 48kHz → 0.10 at 44kHz
    } else if sr >= 30_000.0 {
        0.1 + (44_000.0 - sr) / 56_000.0      // 0.1 at 44kHz → 0.3 at 30kHz
    } else if sr >= 15_000.0 {
        0.3 + (30_000.0 - sr) / 42_857.0      // 0.3 at 30kHz → 0.6 at 15kHz
    } else {
        0.6 + (15_000.0 - sr) / 27_500.0      // 0.6 at 15kHz → 1.0 at 4kHz
    }
}

/// Map bit depth to character set reduction (0.0–1.0)
/// Returns "quantization factor": 0 at 24-bit, 1 at 1-bit
pub fn bit_depth_to_quantization(bits: f32) -> f32 {
    let normalized = ((bits - 1.0) / 23.0).clamp(0.0, 1.0);
    // Inverted: 1.0 at 1-bit (severe), 0.0 at 24-bit (none)
    1.0 - normalized
}

/// Map amplitude (RMS) to layer activity (0.0–1.0)
/// Low = 1 layer, High = 5 layers
pub fn amplitude_to_layer_count(rms: f32) -> f32 {
    // RMS typically 0.0–1.0 (normalized)
    // Exponential growth: quiet -> 0.5 layers (clamped 1), loud -> 4.5 (clamped 5)
    let exponent = 3.0;
    1.0 + (rms.powf(exponent) * 4.0)
}

/// Map jitter parameter to region desynchronization (0.0–1.0)
pub fn jitter_to_region_offset(jitter: f32) -> f32 {
    // Jitter ∈ [0, 1], directly use as offset scale
    jitter.max(0.01) // Minimum offset to avoid total alignment
}

/// Map amplitude to brightness multiplier (0.5–2.0)
/// Low = dim, high = bright
pub fn amplitude_to_brightness(rms: f32) -> f32 {
    0.5 + (rms.clamp(0.0, 1.0) * 1.5)
}

/// Map amplitude to motion speed multiplier (0.5–3.0)
pub fn amplitude_to_motion_speed(rms: f32) -> f32 {
    0.5 + (rms.clamp(0.0, 1.0) * 2.5)
}
