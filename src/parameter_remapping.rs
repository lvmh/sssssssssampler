/// Non-linear parameter remapping for audio quality perception

/// Map sample rate to visual degradation intensity (0.0–1.0)
/// Returns "instability score": 0 at high SR, 1 at very low SR
pub fn sample_rate_to_instability(sr_hz: f32) -> f32 {
    // Zones:
    // 44.1kHz+: minimal (zone 1, 0.0–0.1)
    // 30kHz–44kHz: mild (zone 2, 0.1–0.3)
    // 15kHz–30kHz: moderate (zone 3, 0.3–0.6)
    // <15kHz: extreme (zone 4, 0.6–1.0)

    let normalized = (sr_hz / 96_000.0).clamp(0.0, 1.0);

    if normalized >= 0.46 {        // ≥ 44kHz
        (1.0 - normalized) * 0.2    // 0.0–0.1
    } else if normalized >= 0.31 { // 30–44kHz
        0.1 + ((0.46 - normalized) / 0.15) * 0.2 // 0.1–0.3
    } else if normalized >= 0.16 { // 15–30kHz
        0.3 + ((0.31 - normalized) / 0.15) * 0.3 // 0.3–0.6
    } else {                        // <15kHz
        0.6 + ((0.16 - normalized) / 0.16) * 0.4 // 0.6–1.0
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
