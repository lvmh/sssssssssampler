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
| `src/ascii_bank.rs` | CHARSET (124 chars), image parsing, density mapping |
| `src/audio_feed.rs` | AnimationParams struct (BPM, playing, RMS, etc.) |
| `src/render/offscreen.rs` | FrameBuffer struct |
| `src/render/color_system.rs` | ColorPalette per theme (5 themes) |
| `assets/style.css` | Window layout (540×600), theme CSS |
| `assets/img01.txt`–`img20.txt` | 20 raw ASCII art source images |

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

## CHARSET (124 chars)

```
0–83:    Standard ASCII (artwork-safe — exact match always preserved)
         Space . ' ` , : ; - ~ _ ! i l 1 I r c v u n x z j f t
         L C J Y F o a e s y h k d b p q g S Z w K U X T H
         R E D N V A Q P B G O M 0 W ^ / | \ < > ( ) + = [ ] { } * % # & $ @

84–105:  Block elements (▏▎▖▗▘▝▍▚▞▌▐▄▀░▒▓▙▛▜▟▇█)
106–123: Box drawing (─│┌┐└┘├┤┬┴┼═║╔╗╚╝╬)
```

**Rule**: Indices 84+ (blocks/box) are NEVER used in normal rendering. All displayed chars are clamped to 0–83 (pure ASCII). Block elements only appear during glitch mode (bit depth < 11), affecting max 1–4% of characters.

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

### Play/Stop

```
Playing:  anim_tick += 1 every frame
Stopped:  anim_tick += 1 every 8th frame (1/8 speed "twist down")
          Overlay alpha × 0.15 (dims to 15%)
```

---

## DSP Parameter → Visual Mapping

| Parameter | Range | Visual Effect |
|-----------|-------|---------------|
| **Filter** | 0–1 | Base image opacity only: 0→invisible, 1→full. No effect on overlays. |
| **Mix** | 0–1 | Overlay opacity: mix×0.80 (so mix@100%→80% overlay, mix@0%→invisible). No effect on base image. |
| **Bit depth** | 4–16 | Glitch mode: only below 11. Scales 0% at 11 → 20% at 4. Of affected chars, 1–4% get block element replacements |
| **Jitter** | 0–1 | No direct visual effect currently |
| **BPM** | host | All timing: scroll speed, overlay fade period, overlay drift speed |
| **Playing** | host | Full speed vs 1/8 twist-down; overlay dims to 15% when stopped |

---

## Layer Compositing

Each cell is composited in this order (back to front):

### Layer 1: Background
- Solid fill with `palette.background` (exact theme color via `bg_rgb`)

### Layer 2: Overlay Images (imgs 1–19)
- **3 independent slots**, each cycling through images 1–19
- **ALL overlay characters render** (no sparse mask — full images visible)
- Composited as transparency over background: `bg + (overlay_color - bg) × alpha`
- Alpha = `slot_fade × (0.60 + density×0.30) × overlay_visibility`
- `overlay_visibility = mix × 0.80` (max 80% at mix=100%, no filter interaction)
- Only 5% of overlay chars affected by dust
- 6-row margin top/bottom (empty)

**Per-slot properties:**
```
Fade:       sine wave over 2 bars, 3 slots phase-offset by 2/3 bar
Image:      cycles through imgs 1–19, advances each fade period
Row scroll: independent speed per slot (1.0–2.0× half-note rate)
Col shift:  ±15 random base (per cycle) + ±8 sinusoidal drift (continuous)
Color:      palette.secondary[slot_index % 4]
```

### Layer 3: Base Image (img 0) — composited ON TOP of overlay
- **Highest priority** — always sits on top of overlays
- Composited over whatever is behind it: `behind + (primary - behind) × alpha`
- Alpha = `base_visibility × (0.85 + density×0.15)` where `base_visibility = filter`
- Ping-pong scrolls over 4 bars (forward then reverse)
- Horizontal compound drift: `sin(t/4bars) × 6 + sin(t×2.7) × 3` columns
- 2-row margin top/bottom (empty)

**Per-frame life**: 5% of base cells get ±1 char index jitter each frame (stays in ASCII 1–83). Makes the image shimmer rather than look static.

### Layer 4: Dust Particles (empty cells only)
- **66% of empty cells** filled with dust
- Glyphs: ASCII punctuation only (indices 1–6: `. ' \` , : ;`)
- Never uses block elements
- Color: `palette.secondary[3]` blended over background at 6–50% opacity
- Opacity: power curve (exponent 0.35) for varied brightness

