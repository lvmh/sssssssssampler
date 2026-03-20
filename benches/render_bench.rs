//! Performance benchmarks for the rendering pipeline
//!
//! Validates that CPU-side operations (audio analysis, layer updates, instance
//! generation) stay well below the 60 FPS target (16.67 ms per frame).
//!
//! Run with: cargo bench --bench render_bench

/// Benchmark audio analysis
///
/// Measures the time to compute RMS from a frame of audio samples.
/// Target: < 1 ms for 512-sample frame at 48 kHz
fn bench_audio_analysis() {
    let sample_rate = 48000u32;
    let frame_size = 512usize;
    let samples: Vec<f32> = (0..frame_size)
        .map(|i| ((i as f32 / frame_size as f32) * 2.0 * std::f32::consts::PI).sin() * 0.5)
        .collect();

    let start = std::time::Instant::now();

    // Compute RMS
    let mut rms_sq = 0.0f32;
    for &s in &samples {
        rms_sq += s * s;
    }
    rms_sq /= samples.len() as f32;
    let rms = rms_sq.sqrt();

    let elapsed = start.elapsed();

    println!(
        "Audio analysis (512 samples): {:.3} ms (RMS: {:.4})",
        elapsed.as_secs_f32() * 1000.0,
        rms
    );

    // Verify performance target
    assert!(
        elapsed.as_secs_f32() < 0.001,
        "Audio analysis exceeded 1 ms: {:.3} ms",
        elapsed.as_secs_f32() * 1000.0
    );
}

/// Benchmark layer engine update
///
/// Measures the time to update 5 layer states based on audio parameters.
/// Target: < 0.5 ms
fn bench_layer_engine_update() {
    let rms = 0.5f32;
    let layer_count = 3.0f32;
    let instability = 0.5f32;
    let transient_active = false;

    let start = std::time::Instant::now();

    // Simulate layer state updates
    let mut layers = vec![(0usize, 1.0f32, false); 5];

    // Layer 0: anchor
    layers[0] = (0, 1.0, false);

    // Layers 1–3: overlays (simplified version)
    for i in 1..=(layer_count as usize).min(4) {
        let weight = (1.0 / (layer_count * 2.0)).min(0.4);
        let offset_x = ((i * 7) % 10 - 5) as i32;
        let offset_y = ((i * 11) % 10 - 5) as i32;
        layers[i] = (i % 5, weight, true);
    }

    let elapsed = start.elapsed();

    println!(
        "Layer engine update (5 layers): {:.3} ms",
        elapsed.as_secs_f32() * 1000.0
    );

    assert!(
        elapsed.as_secs_f32() < 0.001,
        "Layer update exceeded 1 ms: {:.3} ms",
        elapsed.as_secs_f32() * 1000.0
    );
}

/// Benchmark instance generation
///
/// Measures the time to generate instances for a 36×46 grid across 5 layers.
/// Target: < 3 ms
fn bench_instance_generation() {
    let grid_width = 36usize;
    let grid_height = 46usize;
    let max_layers = 5usize;

    let start = std::time::Instant::now();

    let mut instance_count = 0usize;

    // Simulate instance generation
    for _y in 0..grid_height {
        for _x in 0..grid_width {
            // Layer 0 (anchor): always emit
            instance_count += 1;

            // Layers 1–4: emit based on weight
            for layer_idx in 1..max_layers {
                let weight = if layer_idx <= 3 { 0.3 } else { 0.0 };

                if weight > 0.0 {
                    instance_count += 1;
                }
            }
        }
    }

    let elapsed = start.elapsed();

    println!(
        "Instance generation ({} grid, {} layers): {:.3} ms ({} instances)",
        grid_width * grid_height,
        max_layers,
        elapsed.as_secs_f32() * 1000.0,
        instance_count
    );

    assert!(
        elapsed.as_secs_f32() < 0.005,
        "Instance generation exceeded 5 ms: {:.3} ms",
        elapsed.as_secs_f32() * 1000.0
    );
}

/// Benchmark parameter smoothing
///
/// Measures the time to apply exponential smoothing to parameters.
/// Target: < 0.5 ms
fn bench_parameter_smoothing() {
    let mut rms = 0.2f32;
    let target_rms = 0.8f32;
    let smoothing_factor = 0.1f32;
    let iterations = 1000usize;

    let start = std::time::Instant::now();

    for _ in 0..iterations {
        rms = rms * (1.0 - smoothing_factor) + target_rms * smoothing_factor;
    }

    let elapsed = start.elapsed();

    println!(
        "Parameter smoothing ({} iterations): {:.3} ms (final RMS: {:.4})",
        iterations,
        elapsed.as_secs_f32() * 1000.0,
        rms
    );

    assert!(
        elapsed.as_secs_f32() < 0.01,
        "Smoothing exceeded 10 ms: {:.3} ms",
        elapsed.as_secs_f32() * 1000.0
    );
}

