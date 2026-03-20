# Rendering Pipeline

## Overview

The rendering system converts audio input into a dynamic ASCII art visualization using GPU acceleration (wgpu). The pipeline consists of five main stages:

1. **Audio Analysis** → Extract RMS, layer count, instability, transients
2. **Layer Engine** → Manage 5 overlaid ASCII images with blending
3. **Instance Generation** → Compute per-glyph transforms and colors
4. **GPU Upload** → Write instance data to GPU buffers
5. **Render Pass** → Execute GPU shader, output to target texture

## Architecture

### Input Flow

```
AudioBuffer (48 kHz, 2 channels)
    ↓
AudioAnalyzer::update()
    ├─ Compute RMS (0.0–1.0)
    ├─ Map to layer_count (1.0–5.0)
    ├─ Map to instability (0.0–1.0)
    └─ Detect transients (boolean)
    ↓
LayerEngine::update()
    ├─ Layer 0: anchor (img01.txt, always full opacity)
    ├─ Layers 1–4: overlays with pop highlights
    └─ Return 5 × LayerState
    ↓
generate_instances()
    ├─ Iterate grid (36×46)
    ├─ Composite layers at each cell
    ├─ Apply spatial offsets
    └─ Return Vec<GlyphInstance>
    ↓
AsciiRenderer::render()
    ├─ Upload instances to GPU buffer
    ├─ Begin render pass
    ├─ Execute shader
    └─ Output to texture
```

## Component Details

### Audio Analyzer (`src/render/audio_analysis.rs`)

Processes incoming audio frames to extract analysis parameters.

**Input:**
- `audio_data: &[f32]` — interleaved or mono samples
- `rms_weight: f32` — smoothing factor (0.0–1.0, typically 0.1)

**Output:**
```rust
pub struct AudioAnalysis {
    pub rms: f32,           // Energy level (0.0–1.0)
    pub peak: f32,          // Absolute peak in frame
    pub zero_crossings: u32, // Transient indicator
}
```

**Algorithm:**
1. Compute RMS: `sqrt(sum(sample^2) / len)`
2. Smooth with exponential moving average: `rms_new = rms_old * (1-α) + rms_frame * α`
3. Count zero crossings to detect transients

**Mapping:**
- RMS 0.0–0.2: silence (layer_count = 1, instability = 0.0)
- RMS 0.2–0.6: normal (layer_count scales, instability ≈ 0.3–0.5)
- RMS 0.6–0.8: active (layer_count → 3–4, instability ≈ 0.6–0.8)
- RMS 0.8–1.0: extreme (layer_count → 5, instability ≈ 1.0)

### Layer Engine (`src/render/layer_engine.rs`)

Manages 5 active layers, each with position, weight, and effects.

**Layer 0 (Anchor):**
- Always references `img01.txt` (36×46 base grid)
- Weight = 1.0 (full opacity)
- No transformation
- Provides stable visual foundation

**Layers 1–4 (Overlays):**
- Random overlay images (from `AsciiBank[1..N]`)
- Weight = `1.0 / (active_count * 2.0)`, clamped to 0.4
- Spatial offset: random (±5 grid cells)
- `pop_highlight = true`: replaces spaces with 1.5x scale, 70% opacity

**Update Logic:**
```
Given: RMS, layer_count, instability, transient_active

1. Layer 0: always anchor
2. For i in 1..ceil(layer_count):
   - Select random overlay image
   - Set weight based on layer count
   - Randomize spatial offset
   - Enable pop highlight
3. Inactive layers (i >= ceil(layer_count)): weight = 0.0
4. On transient: spawn temporary spike layer (weight = 0.5, pop = true)
```

### Instance Generation (`src/render/instancing.rs`)

Converts layer states into per-glyph render commands.

**GlyphInstance Structure (64 bytes, GPU-aligned):**
```rust
pub struct GlyphInstance {
    pub position: [i32; 2],      // Grid cell (x, y)
    pub glyph_idx: u32,          // ASCII code (0–127)
    pub color: [f32; 4],         // RGBA (0.0–1.0)
    pub scale: f32,              // Transform scale (1.0 or 1.5 for pop)
    pub opacity: f32,            // Blend alpha (0.0–1.0)
    pub time_offset: i32,        // Animation phase (frames)
    pub _pad: [f32; 2],          // Padding
}
```