### Layer 5: Glitch (bit depth effect)
- **Only active when bit_depth < 11**
- Probability: 0% at 11 bits → 20% at 4 bits
- Of those, only 1–4% of chars get a visible block element replacement (indices 84–105)
- Color: shifts 30% toward `palette.emphasis`
- This is the ONLY way block/box characters ever appear on screen

---

## Color Rules

**All color is transparency-based.** Colors are always at full saturation; only opacity varies.

```
Compositing formula (everywhere):
  result = background + (foreground - background) × alpha

Never:
  color × multiplier  (darkens toward black, kills saturation)

Always:
  bg + (color - bg) × alpha  (true transparency blend)
```

Linear RGB → sRGB conversion: `srgb = linear.powf(1.0/2.2) × 255`

---

## Overlay Slot System

```rust
struct OverlaySlot {
    img_idx: usize,     // Which image (1–19)
    alpha: f32,         // Fade level (0–1, from sine wave)
    row_shift: usize,   // Vertical scroll offset
    col_shift: i32,     // Horizontal position (wanders)
    color_idx: usize,   // palette.secondary index (0–3)
    render_seed: u32,   // Hash seed (unused for masking now)
}
```

### Lifecycle

```
t=0                    t=fade_period/2        t=fade_period
│                      │                      │
▼                      ▼                      ▼
fade in ──── peak ──── fade out ──── invisible ──── next image
   sin wave above threshold=0.15
```

Three slots are phase-offset so they stagger: when one peaks, another is fading in.

---

## Themes (5)

| Name | `palette.background` | `palette.primary` | Mood |
|------|---------------------|-------------------|------|
| Noni Dark | `#151805` | `#9BB940` lime | Deep forest |
| Noni Light | `#F1F3EA` | `#6D8000` olive | Sage daylight |
| Paris | `#140813` | `#FF5FFF` magenta | Midnight pink |
| Rooney | `#140001` | `#FC000B` red | Man Utd red |
| Brazil Light | `#F4FAF4` | `#007500` green | Forest teal |

Each theme has 4 `secondary` colors (used by overlay slots and dust) plus `emphasis` (glitch target).

---

## Display (ascii_image_display.rs)

### Font Loading

Runtime search (fast builds, no `include_bytes!`):
```
~/Library/Fonts/FiraCodeNerdFontMono-Regular.ttf  (preferred)
~/Library/Fonts/FiraCodeNerdFont-Regular.ttf
~/Library/Fonts/FiraCode-Regular.ttf
/Users/calmingwaterpad/Library/Fonts/...          (hardcoded VST fallback)
/System/Library/Fonts/Menlo.ttc                   (system fallback)
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

Canvas filled with `fb.bg_rgb` — exact theme background color stored in the FrameBuffer.

---

## Grid Constants

```rust
COLS = 46              // Display columns
ROWS = 36              // Display rows (= bank height, min 28 requirement met)
BANK_COLS = 46         // Source image width
BANK_ROWS = 36         // Source image height
BASE_MARGIN = 2        // Empty rows top/bottom for base
OVERLAY_MARGIN = 6     // Empty rows top/bottom for overlays
```

---

## Noise (no rand crate)

```rust
noise_seed = col × 1664525 + row × 22695477 + anim_tick × 134775813
noise        = bits[16..23] / 255   // General purpose
dust_present = bits[8..15]  / 255   // Dust threshold
dust_opacity = bits[0..7]   / 255   // Dust brightness
```

3 independent pseudo-random values per cell per frame. Deterministic, zero allocation.

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

## Image Bank

20 ASCII art files parsed at startup into `AsciiBank`:
- `img01.txt` = base image (always present, primary color)
- `img02–20.txt` = overlay pool (cycled through 3 slots)

Parsing: `raw text → char_to_idx() per char → AsciiGrid → resized(46,36)`

`char_to_idx()` exact-matches first (artwork letters preserved), then falls back to nearest visual density.

---

## Performance

- CPU compositing only, flat `Vec<u8>`, no GPU
- Zero per-frame allocation (FrameBuffer pre-allocated)
- Deterministic noise (integer hashing, no `rand`)
- Font loaded once, cached in `RefCell<Option<FontId>>`
- ~60fps, one UpdateFrameBuffer per Vizia event cycle
- Lock held briefly per frame (`Mutex<Option<FrameBuffer>>`)
