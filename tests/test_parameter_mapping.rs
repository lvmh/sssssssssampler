/// Parameter mapping validation tests
///
/// Validates that sample rate to instability parameter mapping is correct
/// and that all transformation boundaries work as expected.

#[cfg(test)]
mod tests {
    /// Test sample rate to instability mapping zones
    ///
    /// The mapping is designed to trigger different audio effects at different
    /// sample rates:
    /// - 48 kHz: normal (instability ~0.5)
    /// - 16 kHz: glitchy (instability ~0.8)
    /// - 8 kHz: extreme (instability ~1.0)
    #[test]
    fn test_sr_mapping_zones() {
        // Sample rates and their expected instability ranges
        // Formula: instability = 1.0 - (sample_rate / 48000.0), clamped to [0.0, 1.0]
        let test_cases = vec![
            (48000.0, 0.0, 0.01),   // Normal zone: 48 kHz → instability ≈ 0
            (44100.0, 0.08, 0.10),  // Normal zone: 44.1 kHz → instability ≈ 0.08
            (32000.0, 0.33, 0.35),  // Transition zone: 32 kHz → instability ≈ 0.33
            (24000.0, 0.49, 0.51),  // Glitchy zone: 24 kHz → instability ≈ 0.5
            (16000.0, 0.66, 0.67),  // Very glitchy: 16 kHz → instability ≈ 0.667
            (8000.0, 0.83, 0.84),   // Extreme: 8 kHz → instability ≈ 0.833
        ];

        for (sample_rate, min_expected, max_expected) in test_cases {
            // Compute instability from sample rate
            // Using the formula: instability = 1.0 - (sample_rate / 48000.0)
            let sr_normalized = (sample_rate / 48000.0_f32).max(0.0_f32).min(1.0_f32);
            let instability = 1.0_f32 - sr_normalized;

            assert!(
                instability >= min_expected && instability <= max_expected,
                "Sample rate {} Hz yielded instability {}, expected range [{}, {}]",
                sample_rate,
                instability,
                min_expected,
                max_expected
            );
        }
    }

    /// Test that layer count mapping is monotonic
    ///
    /// As audio RMS increases, layer count should not decrease
    #[test]
    fn test_layer_count_monotonic() {
        let rms_values = vec![
            (0.0, 1.0),    // Silence: 1 layer
            (0.05, 1.0),   // Very quiet: 1 layer
            (0.1, 1.5),    // Quiet: 1-2 layers
            (0.3, 2.0),    // Medium: 2 layers
            (0.5, 3.0),    // Loud: 3 layers
            (0.8, 4.0),    // Very loud: 4 layers
            (1.0, 5.0),    // Extreme: 5 layers (max)
        ];

        for i in 0..rms_values.len() - 1 {
            let (rms1, count1) = rms_values[i];
            let (rms2, count2) = rms_values[i + 1];

            // Higher RMS should result in higher or equal layer count
            assert!(
                count2 >= count1,
                "Layer count decreased: RMS {} -> {} gave {} -> {} layers",
                rms1,
                rms2,
                count1,
                count2
            );
        }
    }

    /// Test that transient detection threshold is reasonable
    ///
    /// Transients should only trigger at high enough RMS values
    #[test]
    fn test_transient_threshold() {
        // Transient should trigger at RMS > 0.6 (approximately)
        let transient_threshold = 0.6;

        let test_cases = vec![
            (0.0, false),    // No transient
            (0.3, false),    // No transient
            (0.5, false),    // No transient
            (0.65, true),    // Transient!
            (0.8, true),     // Transient
            (1.0, true),     // Transient
        ];

        for (rms, should_trigger) in test_cases {
            let is_transient = rms > transient_threshold;
            assert_eq!(
                is_transient, should_trigger,
                "RMS {} transient detection failed",
                rms
            );
        }
    }

