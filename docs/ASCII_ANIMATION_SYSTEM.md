# ASCII Animation System V3

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
  └─ Writes FrameBuffer (46×36, RGBA8 + theme colors + preset/theme idx)
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

All images loaded from **`ascii.txt`** (root directory) using `#N` separator format. Currently **22 images**. Parsed at startup by `AsciiBank::from_ascii_txt()`. Each image stored at **native resolution** — no resizing, no distortion. `get_cell()` returns 0 for out-of-bounds.

---

## Image Cycling (BPM-Synced)

### Core Image
- **Changes every 2 bars**
- Random order (hash-based, not sequential)
- **Scatter-dissolve transition** over half a bar with wave bias
- Random starting tick (system time seed per session)
- Can drift partially off-screen (**minimum 30% visible**)

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
| **GlitchBloom** | Transient + energy > 0.6 | Expanding glitch radius from seed cell (block/box chars) | 15–25 frames |
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
if coherent_noise > filter_val: alpha = 0.15 (dimmed)
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
5. **Glitch** — bit depth < 12 only, fatigue-scaled probability, full CHARSET
6. **V3 Moments** — GlitchBloom overlay, Collapse cell removal, brightness boosts
7. **V3 Restraint** — idle/recovery dampening applied last
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
Row 11: theme: noni dark (click to cycle)
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
`FrameBuffer` carries `primary_rgb`, `emphasis_rgb`, `preset_idx`, `theme_idx` so the display can render UI text in the correct theme colors without needing access to the palette.

---

## CHARSET (131 chars)

```
0–83:    Standard ASCII (artwork-safe, exact match preserved)
84–86:   Additional ASCII from sources: " m 8
87–93:   Missing digits: 2 3 4 5 6 7 9
94–114:  Block elements (▏▎▖▗▘▝▍▚▞▌▐▄▀░▒▓▙▛▜▟▇█)
115–130: Box drawing (─│┌┐└┘├┤┬┴┼═║╔╗╚╝╬)
```

---

## Themes (5)

| Name | Background | Primary | Mood |
|------|-----------|---------|------|
| Noni Dark (default) | `#151805` | `#9BB940` | Deep forest lime |
| Noni Light | `#F1F3EA` | `#6D8000` | Sage daylight |
| Paris | `#140813` | `#FF5FFF` | Midnight hot pink + gold |
| Rooney | `#140001` | `#FC000B` | Man Utd red + gold |
| Brazil Light | `#F4FAF4` | `#007500` | Forest teal + gold |

---

## Machine Presets (6)

| Name | Sample Rate | Bit Depth | Poles | Character |
|------|------------|-----------|-------|-----------|
| SP-1200 | 26,040 Hz | 12-bit | 2-pole | Gritty, under-filtered |
| SP-12 | 27,500 Hz | 12-bit | 2-pole | Gritty |
| S612 | 32,000 Hz | 12-bit | 4-pole | Clean 4th-order |
| SP-303 | 44,100 Hz | 16-bit | 4-pole | Clean hi-fi |
| S950 (default) | 48,000 Hz | 12-bit | 6-pole | 36 dB/oct switched-cap |
| MPC3000 | 44,100 Hz | 16-bit | 4-pole | Transparent |

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
ui_expanded: bool
```

---

## Key Files

| File | Purpose |
|------|---------|
| `ascii.txt` | All ASCII art images (`#N` separated) |
| `src/editor.rs` | Animation loop, compositing, moments, memory, restraint |
| `src/ascii_image_display.rs` | femtovg rendering, UI overlay, mouse interaction |
| `src/ascii_bank.rs` | CHARSET (131 chars), image parsing |
| `src/audio_feed.rs` | AnimationParams (energy, transient, BPM, playing) |
| `src/render/color_system.rs` | ColorPalette (5 themes) |
| `src/render/offscreen.rs` | FrameBuffer struct (pixels + theme colors + indices) |
| `src/render/audio_analysis.rs` | AudioAnalyzer (RMS, transient detection) |
| `src/lib.rs` | DSP: sample-and-hold, bit crush, 2/4/6-pole filter, params |
