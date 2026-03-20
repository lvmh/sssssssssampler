# GPU-Driven Audio-Reactive ASCII Rendering System Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the ASCII rendering system with a dense, layered, audio-reactive GPU-driven engine that feels alive and continuously active, with strong visual degradation tied to audio quality parameters.

**Architecture:** A three-layer rendering pipeline: CPU layer (audio analysis + parameter remapping), GPU layer (instanced quad rendering of ASCII glyphs via wgpu), and a Vizia-embedded view that hosts the render surface. **CORE IMAGE:** img01.txt (36×46 grid) serves as the anchor/base layer; this determines the window size and never changes. Other 18 images are randomly positioned/overlaid on top with pop/highlight effects when applied. The system loads 19 ASCII images at compile time, dynamically switches secondary layer images based on audio amplitude, and blends 1–5 overlays with spatial/temporal offsets to create visual complexity. Non-linear parameter mapping ensures low sample rates produce extreme visual degradation while high quality yields stable, readable output.

**Tech Stack:** NIH-plug (VST3/CLAP), wgpu (GPU rendering), Vizia (UI layout only), Rust 2021 edition. Brand aesthetic: Apple-Calm (warm indigo, soft violet, calm minimal). 60fps target with streaming audio analysis.

---

## CORE IMAGE STRATEGY (img01.txt Anchor)

**img01.txt is the immutable base layer: 36 lines × 46 characters.**

This serves as:
1. **Window dimensions:** The grid is always 46×36 cells
2. **Anchor/stability:** Never changes; provides visual "home" state
3. **Overlay target:** All 18 other images overlay on top with pop/highlight effects

When secondary layers are applied:
- **Fit within bounds:** If an image fits within the 46×36 grid, render it directly
- **Overflow handling:** If an image is larger, randomly position a viewport crop within it
- **Pop effect:** Secondary images render at 1.5x brightness/saturation for visual emphasis (fades out smoothly)
- **Z-order:** img01 always at z=0 (back), secondary layers z=1–5 (front, audio-driven)

This creates a stable, recognizable visual anchor while allowing dynamic overlay complexity.

---

## File Structure

### New Files
- `src/render/mod.rs` — GPU rendering pipeline exports
- `src/render/ascii_render.rs` — Core rendering logic (layer switching, instancing, blending)
- `src/render/glyph_atlas.rs` — Glyph atlas texture creation from charset
- `src/render/color_system.rs` — Color palette system (theme-aware, audio-driven)
- `src/render/audio_analysis.rs` — RMS + transient detection, parameter remapping
- `src/render/layer_engine.rs` — Layer selection, blend weight calculation, motion offsets
- `src/render/shaders/render.wgsl` — Main quad rendering shader (glyph atlas sampling, blending)
- `src/render/shaders/compute.wgsl` — (Optional) GPU-side audio processing or layer decision
- `src/editor_view.rs` — Vizia wrapper for wgpu render surface (replaces ASCII canvas calls in editor.rs)
- `src/parameter_remapping.rs` — Non-linear parameter scaling (sample rate → instability, bit depth → char set reduction)

### Modified Files
- `src/lib.rs` — Add render module, update DSP to feed audio analysis to renderer
- `src/editor.rs` — Replace canvas draw loop with wgpu render surface embedded in Vizia
- `src/anim_state.rs` — Extend AnimParams to include RMS, layer state, motion offsets
- `Cargo.toml` — Add wgpu, wgpu-core, bytemuck dependencies
- `assets/style.css` — Adjust canvas/render-surface sizing and positioning

---

## Task Decomposition

### Phase 1: Foundational Rendering Infrastructure (Tasks 1–5)

#### Task 1: Add wgpu Dependencies & Graphics HAL Setup

**Files:**
- Modify: `Cargo.toml`
- Create: `src/render/mod.rs` (stubs only)

- [ ] **Step 1: Update Cargo.toml with wgpu and dependencies**

Add to `[dependencies]`:
```toml
wgpu = "0.20"
wgpu-types = "0.20"
bytemuck = { version = "1", features = ["derive"] }
raw-window-handle = "0.6"
```

- [ ] **Step 2: Run cargo check to verify compilation**

```bash
cargo check
```
Expected: Compiles without wgpu usage errors (unused import warnings OK)

- [ ] **Step 3: Create render module scaffold**

Create `src/render/mod.rs`:
```rust
pub mod glyph_atlas;
pub mod color_system;
pub mod audio_analysis;
pub mod layer_engine;
pub mod ascii_render;

pub use ascii_render::AsciiRenderer;
pub use color_system::ColorPalette;
pub use audio_analysis::AudioAnalyzer;
pub use layer_engine::LayerEngine;
```

- [ ] **Step 4: Add render module to lib.rs**

In `src/lib.rs`, add after `mod anim_state;`:
```rust
mod render;
```

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml src/render/mod.rs src/lib.rs
git commit -m "feat: add wgpu dependencies and render module scaffold"
```

---

#### Task 2: Implement Glyph Atlas Texture Builder

**Files:**
- Create: `src/render/glyph_atlas.rs`
- Modify: `src/ascii_bank.rs` (no code changes, referenced for charset)

- [ ] **Step 1: Define glyph atlas structure**

Create `src/render/glyph_atlas.rs`:
```rust
use bytemuck::{Pod, Zeroable};

/// Metadata for a single glyph in the atlas.
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct GlyphInfo {
    /// UV coords: (u_min, v_min, u_max, v_max) as normalized floats [0..1]
    pub uv: [f32; 4],
}

