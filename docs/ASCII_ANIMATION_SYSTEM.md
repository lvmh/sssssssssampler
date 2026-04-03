# ASCII Animation System — V9

Visual engine for sssssssssampler — a responsive visual instrument driven by audio and interaction.

> **V6** adds braille sub-cell effects (spark burst, edge fringe, background grain) and two new dust modes (vertical rain, beat ring pulse). Together these bring the total effect count to 21 visual layers across 9 integrated modules.
>
> **V6.1** makes all effects braille-reactive: GlitchBloom, Corruption, Collapse, and Jitter now mutate braille dot patterns (XOR/AND/shift) instead of replacing braille with ASCII. Density alpha uses dot-count/8 for braille cells. The `should_update` quantization gate now applies to braille identically to ASCII — braille art flickers and stutters with bandwidth.
>
> **V7** — Deep braille integration: BPM wave, braille dissolve transitions, echo trails, energy-gated multi-dot patterns, global braille tint.
>
> **V8** — Perceptual hierarchy refactor. **Energy = intent, not density.** Ambient effects are restructured as exclusive states — only one ambient layer is active at a time based on energy state. Added visual breathing (8-bar slow sine). Moments suppress the ambient layer entirely. Global braille tint removed — each effect carries its own semantic color. Spark burst and ring pulse draw from a per-frame braille pattern palette (shifts every half-bar) shared with grain, so at most 4 dot-types appear simultaneously.
>
> **V8 state machine:**
> ```
> IDLE  (energy < 0.22 ) → grain + fringe (silence texture)
> FLOW  (0.22 – 0.55)    → rain + fringe  (vertical motion)
> BUILD (0.55 – 0.82)    → BPM wave       (horizontal beat sweep)
> PEAK  (energy > 0.82)  → none ambient   (moments dominate)
> During GlitchBloom / Collapse / PhaseWave → ALL ambient suppressed
> ```
>
> **V9** — Musical timing engine. **Visuals peak ON the beat, not after it.** An anticipation scheduler detects transients and pre-schedules ring/spark effects to arrive exactly on the next beat. Moments are gated to downbeats only (base_prob compensated to preserve density). Phrase boundaries (4-bar/8-bar) trigger forced moments. Ambient effects now pulse with musical rhythm — grain fades with beat phase, fringe swells on beat 1, rain accelerates at bar start.
>
> **V9 anticipation model:**
> ```
> Transient detected → compute frames_to_next_beat
>   Ring starts:   beat − 20 frames  (expands, peaks ON beat)
>   Sparks start:  beat − 6 frames   (scatter, brightest at beat+2)
> Moment roll:   gated to downbeats only (4/4 beat 1)
> 4-bar phrase:  GlitchBloom forced (75% prob)
> 8-bar phrase:  Collapse or GlitchBloom forced (biggest moment)
> ```

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
  ├─ V5: Per-preset visual profiles (14-param VisualProfile per machine)
  ├─ V5: Field warp, DropPhase, intent rendering modes
  ├─ V6: Braille effects (spark burst, edge fringe, background grain)
  ├─ V6: Dust effects (vertical rain, beat ring pulse)
  ├─ V7: BPM braille wave (BUILD state only; beat sweep across empty cells)
  ├─ V7: Braille transition dissolve (dot-count fringe on image crossfade boundary)
  ├─ V7: Braille motion echo trails (ASCII ghost → braille fading dots)
  ├─ V8: State-driven ambient activation (grain/rain/wave/fringe exclusive per energy state)
  ├─ V8: Visual breathing (8-bar slow sine modulates ambient probability)
  ├─ V8: Moment suppression (GlitchBloom/Collapse/PhaseWave clear ambient layer)
  ├─ V8: Per-frame braille palette (coherent dot vocabulary across grain/sparks/ring)
  ├─ V9: Musical clock (beat_phase, bar_phase, is_downbeat, is_phrase_start_4/8)
  ├─ V9: Anticipation scheduler (ring −20 frames, sparks −6 frames, peak ON beat)
  ├─ V9: Downbeat moment gating (moments fire on beat 1 only; density compensated)
  ├─ V9: Phrase forced moments (4-bar=GlitchBloom 75%, 8-bar=Collapse or Bloom)
  ├─ V9: Ambient rhythm lock (beat_gate on grain, fringe_swell on downbeat, rain_bar_mod)
  └─ Writes FrameBuffer (54×42, RGBA8 + char_indices + theme colors + metadata)
         │
         ▼
Display View (ascii_image_display.rs)
  ├─ FiraCode Nerd Font + Noto Symbols 2 (braille fallback), femtovg Canvas
  ├─ Renders CHARSET[char_indices[cell]] as colored monospace glyph
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
- Zero per-frame allocation in hot path (all effects use stack-only stack vars)
- UI overlay never glitched, masked, or distorted

---

## Window & Grid

```
422 × 600 px
COLS = 54       (line 616, editor.rs)
ROWS = 42       (line 617, editor.rs)
Grid center: col 27, row 21
```

The entire VST window is the ASCII display — no header, no bottom controls. All interaction is through the in-grid UI overlay.

---

## CHARSET (390 chars)

```
0–83:    Standard ASCII, artwork-safe (exact match preserved)
84–86:   Additional ASCII from source images: " m 8
87–93:   Digit chars found in images: 2 3 4 5 6 7 9
94–115:  Block elements (▏▎▖▗▘▝▍▚▞▌▐▄▀░▒▓▙▛▜▟▇█)
116–133: Box drawing (─│┌┐└┘├┤┬┴┼═║╔╗╚╝╬ …)
134–389: Braille (U+2800–U+28FF, all 256 patterns in Unicode order)
```