**Generation Algorithm:**
```
For each grid cell (x, y) in 0..36 × 0..46:
  1. Layer 0 (anchor):
     - Always emit instance (position, glyph, opacity=1.0, scale=1.0)

  2. Layers 1–4 (overlays):
     - For each active layer:
       - Fetch glyph at (x + offset_x, y + offset_y)
       - If glyph is space AND pop_highlight = false: skip
       - Otherwise: emit instance (scale=1.5, opacity=0.7 if pop)
```

**Spatial Mapping:**
- Source image may be larger than 36×46 grid
- Spatial offset simulates camera pan (±5 cells)
- Out-of-bounds reads return `None` (skipped)

### Renderer (`src/render/ascii_render.rs`)

GPU-based rasterization using wgpu.

**Initialization (`AsciiRenderer::new`):**
1. Create glyph atlas texture (10 glyphs/row, 16px cells)
2. Initialize sampler (linear filtering, edge clamp)
3. Create bind group (atlas texture + sampler)
4. Allocate GPU buffers:
   - Vertex buffer: grid quad vertices
   - Instance buffer: `Vec<GlyphInstance>` (uploaded each frame)
   - Index buffer: quad indices (0, 1, 2, 1, 3, 2)

**Render Pass (`AsciiRenderer::render`):**
1. Generate instances for current frame
2. Upload instance data to GPU buffer: `queue.write_buffer()`
3. Create render encoder
4. Begin render pass (clear to black, store output)
5. Bind atlas texture + sampler
6. Draw instanced quad (6 indices × instance count)
7. Submit command buffer

**Shader (implicit in wgpu pipeline):**
- Vertex shader: transform instance position + apply scale
- Fragment shader: sample glyph from atlas, apply color, opacity blend

## Performance Considerations

### Frame Budget
- Target: 60 FPS → 16.67 ms per frame
- Audio analysis: ~1 ms (RMS, smoothing)
- Layer update: ~0.5 ms (random selection, state management)
- Instance generation: ~2–3 ms (1,656 cells × 5 layers max)
- GPU submission: ~1 ms (buffer upload, render pass)
- **Total: ~5 ms, leaving 11 ms buffer for GPU rendering**

### Memory Layout

**Instance Buffer:** 1,656 cells × 5 max layers × 64 bytes/instance = 528 KB

**GPU Texture:** Atlas (160×144 px × 4 bytes) = 92 KB

**Stack:** Temporary instance Vec during generation (~200 KB typical)

### Optimization Strategies

1. **Early exit:** Skip zero-weight layers entirely
2. **Spatial bounds:** Check offsets before fetching glyphs
3. **Reuse indices:** Quad index buffer never changes
4. **Batch upload:** Single `write_buffer()` call per frame
5. **Ring buffers:** Future: cycle 3 instance buffers to hide GPU stall

## Testing

### Unit Tests
- `tests/test_parameter_mapping.rs`: Validates all parameter ranges and mappings
- `src/render/instancing.rs`: Tests glyph indexing and instance size

### Integration Tests
- `tests/test_rendering_e2e.rs`: End-to-end flow with mock audio

### Validation
- `cargo test --lib`: Run all lib tests (instancing tests included)
- `cargo test --test test_parameter_mapping`: Parameter validation
- `cargo test --test test_rendering_e2e`: Full flow validation

## Debugging

### GPU Validation
Enable wgpu validation in debug builds:
```rust
let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
    backends: wgpu::Backends::PRIMARY,
    dx12_shader_compiler: Default::default(),
    flags: wgpu::InstanceFlags::validation(),  // ← Enable
});
```

### Logging
Each module outputs key metrics:
```
[AudioAnalyzer] RMS: 0.45 → layer_count: 2.3 instability: 0.55
[LayerEngine] Active layers: 2 (anchor + 1 overlay)
[Instancing] Generated 847 instances
[AsciiRenderer] Uploading 847 instances (54 KB)
```

### Common Issues
1. **GPU buffer overflow:** Check instance count ≠ buffer size
2. **Texture binding:** Verify atlas dimensions match shader expectations
3. **Out-of-bounds reads:** Layer offsets must clamp to grid bounds
4. **Performance drops:** Profile GPU time with `wgpu::Query` instrumentation

## Future Enhancements

1. **Dynamic atlas:** Update glyph ASCII art each frame based on audio shape
2. **Bloom/blur:** Post-process shader for glow effects
3. **Animation curves:** Smooth pop-in with easing functions
4. **Double-buffering:** Reduce GPU stalls with triple-buffering
5. **Compute shader:** Parallelize instance generation on GPU
6. **Tiling:** Support arbitrary output resolutions beyond 36×46