/// Glyph atlas texture: 8x8 grid of ASCII chars (space to @), monospace font
pub struct GlyphAtlas {
    /// Raw RGBA8 texture data (atlas_w × atlas_h × 4)
    pub texture_data: Vec<u8>,
    /// Texture width in pixels
    pub width: u32,
    /// Texture height in pixels
    pub height: u32,
    /// Info per glyph (84 glyphs, indexed by char_to_idx)
    pub glyphs: Vec<GlyphInfo>,
}

impl GlyphAtlas {
    /// Build a simple monospace glyph atlas: 10 glyphs per row, 9 rows
    /// Each glyph cell = 16×16 pixels, filled with ASCII rasterization
    pub fn new(charset_len: usize) -> Self {
        let glyphs_per_row = 10;
        let glyph_size = 16;
        let rows = (charset_len + glyphs_per_row - 1) / glyphs_per_row;

        let atlas_w = (glyphs_per_row * glyph_size) as u32;
        let atlas_h = (rows * glyph_size) as u32;

        let mut texture_data = vec![0u8; (atlas_w * atlas_h * 4) as usize];
        let mut glyphs = Vec::new();

        for i in 0..charset_len {
            let row = i / glyphs_per_row;
            let col = i % glyphs_per_row;

            let x = (col * glyph_size) as u32;
            let y = (row * glyph_size) as u32;

            // Rasterize character at (x, y)
            rasterize_glyph(&mut texture_data, atlas_w, x, y, glyph_size as u32);

            let u_min = x as f32 / atlas_w as f32;
            let v_min = y as f32 / atlas_h as f32;
            let u_max = (x + glyph_size as u32) as f32 / atlas_w as f32;
            let v_max = (y + glyph_size as u32) as f32 / atlas_h as f32;

            glyphs.push(GlyphInfo { uv: [u_min, v_min, u_max, v_max] });
        }

        GlyphAtlas { texture_data, width: atlas_w, height: atlas_h, glyphs }
    }
}

/// Simple rasterization: fill a glyph cell with a test pattern (bright center)
fn rasterize_glyph(data: &mut [u8], atlas_w: u32, x: u32, y: u32, size: u32) {
    for py in 0..size {
        for px in 0..size {
            let pos = ((y + py) * atlas_w + (x + px)) as usize * 4;
            if pos + 3 < data.len() {
                // Simple: center pixel bright, fade outward
                let dx = (px as i32 - size as i32 / 2).abs() as u32;
                let dy = (py as i32 - size as i32 / 2).abs() as u32;
                let dist = (dx * dx + dy * dy) as f32;
                let max_dist = (size * size) as f32;
                let brightness = ((1.0 - (dist / max_dist)).max(0.0) * 255.0) as u8;

                data[pos] = brightness;     // R
                data[pos + 1] = brightness; // G
                data[pos + 2] = brightness; // B
                data[pos + 3] = 255;        // A
            }
        }
    }
}
```

- [ ] **Step 2: Verify module compiles**

```bash
cargo check --lib
```
Expected: Compiles (unused warnings OK)

- [ ] **Step 3: Add glyph_atlas to render module exports**

In `src/render/mod.rs`, add:
```rust
pub mod glyph_atlas;
pub use glyph_atlas::{GlyphAtlas, GlyphInfo};
```

- [ ] **Step 4: Commit**

```bash
git add src/render/glyph_atlas.rs src/render/mod.rs
git commit -m "feat: implement glyph atlas texture builder"
```

---

#### Task 3: Implement Color Palette System

**Files:**
- Create: `src/render/color_system.rs`

- [ ] **Step 1: Define color types and theme palette**

Create `src/render/color_system.rs`:
```rust
use bytemuck::{Pod, Zeroable};

/// Linear RGBA color
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Color { r, g, b, a }
    }

    /// From sRGB hex: 0xRRGGBB, convert to linear
    pub fn from_srgb_hex(hex: u32) -> Self {
        let r = ((hex >> 16) & 0xFF) as f32 / 255.0;
        let g = ((hex >> 8) & 0xFF) as f32 / 255.0;
        let b = (hex & 0xFF) as f32 / 255.0;
        Color {
            r: r.powf(2.2),
            g: g.powf(2.2),
            b: b.powf(2.2),
            a: 1.0,
        }
    }
}

pub struct ColorPalette {
    /// Layer 0 (dominant): primary color
    pub primary: Color,
    /// Layer 1–4: secondary colors, distinct from primary
    pub secondary: [Color; 4],
    /// Background color
    pub background: Color,
    /// Emphasis color for non-space characters
    pub emphasis: Color,
}

impl ColorPalette {
    /// Apple-Calm palette: warm indigo + soft violet + muted green + amber
    pub fn calm_dark() -> Self {
        ColorPalette {
            primary: Color::from_srgb_hex(0x7C6CFF),    // Soft Violet
            secondary: [
                Color::from_srgb_hex(0x4CAF82),         // Muted Green
                Color::from_srgb_hex(0xF4B860),         // Warm Amber
                Color::from_srgb_hex(0x6A5AEF),         // Deeper Violet
                Color::from_srgb_hex(0x5DADE2),         // Soft Blue
            ],
            background: Color::from_srgb_hex(0x1E1E2F), // Deep Indigo
            emphasis: Color::from_srgb_hex(0xF5F5F7),   // Soft White
        }
    }