`ASCII_CHARSET_LEN = 116` — random-effect upper boundary. Effects that should never emit
box-drawing or braille (bloom shape chars, dust glyphs) clamp to this. Box-drawing chars
(116–133) render as visible structural lines and look bad as glitch noise.

`BRAILLE_CHARSET_START = 134` — first braille index. Used by all braille-aware effect
branches; effects check `idx >= BRAILLE_CHARSET_START` to detect braille cells.

**Single-dot braille indices** (1 bit set — one isolated dot per cell):
```
135 = ⠁ (dot 1)   136 = ⠂ (dot 2)   138 = ⠄ (dot 3)   142 = ⠈ (dot 4)
150 = ⠐ (dot 5)   166 = ⠠ (dot 6)   198 = ⡀ (dot 7)   262 = ⢀ (dot 8)
```

**Two-dot braille indices** (2 bits set — used by beat ring pulse):
```
137 = ⠃   139 = ⠅   143 = ⠉   168 = ⠢   199 = ⡁   263 = ⢁
```

**Multi-dot braille indices** (3–6 dots — used by V7 energy-gated effects):
```
141 = ⠍ (3)  190 = ⠾ (4)  183 = ⠷ (4)  161 = ⠡ (3)
149 = ⠕ (3)  165 = ⠥ (3)  197 = ⡅ (4)  272 = ⢐ (3)
```

---

## Image Source

All images loaded from **`ascii.txt`** (root directory) using `#N` separator format. Currently **100 images**. Parsed at startup by `AsciiBank::from_ascii_txt()`. Native resolution — no resizing. `get_cell()` returns 0 for out-of-bounds. Oversized images viewport/pan across their full extent. Each image has computed `density` (non-space fraction) and `complexity` (edge transitions) used for biased selection.

---

## Image Cycling (BPM-Synced)

### Core Image
- **Changes every 2 bars**
- Random order (hash-based)
- **Scatter-dissolve transition** over half a bar with wave bias
- Can drift partially off-screen (**minimum 45% visible**)

### Overlay Slots (4 independent)

| Slot | Cycle Period | Character |
|------|-------------|-----------|
| 0 | **1.5 bars** | Fast, energetic — 30% alpha floor |
| 1 | **2.5 bars** | Medium |
| 2 | **3.0 bars** | Slow, atmospheric |
| 3 | **2.0 bars** | Mid-tempo — 30% alpha floor |

---

## Module 1 — Dust System

```
Priority: fills empty cells when final_density_idx == 0
Every frame, always animating
```

Dust density = `base_dust × 0.88 + energy × 0.17`, boosted +0.20 on transient. Driven by `dust_tick` (always advances, never pauses with audio transport).

### Style 0 — Random Scatter (default)

Wave-mix oscillates slowly between pure random and wave-structured patterns:

```
  ,  '  .    `  ,   '    .   `  ,
.    `    ,  .      .  '     .
   '   .      '  ,   .   `     .
```

### Style 1 — Grid-Aligned (SP-1200)

XOR of quantized cell positions creates a structured digital stepping pattern:

```
.   .   .   .   .   .   .   .   .
  '   '   '   '   '   '   '   '
.   .   .   .   .   .   .   .   .
  '   '   '   '   '   '   '   '
.   .   .   .   .   .   .   .   .
```

### Style 2 — Chaotic Wave (Mirage)

Sinusoidal density bands sweep diagonally with heavy random weighting:

```
                          . , . , . , ,
              . , , . , .
  , , . , . .
, , , , . .
```

### Style 3 — Vertical Rain (NEW in V6, updated V8)

Active only in **FLOW state** (energy 0.22–0.55). Disappears in IDLE (too quiet) and BUILD (wave takes over). Suppressed during moments. Each of the 54 columns has an independent "drop" that descends continuously. Drop speed scales with energy — louder = faster falling. Two-row trail with brightness taper. Pattern pool includes vertical-pair patterns (⠃⠇) for an elongated drip character:

```
col:   0    5   10   15   20   25   30   35   40   45   50
       ⠁              ⠁         ⠂         ⠁         ⠄
       ⠂                        ⠁                   ⠂
            ⠄    ⠂              ⠄              ⠂
            ⠂    ⠁                             ⠁
       ⠁         ⠄              ⠁         ⠄
```
*Each column's drop position advances independently, creating a non-uniform matrix of dots.*

Animation (frames left→right at medium energy):
```
frame 0: ⠁ at row 3     frame 1: ⠁ at row 4*     frame 2: ⠁ at row 5
         ⠂ at row 11              ⠂ at row 12               ⠂ at row 13
         ⠄ at row 27              ⠄ at row 28               ⠄ at row 29
```
*\*drop_speed = 3 at medium energy → 1 row step every 3 frames*

### Style 4 — Beat Ring Pulse (NEW in V6)

On each loud transient (energy > 0.15), a ring of 2-dot braille patterns expands from grid center (col 27, row 21) outward over 20 frames, then vanishes:

```
frame 0 (impact):        frame 5 (ring r≈8):       frame 14 (ring r≈22):
        ·                    ⠃⠅⠉⠃                  ⠃⠅⠉⠃⠅⠃
        ·                ⠅         ⠅            ⠅            ⠅
        ·              ⠉             ⠉         ⠃              ⠃
  ·····+·····          ⠅             ⠅         ⠅              ⠅
        ·              ⠃             ⠃         ⠃              ⠃
        ·                ⠅         ⠅            ⠅            ⠅
        ·                    ⠃⠅⠉⠃                  ⠃⠃⠅⠉⠃⠃
