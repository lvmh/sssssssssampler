# ASCII Animation System

Technical documentation for the live ASCII art animation in sssssssssampler.

---

## Architecture

```
Audio Thread (lib.rs)
  ├─ DSP: sample-and-hold, bit crush, filter
  ├─ Reads host transport → BPM, playing state
  └─ Updates AnimationParams via AudioFeed
         │
         │ Arc<Mutex<AnimationParams>>
         ▼
Editor Model (editor.rs :: UpdateFrameBuffer)
  ├─ Reads AnimationParams + DSP params every frame
  ├─ Composites layers: overlay → base → dust → glitch
  └─ Writes FrameBuffer (46×36, RGBA8)
         │
         │ Arc<Mutex<Option<FrameBuffer>>>
         ▼
Display View (ascii_image_display.rs)
  ├─ femtovg Canvas, FiraCode Nerd Font (runtime loaded)
  ├─ Reads FrameBuffer: RGB=color, A=CHARSET index
  └─ Renders each cell as a centered monospace glyph
```

### Key Files

| File | Purpose |
|------|---------|
| `src/editor.rs` | Animation loop, all compositing logic |
| `src/ascii_image_display.rs` | femtovg rendering, font loading, cell sizing |
| `src/ascii_bank.rs` | CHARSET (127 chars), image parsing |
| `src/audio_feed.rs` | AnimationParams struct (BPM, playing, RMS, etc.) |
| `src/render/offscreen.rs` | FrameBuffer struct |
| `src/render/color_system.rs` | ColorPalette per theme (5 themes) |
| `assets/style.css` | Window layout (540×600), theme CSS |
| `assets/img01.txt`–`img20.txt` | 20 raw ASCII art source images |

---

## Core Rule: Character Preservation

**ASCII artwork characters pass through exactly as they appear in the source files.**

- `char_to_idx()` does exact match only — if a char isn't in CHARSET, it becomes space (0)
- No density approximation, no substitution
- The ONLY way a character changes from source is the **2% glitch** (bit depth < 11)
- This applies to both core and overlay images equally

---

## FrameBuffer

```rust
struct FrameBuffer {
    width: u32,         // 46
    height: u32,        // 36
    pixels: Vec<u8>,    // width × height × 4 bytes
    bg_rgb: [u8; 3],    // Theme background sRGB (for canvas fill)
}
```

**Per-pixel** (4 bytes):
- `[0]` R, `[1]` G, `[2]` B — sRGB color (gamma-corrected)
- `[3]` **CHARSET index** (NOT alpha) — which glyph to render

---

## CHARSET (127 chars)

```
0–83:    Standard ASCII (artwork-safe — exact match preserved)
         Space . ' ` , : ; - ~ _ ! i l 1 I r c v u n x z j f t
         L C J Y F o a e s y h k d b p q g S Z w K U X T H
         R E D N V A Q P B G O M 0 W ^ / | \ < > ( ) + = [ ] { } * % # & $ @

84–86:   Additional ASCII found in source images: " m 8

87–108:  Block elements (▏▎▖▗▘▝▍▚▞▌▐▄▀░▒▓▙▛▜▟▇█)
109–126: Box drawing (─│┌┐└┘├┤┬┴┼═║╔╗╚╝╬)
```

**Indices 87+ (blocks/box)**: NEVER used in normal rendering. All displayed chars are clamped to 0–86 (ASCII). Block/box elements only appear during the 2% glitch effect.

---

## Image Cycling (BPM-Synced)

All 20 images participate as both core and overlay. No image is permanently assigned to any role.

### Core Image
- **Changes every 4 bars**
- Cycles through all 20 images: `core_img = (t / (ticks_per_bar * 4)) % 20`
- **Scatter-dissolve transition** over half a bar when changing:
  - Per-cell hash determines old vs new: `hash < transition_progress → new, else old`
  - Creates a random pixel-by-pixel overwrite effect

### Overlay Slots (3 independent)
| Slot | Cycle Period | Character |
|------|-------------|-----------|
| 0 | **4 bars** | Fast, energetic |
| 1 | **6 bars** | Medium |
| 2 | **8 bars** | Slow, atmospheric |

- Each slot picks an image from the pool, **skipping the current core image** (no duplicates)
- Each slot moves independently (own scroll speed + sinusoidal col drift)
- Fade: sine wave over the slot's period, phase-offset between slots

---

## Timing

All animation is BPM-synced via host transport.

```
Effective BPM = host BPM (if ≤115) or host BPM / 2 (if >115, half-time)

