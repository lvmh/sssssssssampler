# ASCII Animation System V4

Visual engine for sssssssssampler — a responsive visual instrument driven by audio and interaction.

---

## Architecture

```
Audio Thread (lib.rs)
  ├─ DSP: sample-and-hold, bit crush, filter (2/4/6-pole Butterworth)
  ├─ Host transport → BPM, playing
  ├─ AudioAnalyzer → RMS, transient detection
  └─ AnimationParams: energy, transient, BPM, playing, motion_speed...
         │
         ▼
Editor Model (editor.rs :: UpdateFrameBuffer)
  ├─ Smoothed energy → visual state (IDLE/FLOW/BUILD/PEAK)
  ├─ SR temporal quantization + smearing
  ├─ Velocity-based motion (damped, force-driven)
  ├─ Filter → structural visibility (coherent noise masking)
  ├─ Tiered corruption (bit depth), energy-coupled dust
  ├─ V3: Moment system (FreezeCut, GlitchBloom, LockIn, etc.)
  ├─ V3: Memory system (heat, fatigue, afterimage)
  ├─ V3: Restraint system (idle dampening, recovery windows)
  ├─ V4: Phrase system (8-bar arcs driving overlay/brightness modulation)
  ├─ V4: Intent model (tension/release/chaos → moment selection)
  ├─ V4: Anchor-based composition (overlay positioning + collision avoidance)
  ├─ V4: Coherent glitch field (FBM-style layered noise)
  ├─ V4: Light mode awareness (is_light flag → inverted emphasis/boost)
  └─ Writes FrameBuffer (46×36, RGBA8 + theme colors + preset/theme idx + is_light)
         │
         ▼
Display View (ascii_image_display.rs)
  ├─ FiraCode Nerd Font, femtovg Canvas
  ├─ Renders CHARSET[pixel.alpha] as colored monospace glyph
  ├─ femtovg UI overlay (title + collapsible param menu)
  ├─ Mouse interaction: click-drag params, toggle AA, cycle themes/presets
  └─ Menu auto-hides when mouse leaves top-left quarter
```

---

## Non-Negotiable Rules

- ASCII characters from source files pass through exactly as-is by default
- No density-based character substitution
- `char_to_idx()`: exact match only, unknown chars → space (0)
- All visuals deterministic (hash-based noise, no `rand`)
- Zero per-frame allocation, ~60fps
- UI overlay never glitched, masked, or distorted

---

## Window & Grid

```
422 × 600 px (exact fit for 46-col monospace grid)
COLS=46, ROWS=36
BASE_MARGIN=1
Grid left-aligned, vertically centered
```

The entire VST window is the ASCII display — no header, no bottom controls. All interaction is through the in-grid UI overlay.

---

## Image Source

All images loaded from **`ascii.txt`** (root directory) using `#N` separator format. Currently **38 images**. Parsed at startup by `AsciiBank::from_ascii_txt()`. Each image stored at **native resolution** — no resizing, no distortion. `get_cell()` returns 0 for out-of-bounds. Oversized images (wider than 46 cols or taller than 36 rows) are viewported — the scroll/offset system pans across the full image extent. Each image has computed `density` (non-space fraction) and `complexity` (edge transitions) metrics used for biased selection.

---

## Image Cycling (BPM-Synced)

### Core Image
- **Changes every 2 bars**
- Random order (hash-based, not sequential)
- **Scatter-dissolve transition** over half a bar with wave bias
- Random starting tick (system time seed per session)
- Can drift partially off-screen (**minimum 45% visible**)

### Overlay Slots (4 independent)

| Slot | Cycle Period | Character |
|------|-------------|-----------|
| 0 | **1.5 bars** | Fast, energetic — always visible (30% alpha floor) |
| 1 | **2.5 bars** | Medium |
| 2 | **3.0 bars** | Slow, atmospheric |
| 3 | **2.0 bars** | Mid-tempo — always visible (30% alpha floor) |

- Overlays draw on **full grid** (no margin restriction)
- Small drift around center (not large row jumps)
- Scatter-dissolve on image change
- Filter + structural visibility applies to overlays (same as core)
- Settings (filter, bit depth glitch, SR quantization) affect overlays equally

---

## V3: Moment System

One moment active at a time. Each has duration + cooldown.

### Moments