```
*Ring expands at radius = ring_age/20 × 32 cells. Alpha = 1.0 − ring_age/20 (fades as it grows).*

---

## Module 2 — GlitchBloom (V3 Moment)

```
Trigger: transient + energy > 0.6
Duration: 15–25 frames, then → Afterglow
```

Expanding bloom radius from a seed cell. For ASCII/block cells: substitutes block/box drawing chars in `palette.emphasis` color. For braille cells (≥ 134): XORs the dot-bit pattern with the bloom seed — scrambles the braille pattern reactively while color tint still applies.

### Bloom shape 0 — Rectangle

```
frame 0:           frame 4:              frame 8:
   ▄               ▄▄▄▄▄▄              ▄▄▄▄▄▄▄▄▄
                   ▄▄▄▄▄▄              ▄▄▄▄▄▄▄▄▄
                   ▄▄▄▄▄▄              ▄▄▄▄▄▄▄▄▄
```

### Bloom shape 1 — Scanline (SP-1200)

Horizontal bands at ±1 row, extended 2× horizontally:

```
frame 0:           frame 5 (r=5):           frame 10 (r=10):
                   ▄▄▄▄▄▄▄▄▄▄▄▄▄▄ ← top band
   ▄               ███████████████ ← bloom row
                   ▄▄▄▄▄▄▄▄▄▄▄▄▄▄ ← bot band
```

### Bloom shape 2 — Radial (S950, P-2000)

Circular expansion using euclidean distance:

```
frame 0:    frame 5 (r=5):      frame 10 (r=10):
   ▄           ▄▄▄▄▄            ▄▄▄▄▄▄▄▄▄
             ▄▄▄▄▄▄▄▄▄         ▄▄▄▄▄▄▄▄▄▄▄▄▄
           ▄▄▄▄▄▄▄▄▄▄▄▄▄      ▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄
             ▄▄▄▄▄▄▄▄▄         ▄▄▄▄▄▄▄▄▄▄▄▄▄
               ▄▄▄▄▄            ▄▄▄▄▄▄▄▄▄
```

### Bloom shape 3 — Jagged (Mirage)

Irregular per-row radius variation using cell hash:

```
frame 8:
  ▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄
    ▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄
  ▄▄▄▄▄▄▄▄▄▄▄▄▄▄
      ▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄
  ▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄
    ▄▄▄▄▄▄▄▄▄▄▄▄▄
```

---

## Module 3 — Coherent Glitch Field (V4)

```
Trigger: bit depth < 12
3-octave FBM hash: octave 1 (scale 0.15, w 0.5) + octave 2 (0.3, 0.3) + octave 3 (0.6, 0.2)
```

For braille cells (≥ 134): XORs dot bits with a corruption seed. Flip mask scales with tier:
tier 1 = low 4 bits (subtle), tier 2 = low 6 bits (moderate), tier 3 = all 8 bits (heavy).
Emphasis color tint applies identically regardless of ASCII or braille.

### Glitch style 0 — Mixed

Full CHARSET range, emphasizing heavy density chars:

```
before:       after (bit depth 8):
  ###      →    ▓█╬▓█▓
  ###           ╬▓██▓█
  ###           ▓█╬▓█▓
```

### Glitch style 1 — Horizontal Scanline (SP-1200, SP-303)

Only block elements (indices 94–105). Creates CRT scanline aesthetic:

```
before:       after:
  ###      →    ▄▄▀▀▄▄▄
  ###           ▀▀▄▄▀▀▀
  ###           ▄▄▀▀▄▄▄
```

### Glitch style 2 — Warped Melt (Mirage)

Wide range including digits + blocks. Melted digital texture:

```
before:       after:
  ###      →    7▓3█2▄
  ###           ▓6▄2▓4
  ###           3▄█▓5▄
```

### Glitch style 3 — Minimal (S950, MPC3000)

Light chars + thin blocks only (indices 1–40). Subtle, faint artifact:

```
before:       after:
  ###      →    .i-.'
  ###           ;,-.'
  ###           '-i.;
```

---

## Module 4 — Collapse (V3 Moment)

```
Trigger: exiting PEAK state
Duration: 25 frames
```

Coherent noise progressively removes cells. For ASCII cells: zeroes the index. For braille cells: ANDs the dot-bit pattern with a noise-derived mask — thins the dot count gradually before full disappearance. Image dissolves from noisy regions inward:

```
frame 0:      frame 8:       frame 16:      frame 24:
####          #  ##           ## #
####         ##               #
####          ##  #
```
*Cells are not randomly scattered — collapse follows the coherent noise field, producing cluster-shaped erosion.*

---

## Module 5 — Braille Spark Burst (NEW in V6, updated V7)

```
Trigger: transient + energy > 0.15
Lifetime: 8 frames (spark_frames countdown)
Only fires on empty cells (final_density_idx == 0)
```

Scattered braille chars appear across empty cells at the moment of a hit. Each spark is independently hashed — position, dot orientation, and color all vary per-cell. Alpha fades linearly with spark age × energy. In V7, dot density is energy-gated: at low energy → single-dot patterns; at high energy → 3–5 dot patterns (SPARK_MULTI pool), making hard transients visually heavier.

**Frame 0 (impact):**
```
  ⠁  ⠂   ⠄    ⠁  ⠂   ⠄  ⠁    ⠂  ⠁   ⠂