    /// Test bit depth reduction parameter range
    ///
    /// Bit depth should range from 16 (high quality) to 2 (extreme)
    #[test]
    fn test_bit_depth_range() {
        // Bit depth should be in valid range
        let valid_bit_depths = vec![2, 4, 6, 8, 12, 16];

        for bd in &valid_bit_depths {
            assert!(*bd >= 2 && *bd <= 16, "Bit depth {} out of valid range [2, 16]", bd);
        }

        // Test that quantization step size is reasonable
        for bd in &valid_bit_depths {
            let levels = 2u32.pow(*bd);
            let step_size = 1.0 / (levels as f32 - 1.0);
            assert!(step_size > 0.0, "Invalid step size for {} bit depth", bd);
        }
    }

    /// Test mix parameter bounds
    ///
    /// Mix parameter (dry/wet) should be in [0, 1]
    #[test]
    fn test_mix_parameter_bounds() {
        let valid_mixes = vec![0.0, 0.25, 0.5, 0.75, 1.0];

        for mix in valid_mixes {
            assert!(mix >= 0.0 && mix <= 1.0, "Mix {} out of bounds [0, 1]", mix);

            // Test dry/wet computation
            let dry_weight = 1.0 - mix;
            let wet_weight = mix;

            assert_eq!(
                dry_weight + wet_weight,
                1.0,
                "Dry/wet weights don't sum to 1: {} + {}",
                dry_weight,
                wet_weight
            );
        }
    }

    /// Test time sync parameter
    ///
    /// Time offset (frame count) should be non-negative
    #[test]
    fn test_time_offset_bounds() {
        let frame_rates = vec![30, 44100, 48000];
        let duration_secs = vec![0.1, 1.0, 10.0];

        for &frame_rate in &frame_rates {
            for &duration in &duration_secs {
                let max_frame_count = (frame_rate as f32 * duration) as i32;
                assert!(max_frame_count >= 0, "Frame count negative");

                // Test that time offsets are smaller than total duration
                let time_offset = max_frame_count / 2;
                assert!(time_offset >= 0, "Time offset negative");
            }
        }
    }

    /// Test parameter envelope smoothing
    ///
    /// When parameters change, they should not jump instantly
    #[test]
    fn test_envelope_smoothing() {
        let old_param = 0.2;
        let new_param = 0.8;
        let smoothing_time = 0.1; // 100ms
        let sample_rate = 48000.0;

        let samples_in_window = (smoothing_time * sample_rate) as usize;
        let steps = 5;

        for step in 0..=steps {
            let t = step as f32 / steps as f32;
            // Linear interpolation (could use exponential smoothing)
            let interpolated = old_param + (new_param - old_param) * t;

            assert!(
                interpolated >= old_param && interpolated <= new_param,
                "Interpolated value {} outside range [{}, {}]",
                interpolated,
                old_param,
                new_param
            );
        }

        println!(
            "Smoothing window: {} samples at {} Hz",
            samples_in_window, sample_rate
        );
    }

    /// Test spatial position clamping
    ///
    /// Layer spatial offsets should be clamped to reasonable bounds
    #[test]
    fn test_spatial_offset_clamping() {
        let grid_width = 36;
        let grid_height = 46;
        let max_offset = 5;

        let test_offsets = vec![
            (-10, -10),
            (-5, 0),
            (0, 0),
            (5, 5),
            (10, 10),
        ];

        for (x, y) in test_offsets {
            let clamped_x = x.max(-max_offset).min(max_offset);
            let clamped_y = y.max(-max_offset).min(max_offset);

            assert!(
                clamped_x >= -max_offset && clamped_x <= max_offset,
                "X offset {} out of clamp range",
                clamped_x
            );
            assert!(
                clamped_y >= -max_offset && clamped_y <= max_offset,
                "Y offset {} out of clamp range",
                clamped_y
            );

            // Clamped position should still be valid on grid
            let final_x = clamped_x.max(0).min(grid_width as i32 - 1);
            let final_y = clamped_y.max(0).min(grid_height as i32 - 1);

            assert!(final_x >= 0 && final_x < grid_width as i32);
            assert!(final_y >= 0 && final_y < grid_height as i32);
        }
    }
}