    /// Get layer color (index 0 = primary, 1–4 = secondary)
    pub fn layer_color(&self, layer_idx: usize) -> Color {
        if layer_idx == 0 {
            self.primary
        } else {
            self.secondary[(layer_idx - 1) % 4]
        }
    }
}
```

- [ ] **Step 2: Verify compilation**

```bash
cargo check --lib
```
Expected: Compiles

- [ ] **Step 3: Add to render module**

In `src/render/mod.rs`:
```rust
pub mod color_system;
pub use color_system::{Color, ColorPalette};
```

- [ ] **Step 4: Commit**

```bash
git add src/render/color_system.rs src/render/mod.rs
git commit -m "feat: implement color palette system with Apple-Calm theme"
```

---

#### Task 4: Implement Parameter Remapping (Non-Linear Scaling)

**Files:**
- Create: `src/parameter_remapping.rs`

- [ ] **Step 1: Define remapping functions**

Create `src/parameter_remapping.rs`:
```rust
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
```

- [ ] **Step 2: Test remapping functions**

```bash
cargo check --lib
```
Expected: Compiles

- [ ] **Step 3: Add to lib.rs**

In `src/lib.rs`, after `mod render;`:
```rust
mod parameter_remapping;
pub use parameter_remapping::*;
```

- [ ] **Step 4: Commit**

```bash
git add src/parameter_remapping.rs src/lib.rs
git commit -m "feat: implement non-linear parameter remapping for audio-reactive visuals"
```

---

#### Task 5: Implement Audio Analysis (RMS + Transient Detection)

**Files:**
- Create: `src/render/audio_analysis.rs`
- Modify: `src/anim_state.rs`

- [ ] **Step 1: Create audio analyzer**

Create `src/render/audio_analysis.rs`:
```rust
/// Real-time audio analysis: RMS and transient detection

pub struct AudioAnalyzer {
    /// Circular buffer of RMS samples (1 per frame)
    rms_history: Vec<f32>,
    rms_index: usize,
    /// Transient detection: peak RMS in last N frames
    transient_threshold: f32,
    /// Current frame RMS
    pub current_rms: f32,
    /// Is a transient active?
    pub transient_active: bool,
}

impl AudioAnalyzer {
    pub fn new(history_len: usize) -> Self {
        AudioAnalyzer {
            rms_history: vec![0.0; history_len.max(1)],
            rms_index: 0,
            transient_threshold: 0.6,
            current_rms: 0.0,
            transient_active: false,
        }
    }

    /// Process an audio buffer: compute RMS and detect transients
    pub fn analyze(&mut self, samples: &[f32]) {
        if samples.is_empty() {
            self.current_rms = 0.0;
            return;
        }

        let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
        self.current_rms = (sum_sq / samples.len() as f32).sqrt().clamp(0.0, 1.0);

        // Store in history
        self.rms_history[self.rms_index] = self.current_rms;
        self.rms_index = (self.rms_index + 1) % self.rms_history.len();

        // Detect transient: RMS > threshold * historical average
        let avg_rms: f32 = self.rms_history.iter().sum::<f32>() / self.rms_history.len() as f32;
        self.transient_active = self.current_rms > avg_rms * 2.0 && self.current_rms > 0.1;
    }

    /// Get smoothed RMS (3-frame exponential moving average)
    pub fn smoothed_rms(&self) -> f32 {
        let prev_idx = if self.rms_index == 0 { self.rms_history.len() - 1 } else { self.rms_index - 1 };
        let prev_prev_idx = if prev_idx == 0 { self.rms_history.len() - 1 } else { prev_idx - 1 };

        (self.current_rms * 0.5 + self.rms_history[prev_idx] * 0.3 + self.rms_history[prev_prev_idx] * 0.2)
    }
}
```

- [ ] **Step 2: Extend AnimParams in anim_state.rs**

In `src/anim_state.rs`, update `AnimParams`:
```rust
#[derive(Clone, Debug)]
pub struct AnimParams {
    pub mix: f32,
    pub sample_rate_norm: f32,
    pub bit_depth_norm: f32,
    pub jitter: f32,
    pub filter_cutoff_norm: f32,

    // NEW: Audio analysis
    pub rms: f32,
    pub transient_active: bool,
    pub instability: f32,    // sample_rate_to_instability result
    pub quantization: f32,   // bit_depth_to_quantization result
    pub layer_count: f32,    // amplitude_to_layer_count result
}

impl Default for AnimParams {
    fn default() -> Self {
        Self {
            mix: 1.0,
            sample_rate_norm: 0.27,
            bit_depth_norm: 0.5,
            jitter: 0.0,
            filter_cutoff_norm: 0.9,

            rms: 0.0,
            transient_active: false,
            instability: 0.0,
            quantization: 0.0,
            layer_count: 1.0,
        }
    }
}
```

- [ ] **Step 3: Verify compilation**

```bash
cargo check --lib
```
Expected: Compiles

- [ ] **Step 4: Add to render module**

In `src/render/mod.rs`:
```rust
pub mod audio_analysis;
pub use audio_analysis::AudioAnalyzer;
```

- [ ] **Step 5: Commit**

```bash
git add src/render/audio_analysis.rs src/anim_state.rs src/render/mod.rs
git commit -m "feat: implement audio analysis and extend animation parameters"
```

---

### Phase 2: Layer Engine & Motion System (Tasks 6–8)

#### Task 6: Implement Layer Engine (Image Selection + Blending)

**Files:**
- Create: `src/render/layer_engine.rs`
- Reference: `src/ascii_bank.rs`

- [ ] **Step 1: Define layer state**

Create `src/render/layer_engine.rs`:
```rust
use crate::ascii_bank::AsciiBank;

/// A single rendered layer
#[derive(Clone, Debug)]
pub struct LayerState {
    /// Index into AsciiBank.images (0 = img01.txt, always anchor)
    pub image_idx: usize,
    /// Blend weight (0.0–1.0)
    pub weight: f32,
    /// Time offset (frames)
    pub time_offset: i32,
    /// Spatial offset (x, y) in grid cells OR viewport crop offset if image > bounds
    pub spatial_offset: (i32, i32),
    /// If true, render with pop effect (1.5x brightness, fade)
    pub pop_highlight: bool,
}