At 120 BPM effective:
  ticks_per_beat  = 30    (3600 / BPM)
  ticks_per_bar   = 120   (beat × 4)
  ticks_per_half  = 60    (beat × 2)
```

### Two Independent Clocks

| Clock | Drives | When Stopped |
|-------|--------|-------------|
| `anim_tick` | Image cycling, scrolling, overlay fade, transitions | **Freezes** — all images pause in place |
| `dust_tick` (`frame_update_counter`) | Dust noise seed, dust particle positions | **Always advances** — dust keeps moving |

When transport is paused:
- All images (core + overlays) freeze at their current position and stay fully visible
- Dust particles continue animating over the frozen images
- When transport resumes, images continue cycling from where they paused

---

## DSP Parameter → Visual Mapping

| Parameter | Range | Visual Effect |
|-----------|-------|---------------|
| **Filter** | 0–1 | Base/core image opacity only: 0→invisible, 1→full. No effect on overlays. |
| **Mix** | 0–1 | Overlay opacity: `mix × 0.80` (max 80% at mix=100%). No effect on core image. |
| **Bit depth** | 4–16 | Glitch: only below 11. Max 2% of chars get replaced with full CHARSET (blocks, box drawing). |
| **Jitter** | 0–1 | No direct visual effect. |
| **BPM** | host | All timing: image cycling (4/6/8 bars), scroll speed, overlay drift. |
| **Playing** | host | Images freeze when stopped. Dust keeps playing. |

---

## Layer Compositing

Each cell is composited back-to-front:

### Layer 1: Background
- Solid fill with `palette.background` (exact sRGB via `bg_rgb`)

### Layer 2: Overlay Images
- **All overlay characters render** exactly as source (no masking, no density modification)
- Alpha blend over background: `bg + (overlay_color - bg) × alpha`
- Alpha = `slot_fade × 0.80 × overlay_visibility`
- `overlay_visibility = mix × 0.80`
- 6-row margin top/bottom
- Only 2% of overlay chars affected by dust
- **Never wraps** — if row scroll pushes past image bounds, cell shows empty

**Per-slot movement:**
```
Row scroll: independent speed (1.0–2.0× half-note rate)
Col shift:  ±15 random base (per cycle) + ±8 sinusoidal drift (continuous)
Color:      palette.secondary[slot_index % 4]
```

### Layer 3: Core Image — composited ON TOP of overlay
- **Highest priority** — always sits on top
- Alpha = `base_visibility × (0.85 + density × 0.15)` where `base_visibility = filter`
- Ping-pong scroll over 4 bars
- Horizontal compound drift: `sin(phase) × 6 + sin(phase × 2.7) × 3` columns
- 2-row margin top/bottom
- **Never wraps** — if scroll pushes past image bounds, cells show empty instead of wrapping to the other side of the image
- **0.2% per-frame jitter**: subtle ±1 char index shift for shimmer (stays in ASCII 1–83)

### Layer 4: Dust Particles (empty cells only)
- **66% of empty cells** filled
- Glyphs: ASCII punctuation only (indices 1–6: `. ' \` , : ;`)
- Color: `palette.secondary[3]` at 6–50% opacity
- **Always animating** — uses `dust_tick` which never pauses