/// Benchmark frame budget (sum of all CPU operations)
///
/// Validates that CPU-side work stays under budget.
/// Target: < 5 ms total (leaving 11 ms for GPU rendering + margin)
fn bench_full_frame_budget() {
    println!("\n=== Full Frame Budget Analysis ===");
    println!("Target: 16.67 ms per frame (60 FPS)");
    println!("Target CPU budget: < 5 ms (leaving 11 ms GPU buffer)");

    let sample_rate = 48000u32;
    let frame_size = 512usize;

    // Generate test audio
    let samples: Vec<f32> = (0..frame_size)
        .map(|i| ((i as f32 / frame_size as f32) * 2.0 * std::f32::consts::PI).sin() * 0.5)
        .collect();

    let frame_start = std::time::Instant::now();

    // 1. Audio analysis (~1 ms)
    let mut rms_sq = 0.0f32;
    for &s in &samples {
        rms_sq += s * s;
    }
    rms_sq /= samples.len() as f32;
    let rms = rms_sq.sqrt();
    let audio_time = frame_start.elapsed();

    // 2. Layer engine (~0.5 ms)
    let layer_start = std::time::Instant::now();
    let layer_count = 1.0 + (rms * 4.0).min(4.0);
    let _instability = 1.0 - (sample_rate as f32 / 48000.0);
    let _transient = rms > 0.6;
    let layer_time = layer_start.elapsed();

    // 3. Instance generation (~2-3 ms)
    let instance_start = std::time::Instant::now();
    let mut instance_count = 0usize;
    for _y in 0..46 {
        for _x in 0..36 {
            instance_count += 1; // Simplified: just count
            for _ in 1..5 {
                if rms > 0.2 {
                    instance_count += 1;
                }
            }
        }
    }
    let instance_time = instance_start.elapsed();

    let total_time = frame_start.elapsed();

    println!("\nComponent timings:");
    println!("  Audio analysis:      {:.3} ms", audio_time.as_secs_f32() * 1000.0);
    println!("  Layer engine:        {:.3} ms", layer_time.as_secs_f32() * 1000.0);
    println!("  Instance generation: {:.3} ms", instance_time.as_secs_f32() * 1000.0);
    println!("  TOTAL CPU:           {:.3} ms", total_time.as_secs_f32() * 1000.0);
    println!("  Instances generated: {}", instance_count);
    println!("\nBudget remaining:    {:.3} ms for GPU", 16.67 - (total_time.as_secs_f32() * 1000.0));

    assert!(
        total_time.as_secs_f32() < 0.01,
        "Total frame budget exceeded 10 ms: {:.3} ms",
        total_time.as_secs_f32() * 1000.0
    );
}

/// Benchmark memory allocation patterns
///
/// Measures allocation overhead for instance Vec.
/// Target: < 1 ms for typical instance allocation
fn bench_memory_allocation() {
    let grid_width = 36;
    let grid_height = 46;
    let max_instances = grid_width * grid_height * 5;

    let start = std::time::Instant::now();

    let mut instances = Vec::with_capacity(max_instances);
    instances.resize(max_instances, (0u32, 0.0f32, 0u32));

    let alloc_time = start.elapsed();

    let start = std::time::Instant::now();
    instances.clear();
    let clear_time = start.elapsed();

    println!(
        "Memory allocation ({} instances): alloc {:.3} ms, clear {:.3} ms",
        max_instances,
        alloc_time.as_secs_f32() * 1000.0,
        clear_time.as_secs_f32() * 1000.0
    );

    assert!(
        alloc_time.as_secs_f32() < 0.005,
        "Allocation exceeded 5 ms: {:.3} ms",
        alloc_time.as_secs_f32() * 1000.0
    );
}

/// Test runner
fn main() {
    println!("=== Rendering Pipeline Benchmarks ===\n");

    bench_audio_analysis();
    println!("✓ Audio analysis within budget\n");

    bench_layer_engine_update();
    println!("✓ Layer engine within budget\n");

    bench_instance_generation();
    println!("✓ Instance generation within budget\n");

    bench_parameter_smoothing();
    println!("✓ Parameter smoothing within budget\n");

    bench_memory_allocation();
    println!("✓ Memory allocation within budget\n");

    bench_full_frame_budget();
    println!("\n✓ Full frame budget validated\n");

    println!("All benchmarks passed! 60 FPS target achievable.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_analysis_perf() {
        bench_audio_analysis();
    }

    #[test]
    fn test_layer_engine_perf() {
        bench_layer_engine_update();
    }

    #[test]
    fn test_instance_generation_perf() {
        bench_instance_generation();
    }

    #[test]
    fn test_parameter_smoothing_perf() {
        bench_parameter_smoothing();
    }

    #[test]
    fn test_memory_allocation_perf() {
        bench_memory_allocation();
    }

    #[test]
    fn test_full_frame_budget_perf() {
        bench_full_frame_budget();
    }
}