| Moment | Trigger | Effect | Duration |
|--------|---------|--------|----------|
| **FreezeCut** | Transient + energy > 0.8 | Freeze velocity/motion, +10% brightness, dust continues | 5–20 frames |
| **GlitchBloom** | Transient + energy > 0.6 | Expanding glitch radius from seed cell (block/box chars) in `palette.primary` color | 15–25 frames |
| **LockIn** | Entering PEAK state | Overlays use same image as core (alignment moment) | 2 beats |
| **PhaseWave** | Energy > 0.7 (rare) | Horizontal sine displacement on core | 20–35 frames |
| **Collapse** | Exiting PEAK state | Coherent noise progressively removes cells | 25 frames |
| **Afterglow** | Auto after FreezeCut/GlitchBloom | Increased smearing + trail persistence | 20 frames |
| **UserAccent** | Rapid param change (filter/SR delta) | Brightness boost | 10 frames |

### Micro-freezes
- Lighter version of FreezeCut (3–8 frames)
- Triggered by transients when no moment is active
- Creates rhythmic punctuation

---

## V3: Memory System

```rust
heat = lerp(heat, smoothed_energy, 0.05)      // drives glitch scaling, overlay aggression
fatigue += glitch_events * 0.01; fatigue *= 0.98  // reduces glitch after heavy activity
afterimage = lerp(afterimage, energy, 0.1)     // drives smearing + trail persistence
```

---

## V3: Restraint System

- **Idle windows**: when energy < 0.25 and no active moment → dampen to 40% intensity
- **Recovery windows**: after moment ends (cooldown > 15 frames) → dampen to 60%
- **Fatigue**: reduces glitch probability after heavy activity (multiplier 0.2–1.0)

---

## V4: Phrase System

8-bar arcs driving modulation across overlays and brightness:

```
phrase_arc = sin(t / (ticks_per_bar * 8) × π)   // 0→1→0 over 8 bars
phrase_overlay_mod = 0.7 + phrase_arc × 0.3      // overlay alpha scaling
phrase_brightness_mod = 0.9 + phrase_arc × 0.1   // brightness pulsing
```

---

## V4: Intent Model

Three accumulating intent signals derived from audio energy trends:

```
intent_tension  += when energy rising steadily       × 0.95 decay
intent_release  += when energy dropping               × 0.95 decay
intent_chaos    += when energy erratic (high delta)   × 0.95 decay
```

The dominant intent biases moment selection:
- **Tension** → FreezeCut, LockIn
- **Release** → Afterglow, Collapse
- **Chaos** → GlitchBloom, PhaseWave

---

## V4: Anchor-Based Composition

Overlay positions are anchor-driven with role-specific pull rates and soft collision avoidance:

- 10 predefined anchor points across the grid
- Overlays drift toward their anchor with slot-specific pull (0.02–0.08)
- Collision avoidance pushes overlapping overlays apart
- Accent overlay retargets on transients

---

## V4: Coherent Glitch Field

Replaces per-cell random glitch with FBM-style layered noise for spatially coherent corruption:

```
glitch_field(col, row, phase) = 3 octaves of hash noise
  octave 1: scale 0.15, weight 0.5
  octave 2: scale 0.3,  weight 0.3
  octave 3: scale 0.6,  weight 0.2
```

---

## V4: Light Mode Rendering

`ColorPalette.is_light` computed from background luminance (> 0.18 linear). Adjustments:

| Effect | Dark Theme | Light Theme |
|--------|-----------|-------------|
| Brightness boost (FreezeCut, UserAccent) | Additive (+) | Subtractive (−) darkens for emphasis |
| Structural alpha floor | 0.15 | 0.35 |
| Dust opacity | base range | ×1.6 multiplier |
| Glitch corruption alpha | 0.15 + energy×0.2 | 0.30 + energy×0.35 |
| UI highlight | +40 RGB | −40 RGB |

---

## V5: Per-Preset Visual Profiles

Each sampler preset defines a `VisualProfile` controlling 14 parameters that make presets visually identifiable:

| Parameter | Controls | Range Across Presets |
|-----------|----------|---------------------|
| `row_damping` | Core vertical velocity drag | 0.85 (loose) → 0.95 (stable) |
| `col_damping` | Core horizontal velocity drag | 0.82 (loose) → 0.94 (stable) |
| `bpm_force` | BPM rhythmic push amplitude | 0.20 (gentle) → 0.45 (strong) |
| `dust_density` | Base dust particle density | 0.46 (clean) → 0.74 (gritty) |
| `glitch_mult` | Glitch probability multiplier | 0.15 (near-zero) → 1.65 (heavy) |
| `step_quant_mult` | SR temporal stepping scale | 0.7 (smooth) → 1.5 (coarse) |
| `smear_base` | Smear/trail base amount | 0.2 (crisp) → 0.4 (ghostly) |
| `transition_speed` | Image transition window (bars) | 0.3 (fast cuts) → 0.7 (slow fades) |
| `overlay_speed` | Overlay cycling speed mult | 0.8 (slow) → 1.4 (fast) |
| `micro_freeze_thresh` | Micro-freeze probability | 5 (rare) → 20 (frequent) |
| `moment_mult` | Moment trigger probability mult | 0.6 (rare) → 1.4 (frequent) |
| `dust_style` | 0=random, 1=grid, 2=chaotic | Per-preset character |
| `glitch_style` | 0=mixed, 1=h-line, 2=warped, 3=minimal | Per-preset character |
| `bloom_shape` | 0=rect, 1=scanline, 2=radial, 3=jagged | Per-preset character |

Profiles interpolate smoothly (~300ms) when switching presets.

---

## V5: Visual Enhancement Details

### Dust Styles
- **Grid-aligned** (SP-1200): structured digital stepping using quantized cell positions
- **Chaotic drift** (Mirage): heavy wave-based drift with irregular patterns
- **Random** (default): existing wave_mix oscillation between random scatter and structured waves

### Glitch Styles
- **Horizontal-line** (SP-1200, SP-303): only block elements (indices 94-105) for scanline aesthetic
- **Warped-melt** (Mirage): wide character range including digits + blocks for melted look
- **Minimal** (S950, MPC3000): only light chars + thin blocks for subtle artifacts
- **Mixed** (default): full CHARSET range

### GlitchBloom Shapes
- **Scanline** (SP-1200): horizontal line burst (±1 row, ±2× radius columns)
- **Radial** (S950, P-2000): circular expansion using distance check
- **Jagged** (Mirage): irregular per-row radius variation
- **Rectangle** (default): existing square expansion

### Afterglow Accent Tint
When Afterglow is active, color subtly drifts toward `palette.emphasis` (the themed accent). Fades exponentially as the afterglow decays. Produces gold afterglow on Rooney, lime on Noni, teal on Kerama.

### Transient Emphasis Flash
Transient hits produce a brief accent-colored flash (10% tint toward `palette.emphasis`) in addition to brightness boost. Makes transients feel themed.

### Phrase-Coupled Color Drift
Color drift amplitude scales with phrase arc: 0.02 at phrase valleys (settled), 0.08 at peaks (active). Makes the visual "breathe" musically over 8-bar phrases.

### Signal-Class Transition Sharpness
- **Percussive**: 4× transition speed (hard cuts — impact-driven)
- **Ambient**: 0.5× transition speed (soft ghostly merges)
- **Tonal**: 1× (normal dissolve)

---

## V5: Global Polish

### Visual Filter Remap
Filter parameter (0-1) is perceptually remapped for the visual system only (audio DSP unchanged):
- 0.0-0.3 → 0.0-0.5 (dramatic fragmentation range, compressed)
- 0.3-0.7 → 0.5-0.85 (musical sweet spot, expanded)
- 0.7-1.0 → 0.85-1.0 (subtle refinement, compressed)

### Global Restraint
- Damping nudged +2% toward 1.0 (slightly more stable globally)
- Dust density reduced ~0.04 across all profiles
- Glitch multipliers reduced ~15% across all profiles
- Produces "held back" feel rather than overwhelming

---

## Timing & Pacing

### Two Clocks

| Clock | Drives | When Stopped |
|-------|--------|-------------|
| `anim_tick` | Image cycling, scrolling, overlay fade, transitions | **Freezes** |
| `dust_tick` (`frame_update_counter`) | Dust noise, dust positions | **Always advances** |

### BPM Source

```
Effective BPM = host BPM (if ≤115) or host BPM / 2 (if >115, half-time)
ticks_per_beat = 3600 / BPM, ticks_per_bar = beat × 4
```

---

## Sample Rate → Temporal Quantization

### Stepping
```
sr_norm = target_sr / 96000
step_interval = lerp(1, 8, 1 - sr_norm)   frames between updates
```

### Smearing
```
smear_factor = (1 - sr_norm) × 0.3
effective_smear = smear_factor + afterglow + afterimage × 0.15   (capped at 0.8)
```

---

## Filter → Structural Visibility

Per-cell coherent noise compared to filter threshold. Affects **both core and overlay** cells:

```
coherent_noise = center × 0.6 + avg(4 neighbors) × 0.4
if coherent_noise > filter_val: alpha = 0.15 (dark themes) / 0.35 (light themes)
else: alpha = 1.0 (full)
```