⠄   ⠁  ⠂   ⠁     ⠂  ⠁   ⠄  ⠂    ⠁
  ⠂  ⠄   ⠁  ⠂    ⠄  ⠁   ⠂  ⠁   ⠄  ⠂
⠁   ⠂  ⠁   ⠄    ⠂   ⠁  ⠄    ⠂  ⠁
   ⠄  ⠁  ⠂   ⠁    ⠄  ⠂  ⠁    ⠄  ⠂   ⠁
```

**Frame 4 (mid-fade, sparser):**
```
       ⠂              ⠁         ⠄
  ⠁              ⠄          ⠂        ⠁
            ⠂         ⠁         ⠄
       ⠄         ⠂         ⠁
```

**Frame 7 (nearly gone):**
```
                  ⠂
       ⠁                        ⠄
```

Color alternates per-cell between `palette.chart[0]` (most vibrant accent) and `palette.emphasis`.
At bit-depth 8 on Dracula theme this looks like scattered purple/pink sparks on every kick drum hit.

---

## Module 6 — Braille Edge Fringe (NEW in V6, updated V8)

```
Active: IDLE and FLOW states only (visual_state ≤ 1)
Suppressed during GlitchBloom / Collapse / PhaseWave
Only fires on empty cells adjacent to ASCII art content
Probability: (neighbor_count × 0.22) × (0.4 + energy × 0.6)
Ticks every 8 frames (slow, subtle)
```

Empty cells directly bordering ASCII art content get 2–3 dot braille patterns, creating a soft halftone penumbra. More dots on the side facing more content (neighbor count drives probability).

**Before (art edge, no fringe):**
```
    ........                    (ASCII art content)
    ..######
    .####
    ########
    ########
```

**After (edge fringe active):**
```
   ⠅⠃⠉........                 (braille dots bleeding off art edge)
⠃⠃ ..######⠅
⠅  .####⠃⠅
   ########⠃
   ########
```
*Fringe uses `palette.primary` at alpha 0.06–0.14. The effect is intentionally subtle — a soft antialiasing halo, not a visible border. Most noticeable in still frames on dark themes with high-contrast art.*

---

## Module 7 — Braille Background Grain (NEW in V6, updated V8)

```
Active: IDLE state only (visual_state == 0, energy < 0.22)
Suppressed during GlitchBloom / Collapse / PhaseWave
~1.8–4.0% of empty cells; probability modulated by 8-bar breath_mod (0.70–1.00)
Pattern: draws from per-frame braille palette (F0/F1 families at IDLE)
Ticks every 4 frames (slow drift)
```

IDLE-state texture — the visual equivalent of silence. Each grain dot is stable for 4 frames before re-rolling. In V8 the grain uses the shared per-frame braille palette so it visually "matches" any sparks that fire simultaneously. Grain disappears completely when energy rises to FLOW/BUILD, creating genuine contrast when activity begins. The breath_mod sine makes it pulse subtly even in total silence.

**Example (background area, grain visible):**
```
                ⠁
                           ⠂                     ⠄
    ⠁                               ⠁
                    ⠂
                                            ⠂
         ⠄                    ⠁
                   ⠁
```
*At ~60fps this reads as barely-visible film grain. Grain color is `palette.primary` at alpha 0.08–0.14 — on Noni (olive theme, dark) the dots are near-invisible olive specks.*

---

## Module 8 — FreezeCut (V3 Moment)

```
Trigger: transient + energy > 0.8
Duration: 5–20 frames → Afterglow
```

Velocity and position freeze. Brightness flash (+10%). Dust continues animating under the freeze. Creates a stutter/latch feel on very loud hits.

**Animation (timeline):**
```
frame -1: motion → frame 0: FREEZE (+brightness) → frame +12: UNFREEZE → Afterglow
           moving art          same position, brighter
```

---

## Module 9 — Shimmer (Per-cell quantized flicker)

```
Trigger: is_base cell + should_update frame + 0.2% roll per frame
```

Rare single-char flicker where an art character shifts ±1 density index. For braille cells: flips one random dot bit (XOR with a single-bit mask) instead of ±1 index shift. Each cell flickers at most once per quantization window. Braille follows the same `should_update` gate as ASCII — cells go dark on held frames and only shimmer on update frames.

```
normal:    S  ---→  momentary:   T   (one step denser, same position)
frame 0:  ####      frame 1:   ####  (one T appears briefly among #s)
           S  S                  T  S
