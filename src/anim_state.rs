// ─── Shared animation state ───────────────────────────────────────────────────
//
// Written by the GUI timer thread; read by the canvas draw call.
// Everything is f32 so copies are atomic enough for our purposes — we wrap in
// Mutex to satisfy Rust's Send/Sync requirements.

use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub struct AnimParams {
    /// 0.0–1.0 mix from the param
    pub mix: f32,
    /// 0.0–1.0 normalised sample rate (0 = 1kHz, 1 = 96kHz)
    pub sample_rate_norm: f32,
    /// 0.0–1.0 bit depth (0 = 1-bit, 1 = 24-bit)
    pub bit_depth_norm: f32,
    /// 0.0–1.0 jitter
    pub jitter: f32,
    /// 0.0–1.0 filter cutoff normalised
    pub filter_cutoff_norm: f32,
}

impl Default for AnimParams {
    fn default() -> Self {
        Self {
            mix: 1.0,
            sample_rate_norm: 0.27,
            bit_depth_norm: 0.5,
            jitter: 0.0,
            filter_cutoff_norm: 0.9,
        }
    }
}

pub type SharedAnimParams = Arc<Mutex<AnimParams>>;

pub fn new_shared() -> SharedAnimParams {
    Arc::new(Mutex::new(AnimParams::default()))
}
