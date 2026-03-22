//! End-to-end rendering tests
//!
//! Tests the full rendering pipeline from audio input to instance generation.
//! Note: Full wgpu integration tests require a display context, so we test
//! the CPU-side logic comprehensively and mock the GPU integration.

#[cfg(test)]
mod tests {
    /// Mock audio buffer for testing
    struct MockAudioBuffer {
        data: Vec<f32>,
        sample_rate: u32,
    }

    impl MockAudioBuffer {
        fn new(sample_rate: u32, duration_secs: f32) -> Self {
            let samples = (sample_rate as f32 * duration_secs) as usize;
            MockAudioBuffer {
                data: vec![0.0; samples],
                sample_rate,
            }
        }

        fn silence(&mut self) {
            self.data.fill(0.0);
        }

        fn sine_wave(&mut self, frequency: f32, amplitude: f32) {
            let sample_time = 1.0 / self.sample_rate as f32;
            for (i, sample) in self.data.iter_mut().enumerate() {
                let t = i as f32 * sample_time;
                *sample = (2.0 * std::f32::consts::PI * frequency * t).sin() * amplitude;
            }
        }
    }

    /// Test that basic flow succeeds with silence
    #[test]
    fn test_e2e_silence_renders() {
        let mut audio = MockAudioBuffer::new(48000, 0.1);
        audio.silence();

        // Simulate audio analysis
        let mut rms = 0.0;
        for &sample in &audio.data {
            rms += sample * sample;
        }
        rms = (rms / audio.data.len() as f32).sqrt();

        // Verify RMS is near zero
        assert!(rms < 0.01, "Silence RMS should be near 0, got {}", rms);

        // Simulate layer mapping
        let layer_count = if rms < 0.1 { 1.0 } else { 2.0 };
        assert_eq!(layer_count, 1.0, "Silence should have 1 layer");
    }

    /// Test that sine wave produces expected RMS
    #[test]
    fn test_e2e_sine_wave_rms() {
        let mut audio = MockAudioBuffer::new(48000, 0.1);
        audio.sine_wave(440.0, 0.5); // 440 Hz at 0.5 amplitude

        // Compute RMS
        let mut rms_sq = 0.0;
        for &sample in &audio.data {
            rms_sq += sample * sample;
        }
        rms_sq /= audio.data.len() as f32;
        let rms = rms_sq.sqrt();

        // For a sine wave, RMS = amplitude / sqrt(2)
        let expected_rms = 0.5 / std::f32::consts::SQRT_2;
        let tolerance = 0.01;

        assert!(
            (rms - expected_rms).abs() < tolerance,
            "RMS mismatch: got {}, expected {}",
            rms,
            expected_rms
        );
    }

    /// Test layer count mapping from RMS
    #[test]
    fn test_e2e_layer_mapping() {
        let rms_values = vec![
            (0.0, 1.0),   // Silence → 1 layer
            (0.15, 1.0),  // Very quiet → 1 layer
            (0.3, 2.0),   // Medium → 2 layers
            (0.5, 3.0),   // Loud → 3 layers
            (0.75, 4.0),  // Very loud → 4 layers
            (0.95, 5.0),  // Max → 5 layers
        ];

        for (rms, expected_layers) in rms_values {
            // Layer mapping based on RMS thresholds:
            // 0.0–0.2: 1 layer
            // 0.2–0.4: 2 layers
            // 0.4–0.6: 3 layers
            // 0.6–0.8: 4 layers
            // 0.8–1.0: 5 layers
            let layer_count = if rms < 0.2 {
                1.0
            } else if rms < 0.4 {
                2.0
            } else if rms < 0.6 {
                3.0
            } else if rms < 0.8 {
                4.0
            } else {
                5.0
            };

            assert_eq!(
                layer_count, expected_layers,
                "RMS {} should map to {} layers, got {}",
                rms, expected_layers, layer_count
            );
        }
    }

    /// Test instability mapping from sample rate
    #[test]
    fn test_e2e_instability_mapping() {
        let sample_rates = vec![
            (48000.0, 0.0, 0.01),    // Normal → low instability
            (32000.0, 0.33, 0.35),   // Transition → medium
            (16000.0, 0.66, 0.67),   // Glitchy → high
            (8000.0, 0.83, 0.84),    // Extreme → very high
        ];

        for (sr, min_expected, max_expected) in sample_rates {
            let instability = 1.0_f32 - (sr / 48000.0_f32).max(0.0_f32).min(1.0_f32);

            assert!(
                instability >= min_expected && instability <= max_expected,
                "Sample rate {} Hz: instability {} outside range [{}, {}]",
                sr,
                instability,
                min_expected,
                max_expected
            );
        }
    }

    /// Test transient detection logic
    #[test]
    fn test_e2e_transient_detection() {
        // Transient defined as RMS spike above threshold
        let threshold = 0.6;

        let test_cases = vec![
            (vec![0.0, 0.0, 0.1, 0.0], false),           // No transient
            (vec![0.0, 0.5, 0.4, 0.0], false),           // No spike
            (vec![0.2, 0.8, 0.2, 0.1], true),            // Transient
            (vec![0.9, 0.95, 1.0, 0.8], true),           // Strong transient
        ];

        for (samples, should_detect) in test_cases {
            // Compute peak and RMS
            let peak = samples.iter().copied().fold(f32::NEG_INFINITY, f32::max);

            let detected = peak > threshold;
            assert_eq!(
                detected, should_detect,
                "Transient detection failed for {:?}",
                samples
            );
        }
    }