```

---

## Module 10 — BPM Braille Wave (NEW in V7, updated V8)

```
Active: BUILD state only (visual_state == 2, energy 0.55–0.82)
Suppressed during GlitchBloom / Collapse / PhaseWave
Fires once per beat (ticks_per_bar / 4), left-to-right sweep
Wave width: 5 cells; dot density escalates with energy
```

A wave of braille patterns sweeps left-to-right in sync with the beat. The leading edge blazes brightest; cells behind it fade. Dot density is energy-tiered:

| Energy | Pattern tier | Dots |
|--------|-------------|------|
| 0.22–0.40 | 1-dot sparse | ⠁⠂⠄⠈ |
| 0.40–0.60 | 2–3 dot medium | ⠃⠅⠍⠾ |
| 0.60–0.80 | 4-dot full | ⠕⠡⠷⠥ |
| 0.80+ | 5–8 dot dense | ⡅⢐⡷⢿ |

Color: `palette.chart[0]` at alpha proportional to `wave_intensity × 0.55`.

```
beat 0 (wave at col 0):              beat ~1/4 (wave at col 13):
⠁  ⠂  ⠄  ⠁  ⠂                          ⠁  ⠂  ⠄  ⠁  ⠂
⠂     ⠁     ⠄                              ⠂     ⠁     ⠄
⠄  ⠁     ⠂  ⠁                                ⠄  ⠁     ⠂
```

---

## Module 11 — Braille Color Accent (NEW in V7)

```
Always-on, applied to every braille cell after all dot-content effects
Mix: 38% blend toward palette.chart[0]
```

All braille cells (char_indices ≥ 134) receive a 38% tint toward the theme's most vibrant accent color (`palette.chart[0]`). This creates visible contrast between braille dot patterns (accent-tinted) and ASCII block art (primary-colored), making the two visual layers visually distinct.

On Dracula theme: braille dots are warm purple-pink against cold gray ASCII art.
On Rooney theme: braille dots shimmer orange-yellow against white structure.

---

## Module 12 — Braille Transition Dissolve (NEW in V7)

```
Active: during image transitions (in_transition = true)
Alternates per cycle: even cycles = original binary dither, odd = braille dissolve
```

On odd image cycles, the transition threshold zone becomes a curtain of braille dots instead of a hard cut. Cells near the dither boundary (within ±0.10 of `threshold`) render as braille with dot count proportional to their position in the zone:

```
zone position →  0.0    0.1    0.2    ...   0.8    0.9   1.0
dot count     →   ⣿      ⡿      ⠿    ...    ⠇      ⠃     ⠁
               (8 dots)                             (2)  (1)
```

Dot patterns are spatially rotated by a per-cell hash for organic, non-uniform feel. On even cycles the original instant-cut binary dither is used — both behaviors coexist across cycles.

---

## Module 13 — Braille Motion Echo Trails (NEW in V7)

```
Active: motion_echo > 0.01 + energy > 0.1
Writes to unwritten (background) cells only
Echo ages 1–3, alpha = 0.25 − age × 0.08
```

Ghost trails from historical image positions. When the source cell is an ASCII character, the echo behavior now alternates by echo age:

- **Odd echo ages** (1, 3): original behavior — sparse low-density ASCII char (`raw.min(20)`)
- **Even echo ages** (2): braille trail — dot count fades with age (6 dots at age 1→4→2 for older)

Braille source cells are always preserved exactly regardless of age.

```
echo age 1:  ⠿  (6 dots — freshest, brightest)
echo age 2:  ⠏  (4 dots — fading)
echo age 3:  ⠃  (2 dots — nearly gone)
```

---

## V9: Musical Clock

Derived each frame from `t` (anim_tick as f32) and `ticks_per_beat` / `ticks_per_bar`:

```rust
beat_phase        = (t % ticks_per_beat) / ticks_per_beat  // 0.0→1.0 within beat
bar_phase         = (t % ticks_per_bar)  / ticks_per_bar   // 0.0→1.0 within bar
beat_num          = (t / ticks_per_beat) as u64
beat_in_bar       = beat_num % 4                            // 0=downbeat, 1-3=upbeats
is_beat_start     = playing && beat_num changed this frame
is_downbeat       = is_beat_start && beat_in_bar == 0
is_phrase_start_4 = playing && (t / (ticks_per_bar × 4)) changed this frame
is_phrase_start_8 = playing && (t / (ticks_per_bar × 8)) changed this frame
```

All time references are in anim_tick frames. `ticks_per_beat = 3600 / effective_BPM`.

---

## V9: Anticipation Engine

Visuals fire *before* the beat so they peak *on* it — the opposite of reactive triggering.

```
Transient detected (energy > 0.15, ring not already queued):

  frames_to_beat = ceil(ticks_per_beat - (t % ticks_per_beat))

  scheduled_ring   = anim_tick + (frames_to_beat − 20).max(2)
  scheduled_sparks = anim_tick + (frames_to_beat − 6).max(2)

Each frame: if anim_tick >= scheduled_ring   → start ring_frames = 20, clear schedule
            if anim_tick >= scheduled_sparks → start spark_frames = 8, clear schedule
```

The ring pulse expands over exactly 20 frames and reaches peak radius at `beat_start + 0` — it arrives, doesn't react. Sparks scatter 6 frames before the beat and are brightest at beat+2 (two frames into the hit), matching human perception of drum attack envelope.

---

## V9: Downbeat Moment Gating

Moments now only trigger on beat 1 of each bar (downbeat). To preserve average density, `base_prob` is pre-multiplied by `ticks_per_beat` — the probability-per-frame stays the same, but all probability is concentrated into the downbeat frame.

```rust
// Before V9: every frame
if trigger_roll < base_prob { trigger_moment() }