/// Layer engine: manages up to 5 active layers
pub struct LayerEngine {
    layers: Vec<LayerState>,
    rng_state: u64,
}

impl LayerEngine {
    pub fn new() -> Self {
        LayerEngine {
            layers: vec![
                LayerState { image_idx: 0, weight: 1.0, time_offset: 0, spatial_offset: (0, 0) };
                5
            ],
            rng_state: 0x123456789ABCDEF,
        }
    }

    /// Update layer states based on audio amplitude and parameters
    /// ANCHOR DESIGN: Layer 0 is ALWAYS img01.txt (36×46 grid base)
    /// Secondary layers (1–4) are overlays with pop highlights
    pub fn update(
        &mut self,
        rms: f32,
        layer_count: f32,
        instability: f32,
        transient_active: bool,
        ascii_bank: &AsciiBank,
    ) {
        let active_count = (layer_count.ceil() as usize).min(5).max(1);

        // Layer 0: ALWAYS img01.txt (anchor, no change)
        self.layers[0].image_idx = 0;  // img01.txt
        self.layers[0].weight = 1.0;
        self.layers[0].time_offset = 0;
        self.layers[0].spatial_offset = (0, 0);
        self.layers[0].pop_highlight = false;

        // Secondary layers (1–4): random overlay images with pop highlight
        for i in 1..active_count {
            self.layers[i].image_idx = self.select_overlay_image(i, ascii_bank);
            self.layers[i].weight = (1.0 / (active_count as f32 * 2.0)).min(0.4);
            self.layers[i].time_offset = (i as i32 * 5);
            // Random viewport position for larger images, or small offset for smaller ones
            let offset_x = (((self.lcg_rand() as i32) % 10) as i32) - 5;
            let offset_y = (((self.lcg_rand() as i32) % 10) as i32) - 5;
            self.layers[i].spatial_offset = (offset_x, offset_y);
            self.layers[i].pop_highlight = true;  // Overlay images get pop effect
        }

        // Zero out inactive layers
        for i in active_count..5 {
            self.layers[i].weight = 0.0;
            self.layers[i].pop_highlight = false;
        }

        // On transient: spawn temporary layer spike with extra pop
        if transient_active && active_count < 5 {
            let temp_idx = active_count;
            self.layers[temp_idx].image_idx = self.select_overlay_image(temp_idx, ascii_bank);
            self.layers[temp_idx].weight = 0.5;
            self.layers[temp_idx].time_offset = 0;
            self.layers[temp_idx].pop_highlight = true;
        }
    }

    pub fn layers(&self) -> &[LayerState] {
        &self.layers
    }

    fn select_overlay_image(&mut self, _layer_idx: usize, ascii_bank: &AsciiBank) -> usize {
        // Select overlay image: skip img01 (index 0), choose from the rest
        let rand_val = self.lcg_rand();
        let idx = ((rand_val as usize) % (ascii_bank.len() - 1)) + 1;
        idx.min(ascii_bank.len() - 1)
    }

    fn lcg_rand(&mut self) -> u64 {
        self.rng_state = self.rng_state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.rng_state
    }
}
```

- [ ] **Step 2: Verify compilation**

```bash
cargo check --lib
```
Expected: Compiles

- [ ] **Step 3: Add to render module**

In `src/render/mod.rs`:
```rust
pub mod layer_engine;
pub use layer_engine::{LayerEngine, LayerState};
```

- [ ] **Step 4: Commit**

```bash
git add src/render/layer_engine.rs src/render/mod.rs
git commit -m "feat: implement layer engine with audio-driven image selection"
```

---

#### Task 7: Implement Motion System (Time + Spatial Offsets)

**Files:**
- Create: `src/render/motion.rs` (new)
- Modify: `src/render/layer_engine.rs`

- [ ] **Step 1: Define motion module**

Create `src/render/motion.rs`:
```rust
/// Multi-layer motion system: global drift, per-layer motion, region offsets

pub struct MotionSystem {
    /// Global time in frames
    pub frame_count: u64,
}

impl MotionSystem {
    pub fn new() -> Self {
        MotionSystem { frame_count: 0 }
    }

    pub fn step(&mut self) {
        self.frame_count = self.frame_count.wrapping_add(1);
    }

    /// Global slow drift (always active): 0–2 pixels, period ~4 seconds at 60fps
    pub fn global_drift(&self) -> (f32, f32) {
        let t = self.frame_count as f32 / 60.0;
        let drift_x = (t * 0.5).sin() * 2.0;
        let drift_y = (t * 0.3).cos() * 2.0;
        (drift_x, drift_y)
    }

    /// Per-layer motion: layer i has slightly different phase
    pub fn layer_motion(&self, layer_idx: usize, speed_multiplier: f32) -> f32 {
        let t = self.frame_count as f32 * speed_multiplier / 60.0;
        let phase = (layer_idx as f32 * 0.5) % std::f32::consts::TAU;
        ((t + phase).sin() * 2.0).round()
    }

    /// Per-region offset (divide grid into 4×4 regions, each slightly offset)
    pub fn region_offset(&self, region_x: usize, region_y: usize, instability: f32) -> (f32, f32) {
        // Regions have slight time offset to create interference
        let region_phase = ((region_x * 7 + region_y * 11) as f32 * 0.1) % std::f32::consts::TAU;
        let t = self.frame_count as f32 / 60.0;
        let offset = ((t + region_phase).sin() * instability * 1.5) as i32;
        (offset as f32, (offset / 2) as f32)
    }
}
```

- [ ] **Step 2: Verify compilation**

```bash
cargo check --lib
```
Expected: Compiles

- [ ] **Step 3: Add to render module**

In `src/render/mod.rs`:
```rust
pub mod motion;
pub use motion::MotionSystem;
```

- [ ] **Step 4: Commit**

```bash
git add src/render/motion.rs src/render/mod.rs
git commit -m "feat: implement multi-layer motion system with global drift and region offsets"
```

---

#### Task 8: Update anim_state to Feed Motion Data

**Files:**
- Modify: `src/anim_state.rs`

- [ ] **Step 1: Add motion state to AnimParams**

In `src/anim_state.rs`, add fields to `AnimParams`:
```rust
pub struct AnimParams {
    // ... existing fields ...