### Layer 5: Glitch (bit depth < 11 only)
- **Hard cap: 0.2% of chars maximum** (~1 in 500 per frame)
- Replaces char with ANY from full CHARSET (blocks, box drawing, all of it)
- Color shifts 30% toward `palette.emphasis`
- This is the ONLY way characters ever differ from source

---

## Color Rules

**All color is transparency-based.** Full saturation always; only opacity varies.

```
Formula (everywhere):  result = background + (foreground - background) × alpha
Never:                 color × multiplier  (darkens toward black)
```

Linear RGB → sRGB: `srgb = linear.powf(1.0/2.2) × 255`

---

## Themes (5)

| Name | Background | Primary | Mood |
|------|-----------|---------|------|
| Noni Dark | `#151805` | `#9BB940` lime | Deep forest |
| Noni Light | `#F1F3EA` | `#6D8000` olive | Sage daylight |
| Paris | `#140813` | `#FF5FFF` magenta | Midnight pink |
| Rooney | `#140001` | `#FC000B` red | Man Utd red |
| Brazil Light | `#F4FAF4` | `#007500` green | Forest teal |

Each has 4 `secondary` colors (overlay slots + dust) and `emphasis` (glitch target).

---

## Display (ascii_image_display.rs)

### Font Loading
Runtime search (not compile-time, fast builds):
```
~/Library/Fonts/FiraCodeNerdFontMono-Regular.ttf  (preferred)
~/Library/Fonts/FiraCodeNerdFont-Regular.ttf
~/Library/Fonts/FiraCode-Regular.ttf
/Users/calmingwaterpad/Library/Fonts/...          (hardcoded VST fallback)
/System/Library/Fonts/Menlo.ttc
/System/Library/Fonts/Monaco.ttf
```

### Cell Sizing
```
Monospace aspect: 0.60 (width/height)
cell_h = min(bounds.h/rows, bounds.w/cols/0.60)
cell_w = cell_h × 0.60
font_size = cell_h × 0.95
Grid centered within bounds.
```

### Background
Canvas filled with `fb.bg_rgb` — exact theme background color.

---

## Grid Constants

```rust
COLS = 46              // Display columns
ROWS = 36              // Display rows (= bank height)
BANK_COLS = 46         // Source image width
BANK_ROWS = 36         // Source image height
BASE_MARGIN = 2        // Empty rows top/bottom for core
OVERLAY_MARGIN = 6     // Empty rows top/bottom for overlays
NUM_SLOTS = 3          // Overlay slots
SLOT_BARS = [4, 6, 8]  // Bars per overlay cycle
```

---

## Scatter-Dissolve Transition

When the core image changes (every 4 bars):

```
time_in_cycle = t % core_cycle_len
transition_window = half a bar
transition_progress = time_in_cycle / transition_window  (0→1)

Per cell:
  hash(col, row, cycle) → value 0–1
  if value < transition_progress → show NEW image char
  else → show OLD image char
```

Over half a bar, random cells flip from old→new, creating a scatter effect.

---

## Noise (no rand crate)

```rust
noise_seed = col × 1664525 + row × 22695477 + dust_tick × 134775813
noise        = bits[16..23] / 255
dust_present = bits[8..15]  / 255
dust_opacity = bits[0..7]   / 255
```

Uses `dust_tick` (always advances) so dust keeps moving when paused.

---

## Window Layout

```
540 × 600 px

┌─────────────────────────┐
│ Header (44px)           │  "sssssssssampler" + theme pills
├─────────────────────────┤
│ ASCII Display (stretch) │  AsciiImageDisplay view
├─────────────────────────┤
│ Preset Row (32px)       │  ◄ S950 ►
├─────────────────────────┤
│ Controls (88px)         │  SR | BITS | JITTER | FILTER | MIX | AA
└─────────────────────────┘
```

---

## Performance

- CPU compositing, flat `Vec<u8>`, no GPU
- Zero per-frame allocation
- Deterministic noise (integer hashing)
- Font loaded once, cached
- ~60fps via Vizia event cycle