// V9: downbeats only, same average density
base_prob *= ticks_per_beat;
if is_downbeat && trigger_roll < base_prob { trigger_moment() }
```

This means moments always land on "1" — the visual and musical emphasis coincide.

---

## V9: Phrase Forced Moments

At phrase boundaries, moments are forced regardless of current energy or cooldown:

| Boundary | Forced moment | Probability |
|----------|--------------|-------------|
| 4-bar start | GlitchBloom | 75% |
| 8-bar start | Collapse (if recent Bloom) or GlitchBloom | 100% |
| 8-bar start | 2-frame downbeat micro-freeze | always |

The 8-bar boundary fires the biggest available moment — it's the phrase apex. The 2-frame micro-freeze gives each 8-bar downbeat a sharp visual punctuation even when energy is too low for a full moment.

---

## V9: Ambient Rhythm Lock

Ambient effects now breathe with the beat and bar rather than purely following energy:

| Signal | Formula | Applied to |
|--------|---------|-----------|
| `beat_gate` | `1.0 − beat_phase × 0.28` (1.0 on downbeat, fades 28% by beat end) | Grain probability |
| `fringe_swell` | `1.45` on downbeat, `1.0 + 0.15 × (1 − beat_phase)` otherwise | Fringe probability |
| `rain_bar_mod` | `1.0 + 0.35 × (1 − bar_phase)` (peaks at bar start, decays across bar) | Rain drop speed |

Grain: pulses with each beat — the silence texture has rhythmic breath.  
Fringe: blooms most at bar 1, beat 1 — art edges flare on the downbeat.  
Rain: falls fastest at bar start, decelerates across the bar — each new bar is a fresh drop.

---

## V8 State Machine — Ambient Effect Activation

At any time, only one ambient layer is active based on `visual_state`. Strong moments additionally suppress all ambient effects:

| State | Energy range | Ambient active | Suppressed by moments |
|-------|-------------|---------------|----------------------|
| IDLE  | < 0.22 | grain + fringe | GlitchBloom, Collapse, PhaseWave |
| FLOW  | 0.22–0.55 | rain + fringe | GlitchBloom, Collapse, PhaseWave |
| BUILD | 0.55–0.82 | BPM wave | GlitchBloom, Collapse, PhaseWave |
| PEAK  | > 0.82 | none | — |

**Visual breathing:** `breath_mod = f(slow 8-bar sine)` scales ambient probability from 0.70–1.00. At silent stretches the system breathes deepest; under load it stays near full.

**Per-frame braille palette:** 6 dot families (F0=1-dot sparse → F5=8-dot dense), advancing every half-bar. Grain, sparks (tier-0), and ring (tier-0) all pick from the active family — at most 4 dot-types per frame.

---

## Full Compositing Order (Back → Front, per cell each frame)

```
1.  Background fill      — palette.background (exact sRGB, no cell logic)
2.  Overlay images (x4)  — filter + structural alpha, scatter-dissolve transitions
3.  Core image           — velocity scroll, min 30% visible, wave-biased/braille dissolve (V7)
4.  Shimmer              — rare ±1 index flicker on art cells only
5.  Dust glyph           — dust_present < dust_density → ASCII chars 1–6 (.,`',;)
6.  Spark burst          — braille dots, 8-frame transient decay; tier-0 uses frame palette (V8)
7.  Beat ring pulse      — braille dots, expanding ring; tier-0 uses frame palette (V8)
8.  BPM braille wave     — BUILD state only; beat-synced sweep, dot density scales with energy (V8)
9.  Edge fringe          — IDLE/FLOW only; braille halos at art borders (V8)
10. Background grain     — IDLE only; breathes with 8-bar sine; uses frame palette (V8)
11. Vertical rain        — FLOW only; braille falling-column dots (V8)
12. Collapse             — cell zeroing (moment, overrides all below)
13. Coherent glitch      — FBM-noise corruption (bit depth < 12)
14. GlitchBloom          — expanding block/box chars (moment)
15. Jitter               — temporal cell dropout / braille dot halving
16. Brightness boost     — moment + phrase brightness modulation
17. Afterglow tint       — emphasis color drift during Afterglow moment
18. Transient flash      — 10% emphasis tint on transient hit
19. Light mode inversion — brightness becomes darkening on light themes
20. Restraint            — idle/recovery dampening (35%/50%) applied last
21. Motion echo pass     — ghost trails; ASCII sources smear into braille (even ages) (V7)
22. UI overlay           — femtovg text: title + param menu (never in framebuffer)
```

*Layers 1–11 gate on `final_density_idx == 0` — effects earlier in the list take precedence.*
*Layers 8–11 are each exclusively active in one energy state (V8 state machine).*
*Layer 21 (motion echo) only writes cells with alpha == 0 (unwritten background cells).*

---

## V3: Moment System

One moment active at a time. Each has duration + cooldown.

| Moment | Trigger | Effect | Duration |
|--------|---------|--------|----------|
| **FreezeCut** | Transient + energy > 0.8 | Freeze velocity/motion, +10% brightness | 5–20 frames |
| **GlitchBloom** | Transient + energy > 0.6 | Expanding glitch radius (block/box chars) | 15–25 frames |
| **LockIn** | Entering PEAK state | Overlays use same image as core | 2 beats |
| **PhaseWave** | Energy > 0.7 (rare) | Horizontal sine displacement on core | 20–35 frames |
| **Collapse** | Exiting PEAK state | Coherent noise progressively removes cells | 25 frames |
| **Afterglow** | Auto after FreezeCut/GlitchBloom | Increased smearing + trail persistence | 20 frames |
| **UserAccent** | Rapid param change | Brightness boost | 10 frames |

### Micro-freezes
3–8 frame micro-version of FreezeCut. Triggered by transients when no moment is active. Creates rhythmic punctuation without a full moment.

---

## V3: Memory System

```rust
heat      = lerp(heat, smoothed_energy, 0.05)     // drives glitch scaling, overlay aggression
fatigue  += glitch_events * 0.01; fatigue *= 0.98 // reduces glitch after heavy activity
afterimage = lerp(afterimage, energy, 0.1)         // drives smearing + trail persistence
```

---

## V3: Restraint System

- **Idle**: energy < 0.25 + no active moment → 35% intensity
- **Recovery**: after moment ends (cooldown > 15 frames) → 50% intensity
- **Fatigue**: reduces glitch probability 0.2–1.0× after sustained heavy activity

---

## V4: Phrase System (8-bar arcs)

```
phrase_arc          = sin(t / (ticks_per_bar × 8) × π)   // 0→1→0 over 8 bars
phrase_overlay_mod  = 0.7 + phrase_arc × 0.3             // overlay alpha scaling
phrase_brightness_mod = 0.9 + phrase_arc × 0.1           // brightness pulsing
```

---

## V4: Intent Model

Three accumulating intent signals:

```
intent_tension  += when energy rising steadily       (×0.95 decay)
intent_release  += when energy dropping              (×0.95 decay)
intent_chaos    += when energy erratic (high delta)  (×0.95 decay)
```

Dominant intent biases moment selection:
- **Tension** → FreezeCut, LockIn
- **Release** → Afterglow, Collapse
- **Chaos** → GlitchBloom, PhaseWave

---

## V4: Anchor-Based Composition

- 10 predefined anchor points across the grid
- Overlays drift toward their anchor at slot-specific pull rates (0.02–0.08)
- Soft collision avoidance pushes overlapping overlays apart
- Accent overlay retargets on transients

---

## V5: Per-Preset Visual Profiles

14-parameter `VisualProfile` per machine, interpolating smoothly (~300ms) on preset switch:

| Parameter | Controls | Range |
|-----------|----------|-------|
| `row_damping` | Core vertical velocity drag | 0.85 → 0.95 |
| `col_damping` | Core horizontal velocity drag | 0.82 → 0.94 |
| `bpm_force` | BPM rhythmic push amplitude | 0.20 → 0.45 |
| `dust_density` | Base dust particle density | 0.46 → 0.74 |
| `glitch_mult` | Glitch probability multiplier | 0.15 → 1.65 |
| `step_quant_mult` | SR temporal stepping scale | 0.7 → 1.5 |
| `smear_base` | Smear/trail base amount | 0.2 → 0.4 |
| `transition_speed` | Image transition window (bars) | 0.3 → 0.7 |
| `overlay_speed` | Overlay cycling speed mult | 0.8 → 1.4 |
| `micro_freeze_thresh` | Micro-freeze probability | 5 → 20 |
| `moment_mult` | Moment trigger probability mult | 0.6 → 1.4 |
| `sig_param` | Signature effect intensity | 0.0 → 0.8 |
| `dust_style` | 0=random, 1=grid, 2=chaotic | Per-preset |
| `glitch_style` | 0=mixed, 1=scanline, 2=warped, 3=minimal | Per-preset |
| `bloom_shape` | 0=rect, 1=scanline, 2=radial, 3=jagged | Per-preset |

---

## V5: Field Warp

`warp_phase` advances 0.007/frame. Applies coordinate displacement to overlay image lookups, creating a flowing spatial distortion that breathes with `intent_chaos`.

---

## V5: DropPhase Suppress

Brief visual suppression (1–4 frames) on detect of audio dropout or abrupt silence. Prevents the visual system from over-reacting to sudden silence as if it were a transient event.

---

## Timing & Pacing

### Two Clocks

| Clock | Drives | When Audio Stops |
|-------|--------|-----------------|
| `anim_tick` | Image cycling, scroll, overlay fade, transitions | **Freezes** |
| `dust_tick` (`frame_update_counter`) | Dust, grain, rain, ring | **Always advances** |

### BPM Source
```
Effective BPM = host BPM (if ≤115) or host BPM / 2 (>115 = half-time)
ticks_per_beat = 3600 / BPM,  ticks_per_bar = beat × 4
```

---

## Sample Rate → Temporal Quantization

```
sr_norm        = target_sr / 96000
step_interval  = lerp(1, 8, 1 − sr_norm)  frames between updates
smear_factor   = (1 − sr_norm) × 0.3
effective_smear = smear_factor + afterimage × 0.15  (capped 0.8)
```

Low SR (4kHz) = step_interval ≈ 8 (jumpy, stuttered) + heavy smearing/ghosting.

---

## Filter → Structural Visibility

Per-cell coherent noise vs. filter threshold, applied identically to core and overlay:

```
coherent_noise = center × 0.6 + avg(4 neighbors) × 0.4
coherent_noise > filter_val → alpha 0.15 (dark) / 0.35 (light)  [faded]
coherent_noise ≤ filter_val → alpha 1.0  [full]
```

---

## DSP Parameter → Visual Mapping

| Parameter | Range | Visual Effect |
|-----------|-------|---------------|
| **Bandwidth** | 1k–48k Hz | Temporal quantization: low = stepped motion + ghosting |
| **Filter** | 0–1 | Structural visibility (coherent masking on core + overlays) |
| **Mix** | 0–1 | Overlay density (2%→100%) + speed + max 80% alpha |
| **Bit Depth** | 1–24 | Tiered corruption: 16-12=clean, 11-9=point, 8-6=cluster, 5-4=structural |
| **Jitter** | 0–1 | Temporal flicker: randomly zeroes cells. Braille: coin-flip between halving dot count or full dropout |
| **BPM** | host | All timing: cycling, update cap, ring pulse, rain speed |
| **Playing** | host | Images freeze. Dust + grain + rain + ring keep moving. |

---

## Light Mode Rendering

`palette.is_light` computed from background luminance (> 0.18 linear):

| Effect | Dark Theme | Light Theme |
|--------|-----------|-------------|
| Brightness boost | Additive (+) | Subtractive (−) — darkens for emphasis |
| Structural floor | 0.15 | 0.35 |
| Dust opacity | base | ×1.6 |
| Glitch alpha | 0.15 + energy×0.2 | 0.30 + energy×0.35 |
| Edge fringe | same | same (low-alpha always) |
| Spark burst | same | same (energy-scaled) |

---

## Machine Presets (7)

| Name | SR | Bits | Poles | Dust | Glitch | Visual Character |
|------|----|------|-------|------|--------|-----------------|
| **SP-1200** | 26,040 Hz | 12 | 2 | Grid | Scanline | Punchy, structured steps |
| **MPC60** | 40,000 Hz | 12 | 4 | Random | Mixed | Clean, energetic |
| **S950** | 48,000 Hz | 12 | 6 | Random | Minimal | Smooth, rare radial blooms |
| **Mirage** | 33,000 Hz | 8 | 4 | Chaotic | Warped | Gritty melt, heavy drift |
| **P-2000** | 41,667 Hz | 12 | 4 | Random | Minimal | Analog drift, radial blooms |
| **MPC3000** | 44,100 Hz | 16 | 4 | Random | Minimal | Clean, transient flashes |
| **SP-303** | 44,100 Hz | 16 | 4 | Random | Scanline | Clean + digital scanlines |

---

## Themes (14 × light/dark)

| # | Name | Primary | Accent | Spark/Ring Color |
|---|------|---------|--------|-----------------|
| 0 | Pink | 340 (pink) | 355 (rose) | rose sparks |
| 1 | Kerama | 250 (cobalt) | 195 (teal) | teal ring pulses |
| 2 | Brazil | 145 (green) | 95 (yellow) | yellow sparks |
| 3 | **Noni** | 118 (olive) | 122 (lime) | lime sparks |
| 4 | Paris | 328 (fuchsia) | 72 (gold) | gold ring |
| 5 | Rooney | 22 (red) | 78 (gold) | gold sparks |
| 6 | k+k | 260 (gray) | 260 (gray) | gray sparks |
| 7 | Catppuccin | 300 (mauve) | 265 (lavender) | lavender |
| 8 | Kanagawa | 222 (wave blue) | 80 (amber) | amber ring |
| 9 | Rosé Pine | 0 (rose) | 300 (iris) | iris sparks |
| 10 | Dracula | 295 (purple) | 340 (pink) | pink sparks |
| 11 | Papaya | 50 (orange) | 45 (orange) | orange ring |
| 12 | Dominican | 264 (royal blue) | 22 (red) | red sparks |
| 13 | Calsonic | 260 (ocean blue) | 18 (coral) | coral ring |

---

## Per-Frame State (EditorData) — V9

```rust
// Animation
smoothed_energy: f32
velocity_row: f32, velocity_col: f32
anim_tick: usize, quant_frame: u64
prev_row_scroll: f32, prev_col_drift: f32
prev_overlay_rows: [f32; 4], prev_overlay_cols: [f32; 4]

// Moments & Memory
moment: MomentState { active, timer, duration, cooldown, seed, bloom_center }
memory: MemoryState { heat, fatigue, afterimage }
micro_freeze_frames: u32
prev_energy_state: u8
prev_filter: f32, prev_sr: f32
glitch_events_this_frame: u32

// Phrase, Intent, Composition
phrase_tick: f32
intent_tension: f32, intent_release: f32, intent_chaos: f32
core_pos: (f32, f32), core_anchor: (f32, f32)
overlay_anchors: [(f32, f32); 4], overlay_positions: [(f32, f32); 4]
accent_slot_alpha: f32
glitch_field_phase: f32, recent_moment_count: u32

// V5: Field warp + DropPhase + intent rendering
drop_phase_timer: u32, drop_reentry_timer: u32
warp_phase: f32
intent_mode: u8, intent_mode_t: f32, intent_mode_bars: f32

// V6: Braille effects
spark_frames: u32    // 8-frame countdown, set on transient — drives spark burst
ring_frames: u32     // 20-frame countdown, set on transient — drives beat ring pulse

// V9: Anticipation scheduler
scheduled_sparks: Option<u64>         // anim_tick to start spark burst (pre-beat)
scheduled_ring: Option<u64>           // anim_tick to start ring pulse (pre-beat)
pending_moment: Option<(u64, Moment)> // phrase-forced moment, fires at scheduled tick
```

---

## Key Files

| File | Purpose |
|------|---------|
| `ascii.txt` | 100 ASCII art images (`#N` separated) |
| `src/editor.rs` | Full animation engine — all 9 modules |
| `src/ascii_image_display.rs` | femtovg rendering, UI overlay, mouse interaction |
| `src/ascii_bank.rs` | CHARSET (380 chars incl. braille), image parsing |
| `src/audio_feed.rs` | `AnimationParams` (energy, transient, BPM, playing) |
| `src/render/color_system.rs` | `ColorPalette` (14 themes × light/dark) |
| `src/render/offscreen.rs` | `FrameBuffer` (pixels + char_indices + theme metadata) |
| `src/render/audio_analysis.rs` | `AudioAnalyzer` (RMS, transient detection) |
| `src/lib.rs` | DSP: bandwidth/bit-crush/filter, machine-specific saturation |