---

## DSP Parameter → Visual Mapping

| Parameter | Range | Effect |
|-----------|-------|--------|
| **Sample Rate** | 1k–96k Hz | Temporal quantization: low SR = stepped motion + ghosting |
| **Filter** | 0–1 | Structural visibility (coherent masking on core + overlays) + layer priority |
| **Mix** | 0–1 | Overlay density (2%→100%) + speed + max 80% alpha |
| **Bit Depth** | 1–24 | Tiered corruption: 16-12=none, 11-9=point, 8-6=cluster, 5-4=structural |
| **Jitter** | 0–1 | No direct visual effect |
| **BPM** | host | All timing: cycling, update cap |
| **Playing** | host | Images freeze. Dust keeps moving. |

---

## Layer Compositing (Back to Front)

1. **Background** — `palette.background` exact sRGB
2. **Overlay Images** — full grid, filter + structural alpha applied, scatter-dissolve transitions
3. **Core Image** — on top, velocity-based scroll, min 30% on-screen, wave-biased dissolve
4. **Dust** — always animating, energy-coupled density (0.66 + energy × 0.2)
5. **Glitch** — bit depth < 12 only, fatigue-scaled probability, V4 coherent glitch field
6. **V3 Moments** — GlitchBloom overlay, Collapse cell removal, brightness boosts
7. **V4 Phrase + Light mode** — brightness boost (additive on dark, subtractive on light)
8. **V3 Restraint** — idle/recovery dampening applied last
8. **UI Overlay** — femtovg text rendered AFTER grid (never in framebuffer, never affected by animation)

---

## Embedded UI System

### Rendering
UI text rendered as a **femtovg overlay** on top of the ASCII grid in `ascii_image_display.rs::draw()`. Never stamped into the framebuffer — animation underneath is never overwritten.

### Visibility
- **Title** ("sssssssssampler") in primary/pop color — always visible at row 1, col 3
- **Menu** — only visible when mouse is in the **top-left quarter** of the display (or while dragging)
- **Hover highlight** — hovered row brightens by +30 RGB

### Layout (grid rows 3+)

**Always visible:**
```
Row 3: sr: 48.0k        (click-drag)
Row 4: filter: 100%     (click-drag)
Row 5: aa: on            (click toggle)
Row 6: [ more ]          (click to expand)
```

**Expanded:**
```
Row 6: [ less ]          (click to collapse)
Row 7: < S950 >          (click left=prev, right=next)
Row 8: bits: 12.0        (click-drag)
Row 9: jitter: 1.0%      (click-drag)
Row 10: mix: 100%        (click-drag)
Row 11: theme: noni      (click to cycle 14 themes)
Row 12: mode: dark       (click to toggle dark/light)
Row 13: feel: expressive (click to cycle tight/expressive/chaotic)
```

### Interaction
- **Drag**: `delta = delta_x + delta_y` maps to value change
- **Sample Rate**: logarithmic scaling (1k–96k)
- **Filter, Jitter, Mix**: linear (0–1), sensitivity 0.004
- **Bit Depth**: linear (1–24), sensitivity 0.06
- **Anti-Alias**: click toggle
- **Preset**: click left half = prev, right half = next machine
- **Theme**: click to cycle through 5 themes
- Cursor captured and locked during drag

### Theme colors in FrameBuffer
`FrameBuffer` carries `primary_rgb`, `emphasis_rgb`, `preset_idx`, `theme_idx` (0-13), `dark_mode`, `feel_idx`, `energy`, and `is_light` so the display can render UI text in the correct theme colors and adapt highlight direction without needing access to the palette.

---

## CHARSET (124 chars)

```
0–83:    Standard ASCII (artwork-safe, exact match preserved)
84–86:   Additional ASCII from sources: " m 8
87–93:   Missing digits: 2 3 4 5 6 7 9
94–105:  Block elements (▏▎▖▗▘▝▍▚▞▌▐▄▀░▒▓▙▛▜▟▇█)
106–123: Box drawing (─│┌┐└┘├┤┬┴┼═║╔╗╚╝╬)
```

---

## Themes (14 × light/dark)

Ported from the Coconut Creme design system. Each theme has independent light and dark mode variants. Theme and mode are separate controls in the UI.

