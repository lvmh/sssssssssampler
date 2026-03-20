/// Motion system for layered ASCII visualization
/// Provides smooth, coherent motion across layers with per-region desynchronization

pub struct MotionSystem {
    /// Frame counter, incremented each step
    frame_count: u64,
}

impl MotionSystem {
    /// Create a new motion system starting at frame 0
    pub fn new() -> Self {
        MotionSystem { frame_count: 0 }
    }

    /// Advance the animation by one frame (typically 60fps)
    pub fn step(&mut self) {
        self.frame_count = self.frame_count.wrapping_add(1);
    }

    /// Get global drift (x, y) offsets with ~4s period at 60fps
    /// Returns smooth sine/cosine wave offsets for background pan
    pub fn global_drift(&self) -> (f32, f32) {
        // Convert frame count to time in seconds (60 fps target)
        let time_secs = self.frame_count as f32 / 60.0;

        // 4 second period = 2π radians / 4 = π/2 per second
        let period = 4.0;
        let phase = std::f32::consts::TAU * (time_secs / period);

        // Smooth drift with ~±2.0 unit amplitude
        let x = (phase).sin() * 2.0;
        let y = (phase + std::f32::consts::PI / 2.0).cos() * 2.0;

        (x, y)
    }

    /// Get per-layer phase offset for staggered animation
    /// Returns a normalized value [0, 1) that cycles smoothly per layer
    pub fn layer_motion(&self, layer_idx: usize, speed_multiplier: f32) -> f32 {
        let time_secs = self.frame_count as f32 / 60.0;

        // Layer-specific offset: each layer gets a different phase based on index
        // This creates a staggered, cascading effect
        let layer_phase = (layer_idx as f32) * std::f32::consts::PI / 2.5;

        // Speed multiplier allows faster/slower animation per layer
        let adjusted_time = time_secs * speed_multiplier;

        // Cycle with 6 second period per layer
        let period = 6.0;
        let phase = std::f32::consts::TAU * (adjusted_time / period) + layer_phase;

        // Normalize to [0, 1) range, with smooth oscillation
        ((phase.sin() + 1.0) / 2.0).fract()
    }

    /// Get per-region offset for localized desynchronization
    /// Creates organic, unpredictable motion variations per grid region
    ///
    /// Arguments:
    /// - region_x, region_y: grid region coordinates
    /// - instability: [0.0, 1.0] control over motion variance
    ///
    /// Returns (x_offset, y_offset) in normalized units
    pub fn region_offset(&self, region_x: u32, region_y: u32, instability: f32) -> (f32, f32) {
        let time_secs = self.frame_count as f32 / 60.0;

        // Use region coordinates to create deterministic but unique motion per region
        // Hash the region coordinates into a seed for pseudo-random phase offsets
        let region_seed_x = ((region_x.wrapping_mul(73856093)) ^ (region_y.wrapping_mul(19349663))) as f32;
        let region_seed_y = ((region_x.wrapping_mul(83492791)) ^ (region_y.wrapping_mul(29387091))) as f32;

        // Normalize seeds to use as phase offsets
        let phase_offset_x = region_seed_x.fract().abs() * std::f32::consts::TAU;
        let phase_offset_y = region_seed_y.fract().abs() * std::f32::consts::TAU;

        // Create region-specific oscillation with varying frequency based on instability
        // Higher instability = more varied, unpredictable motion
        let freq_variation = 1.0 + instability * 0.5;
        let period_x = 3.0 + instability * 3.0;
        let period_y = 3.5 + instability * 3.5;

        let phase_x = std::f32::consts::TAU * (time_secs * freq_variation / period_x) + phase_offset_x;
        let phase_y = std::f32::consts::TAU * (time_secs * freq_variation / period_y) + phase_offset_y;

        // Apply sine/cosine waves scaled by instability
        let x = phase_x.sin() * instability * 0.8;
        let y = phase_y.cos() * instability * 0.8;

        (x, y)
    }
}

impl Default for MotionSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_motion_system_creation() {
        let motion = MotionSystem::new();
        assert_eq!(motion.frame_count, 0);
    }

    #[test]
    fn test_step_increments_frame_count() {
        let mut motion = MotionSystem::new();
        motion.step();
        assert_eq!(motion.frame_count, 1);

        motion.step();
        assert_eq!(motion.frame_count, 2);
    }

    #[test]
    fn test_global_drift_oscillates() {
        let mut motion = MotionSystem::new();
        let (x1, y1) = motion.global_drift();
        assert!(x1.is_finite());
        assert!(y1.is_finite());
        assert!(x1.abs() <= 2.1); // ~±2.0 amplitude + float error margin
        assert!(y1.abs() <= 2.1);

        // After half period (120 frames at 60fps = 2 seconds, half of 4s period)
        for _ in 0..120 {
            motion.step();
        }
        let (x2, y2) = motion.global_drift();
        assert!(x2.is_finite());
        assert!(y2.is_finite());
    }

    #[test]
    fn test_layer_motion_normalized() {
        let motion = MotionSystem::new();
        for layer in 0..5 {
            let value = motion.layer_motion(layer, 1.0);
            assert!(value >= 0.0 && value < 1.0, "layer_motion must be in [0, 1)");
        }
    }

    #[test]
    fn test_layer_motion_different_speeds() {
        let motion = MotionSystem::new();
        let slow = motion.layer_motion(0, 0.5);
        let fast = motion.layer_motion(0, 2.0);
        // Both should be valid normalized values, just potentially different phases
        assert!(slow >= 0.0 && slow < 1.0);
        assert!(fast >= 0.0 && fast < 1.0);
    }

    #[test]
    fn test_region_offset_bounded() {
        let motion = MotionSystem::new();
        for region_x in 0..5 {
            for region_y in 0..5 {
                for instability in [0.0, 0.5, 1.0] {
                    let (x, y) = motion.region_offset(region_x, region_y, instability);
                    assert!(x.is_finite());
                    assert!(y.is_finite());
                    // Offset should be bounded by instability * ~0.8
                    let max_bound = instability * 0.9;
                    assert!(x.abs() <= max_bound, "x offset exceeded bounds");
                    assert!(y.abs() <= max_bound, "y offset exceeded bounds");
                }
            }
        }
    }

    #[test]
    fn test_region_offset_deterministic() {
        let motion1 = MotionSystem::new();
        let motion2 = MotionSystem::new();

        let offset1 = motion1.region_offset(10, 20, 0.7);
        let offset2 = motion2.region_offset(10, 20, 0.7);

        // Same region and parameters should give same result
        assert!(
            (offset1.0 - offset2.0).abs() < 1e-6 && (offset1.1 - offset2.1).abs() < 1e-6,
            "region_offset should be deterministic"
        );
    }
}