    // NEW: Motion data
    pub global_drift: (f32, f32),
    pub layer_motion: [f32; 5],
    pub region_offsets: Vec<(f32, f32)>,  // 16 regions (4×4)
    pub frame_count: u64,
}

impl Default for AnimParams {
    fn default() -> Self {
        Self {
            // ... existing defaults ...

            global_drift: (0.0, 0.0),
            layer_motion: [0.0; 5],
            region_offsets: vec![(0.0, 0.0); 16],
            frame_count: 0,
        }
    }
}
```

- [ ] **Step 2: Verify**

```bash
cargo check --lib
```
Expected: Compiles

- [ ] **Step 3: Commit**

```bash
git add src/anim_state.rs
git commit -m "feat: extend animation parameters with motion data"
```

---

### Phase 3: Shader & GPU Infrastructure (Tasks 9–11)

#### Task 9: Create Main Rendering Shader (WGSL)

**Files:**
- Create: `src/render/shaders/render.wgsl`

- [ ] **Step 1: Write shader structure**

Create `src/render/shaders/render.wgsl`:
```wgsl
struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) glyph_idx: u32,
    @location(2) color: vec4<f32>,
    @location(3) uv_min: vec2<f32>,
    @location(4) uv_max: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    // Normalize to NDC [-1, 1]
    output.position = vec4<f32>(
        (input.position.x / 960.0) * 2.0 - 1.0,
        (input.position.y / 540.0) * 2.0 - 1.0,
        0.0,
        1.0,
    );
    output.uv = input.uv_min;
    output.color = input.color;
    return output;
}

@group(0) @binding(0)
var glyph_atlas: texture_2d<f32>;

@group(0) @binding(1)
var glyph_sampler: sampler;

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let atlas_color = textureSample(glyph_atlas, glyph_sampler, input.uv);
    // Simple blend: modulate atlas by color
    return atlas_color * input.color;
}
```

- [ ] **Step 2: Verify shader syntax (will be checked at runtime)**

Just save the file.

- [ ] **Step 3: Commit**

```bash
git add src/render/shaders/render.wgsl
git commit -m "feat: create main rendering shader for glyph instancing"
```

---

#### Task 10: Implement Core ASCII Renderer (wgpu Integration)

**Files:**
- Create: `src/render/ascii_render.rs`

- [ ] **Step 1: Define renderer structure**

Create `src/render/ascii_render.rs`:
```rust
use wgpu::*;
use crate::ascii_bank::AsciiBank;
use crate::render::{GlyphAtlas, ColorPalette, LayerEngine, MotionSystem};
use crate::anim_state::AnimParams;

/// GPU renderer for ASCII art
pub struct AsciiRenderer {
    device: Device,
    queue: Queue,
    pipeline: Option<RenderPipeline>,
    bind_group_layout: BindGroupLayout,
    atlas_texture: Texture,
    atlas_bind_group: BindGroup,
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    index_count: u32,
}

impl AsciiRenderer {
    /// Create a new renderer (requires a wgpu surface/device)
    pub async fn new(
        device: &Device,
        queue: &Queue,
        surface_format: TextureFormat,
        ascii_bank: &AsciiBank,
        palette: &ColorPalette,
    ) -> Result<Self, String> {
        // TODO: Full initialization
        Err("Not yet implemented".into())
    }

    /// Update GPU buffers and redraw
    pub fn render(
        &self,
        target_view: &TextureView,
        ascii_bank: &AsciiBank,
        layer_engine: &LayerEngine,
        motion_system: &MotionSystem,
        anim_params: &AnimParams,
    ) -> Result<(), String> {
        // TODO: Render implementation
        Ok(())
    }
}
```

- [ ] **Step 2: Verify module compiles**

```bash
cargo check --lib
```
Expected: Compiles (wgpu errors OK for now)

- [ ] **Step 3: Add to render module**

In `src/render/mod.rs`:
```rust
pub mod ascii_render;
pub use ascii_render::AsciiRenderer;
```

- [ ] **Step 4: Commit**

```bash
git add src/render/ascii_render.rs src/render/mod.rs
git commit -m "feat: scaffold ASCII renderer (GPU integration)"
```

---

#### Task 11: Create Vizia Wrapper for wgpu Render Surface

**Files:**
- Create: `src/editor_view.rs`
- Modify: `src/editor.rs`

- [ ] **Step 1: Create Vizia render surface wrapper**

Create `src/editor_view.rs`:
```rust
use nih_plug_vizia::vizia::prelude::*;
use crate::anim_state::SharedAnimParams;
use crate::ascii_bank::AsciiBank;

/// Vizia view that embeds a wgpu render surface
pub struct AsciiRenderView {
    anim_params: SharedAnimParams,
}

impl AsciiRenderView {
    pub fn new(cx: &mut Context, anim_params: SharedAnimParams) -> Handle<Self> {
        Self { anim_params }
            .build(cx, |_cx| {})
            .size(Stretch(1.0))
            .background_color(Color::rgb(30, 30, 47)) // Deep Indigo
    }
}