| # | Name | Primary Hue | Accent Hue | Mood |
|---|------|-------------|------------|------|
| 0 | Pink | 340 (pink) | 355 (rose) | Warm, confident |
| 1 | Kerama | 250 (cobalt) | 195 (teal) | Deep ocean |
| 2 | Brazil | 145 (green) | 95 (yellow) | Flag, tropical |
| 3 | **Noni** (default) | 118 (olive) | 122 (lime) | Earthy, fresh |
| 4 | Paris | 328 (fuchsia) | 72 (gold) | Glamour, loud |
| 5 | Rooney | 22 (red) | 78 (gold) | Football, bold |
| 6 | k+k | 260 (gray) | 260 (gray) | Minimalist |
| 7 | Catppuccin | 300 (mauve) | 265 (lavender) | Pastel, cozy |
| 8 | Kanagawa | 222 (wave blue) | 80 (amber) | Ukiyo-e, inky |
| 9 | Rosé Pine | 0 (rose) | 300 (iris) | Romantic, muted |
| 10 | Dracula | 295 (purple) | 340 (pink) | Syntax, vivid |
| 11 | Papaya | 50 (orange) | 45 (orange) | Motorsport |
| 12 | Dominican | 264 (royal blue) | 22 (red) | Baseball, patriotic |
| 13 | Calsonic | 260 (ocean blue) | 18 (coral) | JDM racing |

**Emphasis/flash color** is set to each theme's accent color (not generic white/black), so glitch corruption carries the theme identity.

---

## Machine Presets (7)

| Name | Sample Rate | Bit Depth | Poles | Character | Hardware Evidence |
|------|------------|-----------|-------|-----------|-------------------|
| SP-1200 (default) | 26,040 Hz | 12-bit | 2-pole | Gritty, aliasing, punchy | AD7541 DAC, SSM2044 VCF |
| MPC60 | 40,000 Hz | 12-bit | 4-pole | Clean 12-bit, punch | Burr Brown PCM54HP DAC |
| S950 | 48,000 Hz | 12-bit | 6-pole | Smooth, warm | MF6CN-50 36 dB/oct |
| Mirage | 33,000 Hz | 8-bit | 4-pole | 8-bit gritty, warm | CEM3328 resonant filter |
| P-2000 | 41,667 Hz | 12-bit | 4-pole | Analog filter sweeps | CEM3379 resonant VCF |
| MPC3000 | 44,100 Hz | 16-bit | 4-pole | Clean reference | PCM69A 18-bit DAC |
| SP-303 | 44,100 Hz | 16-bit | 4-pole | Clean + digital FX | COSM DSP |

---

## Per-Frame State (EditorData)

```rust
// V2: Animation
smoothed_energy: f32
velocity_row: f32
velocity_col: f32
quant_frame: u64
prev_row_scroll: f32
prev_col_drift: f32
prev_overlay_rows: [f32; 4]
prev_overlay_cols: [f32; 4]

// V3: Moments & Memory
moment: MomentState { active, timer, duration, cooldown, seed, bloom_center }
memory: MemoryState { heat, fatigue, afterimage }
micro_freeze_frames: u32
prev_energy_state: u8
prev_filter: f32
prev_sr: f32
glitch_events_this_frame: u32

// V4: Phrase, Intent, Composition
phrase_tick: f32
intent_tension: f32
intent_release: f32
intent_chaos: f32
core_pos: (f32, f32)
core_anchor: (f32, f32)
overlay_anchors: [(f32, f32); 4]
overlay_positions: [(f32, f32); 4]
accent_slot_alpha: f32
glitch_field_phase: f32
recent_moment_count: u32
```

---

## Key Files

| File | Purpose |
|------|---------|
| `ascii.txt` | All 38 ASCII art images (`#N` separated) |
| `src/editor.rs` | Animation loop, compositing, moments, memory, phrase, intent, anchors |
| `src/ascii_image_display.rs` | femtovg rendering, UI overlay, mouse interaction |
| `src/ascii_bank.rs` | CHARSET (124 chars), image parsing, density/complexity metrics |
| `src/audio_feed.rs` | AnimationParams (energy, transient, BPM, playing) |
| `src/render/color_system.rs` | ColorPalette (14 themes × light/dark, is_light flag, themed emphasis) |
| `src/render/offscreen.rs` | FrameBuffer struct (pixels + theme colors + indices + is_light) |
| `src/render/audio_analysis.rs` | AudioAnalyzer (RMS, transient detection) |
| `src/render/layer_engine.rs` | Layer state management (anchor layer + 4 overlays) |
| `src/lib.rs` | DSP: sample-and-hold, bit crush, 2/4/6-pole filter, params |