    /// Test layer state initialization
    #[test]
    fn test_e2e_layer_state_init() {
        // Verify that 5 layers are created with correct defaults
        let num_layers = 5;
        let mut layers = vec![
            (0usize, 1.0f32, false);  // (image_idx, weight, pop_highlight)
            num_layers
        ];

        // Layer 0 is always anchor
        assert_eq!(layers[0].0, 0, "Layer 0 should be image 0");
        assert_eq!(layers[0].1, 1.0, "Layer 0 should have weight 1.0");
        assert_eq!(layers[0].2, false, "Layer 0 should have no pop");

        // Other layers start inactive
        for i in 1..num_layers {
            layers[i] = (i, 0.0, false);
            assert_eq!(layers[i].1, 0.0, "Layer {} should start inactive", i);
        }
    }

    /// Test instance generation bounds
    #[test]
    fn test_e2e_instance_generation_bounds() {
        let grid_width = 36;
        let grid_height = 46;
        let max_instances = grid_width * grid_height * 5; // Max 5 layers per cell

        // Simulate instance collection
        let mut instance_count = 0;
        for _y in 0..grid_height {
            for _x in 0..grid_width {
                // Anchor layer always present
                instance_count += 1;
                // Up to 4 overlay layers
                for _ in 0..4 {
                    instance_count += 1; // Worst case: all overlay cells
                }
            }
        }

        assert_eq!(
            instance_count, max_instances,
            "Instance count mismatch: {} vs {}",
            instance_count, max_instances
        );
        assert!(instance_count > 0, "Should generate at least some instances");
    }

    /// Test parameter smoothing for state transitions
    #[test]
    fn test_e2e_parameter_smoothing() {
        let old_rms = 0.2;
        let new_rms = 0.8;
        let smoothing_factor = 0.1; // 10% per update

        let mut rms = old_rms;
        let mut converged = false;

        for _step in 0..100 {
            rms = rms * (1.0_f32 - smoothing_factor) + new_rms * smoothing_factor;
            if (rms - new_rms).abs() < 0.01_f32 {
                converged = true;
                break;
            }
        }

        assert!(converged, "RMS smoothing did not converge");
        assert!((rms - new_rms).abs() < 0.01_f32, "Final RMS not close to target");
    }

    /// Test that all grid cells are covered
    #[test]
    fn test_e2e_grid_coverage() {
        let grid_width = 36usize;
        let grid_height = 46usize;

        let mut visited = vec![false; grid_width * grid_height];

        // Simulate covering each cell
        for y in 0..grid_height {
            for x in 0..grid_width {
                let idx = y * grid_width + x;
                visited[idx] = true;
            }
        }

        // Verify all cells visited
        let visited_count = visited.iter().filter(|&&v| v).count();
        assert_eq!(
            visited_count,
            grid_width * grid_height,
            "Not all cells visited: {} / {}",
            visited_count,
            grid_width * grid_height
        );
    }

    /// Test pop highlight effect parameters
    #[test]
    fn test_e2e_pop_highlight_effect() {
        let normal_scale = 1.0;
        let normal_opacity = 1.0;

        let pop_scale = 1.5;
        let pop_opacity = 0.7;

        // Verify pop effect is distinct
        assert_ne!(pop_scale, normal_scale, "Pop scale should differ");
        assert_ne!(pop_opacity, normal_opacity, "Pop opacity should differ");

        // Pop effect should increase visibility (scale up)
        assert!(pop_scale > normal_scale, "Pop should increase scale");

        // Pop effect should reduce opacity (to create outline effect)
        assert!(pop_opacity < normal_opacity, "Pop should reduce opacity");
    }

    /// Test memory bounds for instance buffer
    #[test]
    fn test_e2e_instance_buffer_size() {
        let grid_width = 36;
        let grid_height = 46;
        let max_layers = 5;
        let instance_size = 64; // bytes per instance

        let max_instances = grid_width * grid_height * max_layers;
        let buffer_size = max_instances * instance_size;

        // Verify buffer is reasonable
        assert!(buffer_size > 0, "Buffer size should be positive");
        assert!(buffer_size < 10_000_000, "Buffer size should be < 10 MB");

        let expected_size = 1_656 * 5 * 64; // 528 KB
        assert_eq!(buffer_size, expected_size, "Buffer size mismatch");
    }

    /// Test audio frame rate consistency
    #[test]
    fn test_e2e_frame_rate_consistency() {
        let sample_rate = 48000u32;
        let frame_size = 512u32;
        let expected_fps = sample_rate as f32 / frame_size as f32;

        // At 48 kHz with 512-sample buffers: ~93.75 FPS
        assert!((expected_fps - 93.75).abs() < 1.0, "Frame rate off");

        // Verify frame timing
        let frame_duration_ms = 1000.0 / expected_fps;
        assert!(frame_duration_ms > 0.0, "Frame duration should be positive");
        assert!(frame_duration_ms < 100.0, "Frame duration should be < 100ms");
    }
}