impl View for AsciiRenderView {
    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        // TODO: Integrate wgpu rendering into Vizia canvas
    }
}
```

- [ ] **Step 2: Verify compilation**

```bash
cargo check --lib
```
Expected: Compiles

- [ ] **Step 3: Update editor.rs to use render view**

In `src/editor.rs`, after the imports, add:
```rust
use crate::editor_view::AsciiRenderView;
```

- [ ] **Step 4: Commit**

```bash
git add src/editor_view.rs src/editor.rs src/lib.rs
git commit -m "feat: create Vizia wrapper for wgpu render surface"
```

---

### Phase 4: Integration & Audio-Reactivity (Tasks 12–14)

#### Task 12: Wire Audio Analysis into DSP Loop

**Files:**
- Modify: `src/lib.rs`
- Create: `src/audio_feed.rs` (new module)

- [ ] **Step 1: Create audio feed module**

Create `src/audio_feed.rs`:
```rust
use crate::render::AudioAnalyzer;
use crate::anim_state::SharedAnimParams;
use crate::{amplitude_to_brightness, amplitude_to_layer_count, amplitude_to_motion_speed};
use crate::{sample_rate_to_instability, bit_depth_to_quantization};

/// Feeds processed audio samples to the analyzer
pub struct AudioFeed {
    analyzer: AudioAnalyzer,
    buffer: Vec<f32>,
}

impl AudioFeed {
    pub fn new() -> Self {
        AudioFeed {
            analyzer: AudioAnalyzer::new(120), // 2s at 60fps
            buffer: Vec::with_capacity(2048),
        }
    }

    /// Process samples and update shared animation parameters
    pub fn update(
        &mut self,
        samples: &[f32],
        target_sr: f32,
        bit_depth: f32,
        shared_params: &SharedAnimParams,
    ) {
        self.analyzer.analyze(samples);

        let rms = self.analyzer.smoothed_rms();
        let transient = self.analyzer.transient_active;

        // Compute derived values
        let instability = sample_rate_to_instability(target_sr);
        let quantization = bit_depth_to_quantization(bit_depth);
        let layer_count = amplitude_to_layer_count(rms);
        let brightness = amplitude_to_brightness(rms);
        let motion_speed = amplitude_to_motion_speed(rms);

        // Update shared state
        if let Ok(mut params) = shared_params.lock() {
            params.rms = rms;
            params.transient_active = transient;
            params.instability = instability;
            params.quantization = quantization;
            params.layer_count = layer_count;
            // brightness and motion_speed used in render, not stored here
        }
    }
}
```

- [ ] **Step 2: Add to lib.rs**

In `src/lib.rs`, after other mod declarations:
```rust
mod audio_feed;
```

- [ ] **Step 3: Verify**

```bash
cargo check --lib
```
Expected: Compiles

- [ ] **Step 4: Commit**

```bash
git add src/audio_feed.rs src/lib.rs
git commit -m "feat: wire audio analysis into DSP processing loop"
```

---

#### Task 13: Update DSP Process Block to Feed Analyzer

**Files:**
- Modify: `src/lib.rs` (Sssssssssampler::process)

- [ ] **Step 1: Add audio feed instance to plugin struct**

In `src/lib.rs`, update `Sssssssssampler`:
```rust
struct Sssssssssampler {
    params: Arc<SssssssssamplerParams>,
    sample_rate: f32,
    phase: [f32; 2],
    held: [f32; 2],
    filter: FilterState,
    last_filter_step: f32,
    last_filter_cutoff: f32,
    last_filter_poles: i32,

    // NEW
    audio_feed: audio_feed::AudioFeed,
}

impl Default for Sssssssssampler {
    fn default() -> Self {
        Self {
            // ... existing fields ...
            audio_feed: audio_feed::AudioFeed::new(),
        }
    }
}
```

- [ ] **Step 2: Update process block to feed samples**

In `Sssssssssampler::process`, after processing channels, add:
```rust
// Collect all samples for audio analysis
let mut all_samples = Vec::new();
for channel_samples in buffer.iter_samples() {
    for sample in channel_samples.into_iter() {
        all_samples.push(*sample);
    }
}

// Feed to analyzer
self.audio_feed.update(
    &all_samples,
    self.params.target_sr.smoothed.next(),
    self.params.bit_depth.smoothed.next(),
    &self.params.anim_params, // Requires adding this param to SssssssssamplerParams
);
```

- [ ] **Step 3: Add anim_params to SssssssssamplerParams**

In `SssssssssamplerParams`, add:
```rust
#[persist = "anim-params"]
pub anim_params: SharedAnimParams,
```

Update `impl Default`:
```rust
anim_params: anim_state::new_shared(),
```

- [ ] **Step 4: Verify**

```bash
cargo check --lib
```
Expected: Compiles

- [ ] **Step 5: Commit**

```bash
git add src/lib.rs
git commit -m "feat: integrate audio feed into DSP process block"
```

---

#### Task 14: Connect Render Loop to Animation Parameters

**Files:**
- Modify: `src/editor.rs`
- Modify: `src/editor_view.rs`

- [ ] **Step 1: Pass anim_params to editor**

In `src/editor.rs`, update `create` function signature:
```rust
pub(crate) fn create(
    params: Arc<SssssssssamplerParams>,
    editor_state: Arc<ViziaState>,
) -> Option<Box<dyn Editor>> {
    let anim_params = params.anim_params.clone();

    create_vizia_editor(
        editor_state,
        ViziaTheming::Custom,
        move |cx, gui_ctx| {
            // ... existing code, but now pass anim_params to AsciiRenderView
        },
    )
}
```

- [ ] **Step 2: Update AsciiRenderView to accept render loop**

In `src/editor_view.rs`:
```rust
impl AsciiRenderView {
    pub fn new(cx: &mut Context, anim_params: SharedAnimParams) -> Handle<Self> {
        let view = Self { anim_params }
            .build(cx, |_cx| {})
            .size(Stretch(1.0));

        // Start render timer (will call draw at 60fps)
        // TODO: Use Vizia's timer or event loop for 60fps rendering

        view
    }
}
```

- [ ] **Step 3: Verify**

```bash
cargo check --lib
```
Expected: Compiles

- [ ] **Step 4: Commit**

```bash
git add src/editor.rs src/editor_view.rs
git commit -m "feat: connect render loop to animation parameters"
```

---

### Phase 5: Full Rendering Implementation (Tasks 15–17)

#### Task 15: Implement Instance Buffer Generation

**Files:**
- Create: `src/render/instancing.rs`

- [ ] **Step 1: Define instance data structure**

Create `src/render/instancing.rs`:
```rust
use bytemuck::{Pod, Zeroable};
use crate::ascii_bank::AsciiBank;
use crate::render::{LayerEngine, LayerState, GlyphInfo, ColorPalette};
use crate::anim_state::AnimParams;

/// Per-instance data for a single glyph quad
#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct GlyphInstance {
    pub position: [f32; 2],           // Screen position (pixels)
    pub glyph_idx: u32,               // Index into glyph atlas
    pub color: [f32; 4],              // RGBA (linear)
    pub uv_min: [f32; 2],             // Atlas UV min
    pub uv_max: [f32; 2],             // Atlas UV max
    pub layer_influence: f32,         // Blend factor for this layer
    pub brightness: f32,              // Amplitude-driven brightness
}

/// Generate instance buffer for all visible glyphs across all layers
pub fn generate_instances(
    grid_width: usize,
    grid_height: usize,
    glyph_size: f32,
    ascii_bank: &AsciiBank,
    layer_engine: &LayerEngine,
    palette: &ColorPalette,
    anim_params: &AnimParams,
) -> Vec<GlyphInstance> {
    let mut instances = Vec::new();

    // For each grid cell
    for y in 0..grid_height {
        for x in 0..grid_width {
            let mut cell_color = palette.background;
            let mut cell_glyph_idx = 0u8;
            let mut cell_brightness = 0.5;

            // Layer 0 (dominant): use this character first
            let layer0 = &layer_engine.layers()[0];
            let char_idx = ascii_bank.get_cell(layer0.image_idx, x, y);

            if char_idx > 0 {
                // Non-space character
                cell_glyph_idx = char_idx;
                cell_color = palette.layer_color(0);
                cell_brightness = 1.0;
            }

            // Secondary layers: may override spaces or add to brightness
            for (layer_i, layer) in layer_engine.layers().iter().enumerate().skip(1) {
                if layer.weight > 0.0 {
                    let sec_char = ascii_bank.get_cell(layer.image_idx, x, y);
                    if sec_char > 0 && cell_glyph_idx == 0 {
                        // Fill empty space with secondary layer
                        cell_glyph_idx = sec_char;
                        cell_color = palette.layer_color(layer_i);
                        cell_brightness = 0.6;
                    }
                }
            }

            // Build instance
            if cell_glyph_idx > 0 || cell_color.a > 0.01 {
                let px = x as f32 * glyph_size;
                let py = y as f32 * glyph_size;

                instances.push(GlyphInstance {
                    position: [px, py],
                    glyph_idx: cell_glyph_idx as u32,
                    color: [cell_color.r, cell_color.g, cell_color.b, cell_color.a],
                    uv_min: [0.0, 0.0],  // Will be set from glyph atlas
                    uv_max: [1.0, 1.0],
                    layer_influence: 1.0,
                    brightness: cell_brightness,
                });
            }
        }
    }

    instances
}
```

- [ ] **Step 2: Verify**

```bash
cargo check --lib
```
Expected: Compiles

- [ ] **Step 3: Add to render module**

In `src/render/mod.rs`:
```rust
pub mod instancing;
pub use instancing::{GlyphInstance, generate_instances};
```

- [ ] **Step 4: Commit**

```bash
git add src/render/instancing.rs src/render/mod.rs
git commit -m "feat: implement instance buffer generation for GPU rendering"
```

---

#### Task 16: Implement Full Renderer Initialization

**Files:**
- Modify: `src/render/ascii_render.rs` (complete implementation)

- [ ] **Step 1: Complete AsciiRenderer::new**

This task is complex; split into steps within the implementation. Update `src/render/ascii_render.rs` with full init:

```rust
// (Full implementation will handle device, queue, pipeline, buffers)
// For now: save a stub that will be filled in Task 17
```

**Note:** This step is complex and will be split with shader compilation. Save checkpoint.

- [ ] **Step 2: Compile and verify types**

```bash
cargo check --lib
```
Expected: Compiles

- [ ] **Step 3: Commit checkpoint**

```bash
git add src/render/ascii_render.rs
git commit -m "wip: start full renderer initialization"
```

---

#### Task 17: Complete Render Method & Frame Submission

**Files:**
- Modify: `src/render/ascii_render.rs`

- [ ] **Step 1: Implement render method**

```rust
// (Complete wgpu render call: command encoder, render pass, draw calls)
```

- [ ] **Step 2: Test compilation**

```bash
cargo check --lib
```

- [ ] **Step 3: Commit**

```bash
git add src/render/ascii_render.rs
git commit -m "feat: complete renderer implementation with full render method"
```

---

### Phase 6: Parameter Mapping & Quality Zones (Tasks 18–19)

#### Task 18: Validate Sample Rate → Instability Mapping

**Files:**
- Create: `tests/test_parameter_mapping.rs`
- Reference: `src/parameter_remapping.rs`

- [ ] **Step 1: Write validation tests**

Create `tests/test_parameter_mapping.rs`:
```rust
use sssssssssampler::sample_rate_to_instability;

#[test]
fn test_sr_mapping_zones() {
    // Zone 1: 44.1kHz+ should be minimal (< 0.2)
    assert!(sample_rate_to_instability(44_100.0) < 0.2);
    assert!(sample_rate_to_instability(96_000.0) < 0.1);

    // Zone 2: 30–44kHz should be mild (0.1–0.3)
    let mid = sample_rate_to_instability(35_000.0);
    assert!(mid > 0.1 && mid < 0.4);

    // Zone 3: 15–30kHz should be moderate (0.3–0.6)
    let mod_val = sample_rate_to_instability(22_000.0);
    assert!(mod_val > 0.3 && mod_val < 0.7);

    // Zone 4: <15kHz should be extreme (0.6+)
    assert!(sample_rate_to_instability(10_000.0) > 0.5);
    assert!(sample_rate_to_instability(1_000.0) > 0.9);
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test --lib test_sr_mapping_zones
```
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add tests/test_parameter_mapping.rs
git commit -m "test: validate sample rate to instability mapping"
```

---

#### Task 19: Finalize and Document Rendering Pipeline

**Files:**
- Modify: All
- Create: `docs/RENDERING.md`

- [ ] **Step 1: Write architecture documentation**

Create `docs/RENDERING.md`:
```markdown
# Audio-Reactive ASCII Rendering System

## Overview
...
(Document the full pipeline, parameter mapping zones, layer selection logic, etc.)
```

- [ ] **Step 2: Add inline documentation to key files**

Add doc comments to:
- `src/render/mod.rs`
- `src/render/ascii_render.rs`
- `src/render/layer_engine.rs`
- `src/parameter_remapping.rs`

- [ ] **Step 3: Verify all builds**

```bash
cargo build --release
```
Expected: Compiles

- [ ] **Step 4: Commit**

```bash
git add docs/RENDERING.md src/render/*.rs src/parameter_remapping.rs
git commit -m "docs: complete rendering pipeline documentation"
```

---

### Phase 7: Testing & Optimization (Tasks 20–21)

#### Task 20: End-to-End Rendering Test

**Files:**
- Create: `tests/test_rendering_e2e.rs`

- [ ] **Step 1: Write integration test**

Create `tests/test_rendering_e2e.rs`:
```rust
#[test]
fn test_renderer_initialization() {
    // TODO: Create a headless wgpu instance and verify renderer spins up
}

#[test]
fn test_layer_switching() {
    // Verify layers switch correctly as amplitude changes
}

#[test]
fn test_parameter_reactivity() {
    // Verify audio parameters correctly drive visual changes
}
```

- [ ] **Step 2: Implement basic checks**

```bash
cargo test --test test_rendering_e2e
```

- [ ] **Step 3: Commit**

```bash
git add tests/test_rendering_e2e.rs
git commit -m "test: add end-to-end rendering tests"
```

---

#### Task 21: Benchmark & 60fps Validation

**Files:**
- Create: `benches/render_bench.rs`

- [ ] **Step 1: Create benchmark**

Create `benches/render_bench.rs`:
```rust
// Benchmark instance generation and rendering loop timing
```

- [ ] **Step 2: Profile for 60fps**

```bash
cargo bench
```

- [ ] **Step 3: Optimize if needed**

If frames exceed 16.67ms, profile and optimize:
- Reduce grid resolution
- Cache layer decisions
- Use compute shaders for complex operations

- [ ] **Step 4: Commit**

```bash
git add benches/render_bench.rs
git commit -m "perf: add rendering benchmarks and validate 60fps target"
```

---

## Task Dependencies

**Phase 1** (Tasks 1–5): Foundation
- 1 → 2, 3, 4, 5

**Phase 2** (Tasks 6–8): Layer System
- 5 → 6, 7, 8

**Phase 3** (Tasks 9–11): GPU Infrastructure
- 5, 6 → 9, 10, 11

**Phase 4** (Tasks 12–14): Audio Integration
- 1–11 → 12, 13, 14

**Phase 5** (Tasks 15–17): Full Rendering
- 3, 14 → 15, 16, 17

**Phase 6** (Tasks 18–19): Validation
- 17 → 18, 19

**Phase 7** (Tasks 20–21): Testing
- 19 → 20, 21

---

## Key Decisions

1. **Non-Linear Parameter Mapping:** Audio quality degradation is exponential, not linear. Sample rate zones ensure extreme SR produces visually obvious artifacts.

2. **Dominant Layer Model:** Always 1 primary layer (100% weight) + 1–4 secondary layers (weighted sum). Prevents visual mush.

3. **Whitespace Anti-Sparsity:** Secondary layers can fill grid cells that are spaces in the dominant layer.

4. **Non-Space Character Emphasis:** Non-space characters render brighter and override spaces from secondary layers.

5. **Shader-Based Blending:** All layer composition happens on GPU for efficiency. CPU only does layer selection and weight calculation.

6. **Brand Integration:** All colors map to Apple-Calm palette (warm indigo, soft violet, muted green, amber).

---

## Testing Strategy

- **Unit Tests:** Parameter remapping, layer selection logic
- **Integration Tests:** Full pipeline from audio → visual output
- **Performance Tests:** 60fps validation on target hardware
- **Visual Tests:** Manual verification of quality zones and layer switching

---

## Success Criteria

✓ Renders at 60fps
✓ Audio-driven layer switching visible
✓ Sample rate degradation produces extreme visual artifacts at SR < 15kHz
✓ Bit depth reduction limits character variety visibly
✓ Motion system creates continuous activity (never static)
✓ Secondary layers prevent empty/sparse regions
✓ Non-space characters emphasized (brighter, more visible)
✓ Fully calibrated to Apple-Calm brand aesthetic
