# ASCII Animation System V5 — Design Spec

**Date:** 2026-03-23
**Status:** Approved
**Scope:** `editor.rs` only (all other files untouched)

---

## Goal

Upgrade the V4 ASCII animation engine into a more expressive, instrument-like visual engine by improving contrast, timing, and identity — without increasing overall system complexity or violating existing constraints.

**Non-negotiables:**
- Deterministic behavior (hash-based noise only, no `rand`)
- No per-frame allocations
- ASCII integrity (no character substitution)
- UI overlay untouched and unglitched
- 60fps performance target

---

## Approach

**Inline patches + file-scoped helpers (Approach 2)**

All 9 features implemented directly in `editor.rs`. Three small non-allocating helper `fn`s extracted for clarity (`warp_offset`, `edge_brightness_delta`, `signature_tick`). No new modules, no new abstractions.

Per-preset signature behaviors use **Option A**: one `sig_param: f32` field added to `VisualProfile`; behavior dispatched by `preset_idx` in `signature_tick`.

---

## Section 1: State additions

### `VisualProfile` — 1 new field

```rust
sig_param: f32,   // signature effect intensity (0.0 = off)
```

Per-preset values:

| Preset | `sig_param` | Behavior |
|---|---|---|
| SP-1200 | 0.7 | Horizontal tearing bands |
| MPC60 | 0.5 | Quantized grid snap |
| S950 | 0.4 | Rare symmetric bloom |
| Mirage | 0.8 | Vertical melt shimmer |
| P-2000 | 0.5 | Analog wave drift |
| MPC3000 | 0.6 | Sharp transient flash |
| SP-303 | 0.7 | Digital block flicker |

`sig_param` joins the existing `vr = 0.04` interpolation block for smooth preset transitions.

### `EditorData` — 7 new fields

| Field | Type | Initial | Purpose |
|---|---|---|---|
| `drop_phase_timer` | `u32` | 0 | Visual suppression countdown (1–4 frames) |
| `drop_reentry_timer` | `u32` | 0 | Post-drop glitch spike countdown (0–10 frames) |
| `warp_phase` | `f32` | 0.0 | Drives field warp; advances 0.007/frame |
| `intent_mode` | `u8` | 0 | 0=none, 1=Tension, 2=Chaos, 3=Release |
| `intent_mode_t` | `f32` | 0.0 | Smooth 0→1 interpolation for active mode |
| `intent_mode_bars` | `f32` | 0.0 | Bars elapsed under dominant intent |
| `sig_event_timer` | `u32` | 0 | S950 sym-bloom / MPC3000 flash refractory |

---

## Section 2: Three file-scoped helper functions

Added near `glitch_field` and `select_biased_image` at the top of `editor.rs`.

### `warp_offset`

```rust
fn warp_offset(col: u32, row: u32, phase: f32, chaos: f32, energy: f32) -> (i32, i32)
```

Computes coordinate-space displacement for field warping.

- `warp_x = sin(row × 0.2 + phase) × intensity`
- `warp_y` = two-octave hash FBM: `coarse(col/8, row/8) × 0.6 + fine(col/4, row/4) × 0.4`
- `intensity = (energy × 0.12 + chaos × 0.08).clamp(0.0, 0.20)`
- Output clamped to `(-2, 2)` per axis — grid bounds never broken

### `edge_brightness_delta`

```rust
fn edge_brightness_delta(bank: &AsciiBank, img_idx: usize, col: usize, row: usize) -> f32
```

Checks 4 cardinal neighbors in the source image:
- Returns `0.0` for background cells
- Fewer than 3 neighbors have content → edge cell → `+0.07`
- All 4 neighbors have content → interior → `-0.04`

Applied as linear additive after compositing, before gamma encoding.

### `signature_tick`

```rust
fn signature_tick(
    preset_idx: usize, col: u32, row: u32, energy: f32, sig_param: f32,
    dust_tick: u32, warp_phase: f32, transient: bool, anim_tick: u64,
) -> (f32, f32, f32)
```

Returns `(dr, dg, db)` linear delta added to cell color after compositing:

| Preset | Behavior detail |
|---|---|
| SP-1200 | Row-grouped hash; rare band shifts color ±RGB (max `sig_param × 0.08 × energy`) |
| MPC60 | Returns `(0,0,0)` — snap handled in velocity block |
| S950 | ~1 event per 300 ticks; radial flash near center when `bloom_roll < sig_param × 0.02` |
| Mirage | Column-phase sine driving blue-tinted shimmer (`sig_param × 0.06 × energy`) |
| P-2000 | Low-freq warp-phase wave; subtle color oscillation (`sig_param × 0.04 × energy`) |
| MPC3000 | White-ish spike only when `transient == true` (`sig_param × 0.15 × energy`) |
| SP-303 | 6×4-cell blocks flicker on transient (`sig_param × energy × 0.12`) |

All magnitude ranges: `0.03–0.15` linear, always energy-gated.

---

## Section 3: Feature implementations

### 1. DropPhase System

**Location:** moment-trigger block, replacing unconditional `Moment::Collapse` on drop entry.

**Trigger:** `intent_tension > 0.65` AND (`energy < 0.25` OR transient spike with descending energy).

**Suppression** (while `drop_phase_timer > 0`, 3–4 frames):
- `overlay_alpha_mult = 0.05` (applied to all slot alphas)
- `glitch_prob = 0.0`
- `dust_density *= 0.20`
- `effective_smear = 0.0`

**Re-entry** (when timer expires AND energy recovering):
- Set `drop_reentry_timer = 10`
- During re-entry: `glitch_prob *= (1.8 + intent_chaos × 0.4)` (up to ×2.2)
- Force `Moment::GlitchBloom`

Extends existing `drop_detected` / `drop_timer` system; does not duplicate it.

---

### 2. Hero Lock

**Location:** physics block, before the velocity update.

**Condition:** `visual_state == 3` (PEAK) OR `lockin_active`.

- Core pull: `CORE_PULL × 5.0` → effective `0.15`
- Extra velocity damping: `velocity_row × 0.85`, `velocity_col × 0.85` after normal damping
- Overlay anchors: snap to 4 rotationally-symmetric positions around `core_anchor` at radius 8 (using `sin`/`cos` of `0, π/2, π, 3π/2`)
- `glitch_prob *= 0.90`

---

### 3. Global Field Warping

**Location:** per-cell loop, before `base_col` / `src_row` computation.

```rust
let (wx, wy) = warp_offset(col, row, self.warp_phase, self.intent_chaos, energy);
// wx/wy added to base_col and src_row_signed respectively
```

Chaos Mode (item 4) adds `0.08` to warp intensity. `warp_phase += 0.007` after the frame loop.

---

### 4. Intent → Rendering Modes

**Location:** after intent model update, before per-cell loop.

**Mode detection:**
- `dominant = argmax(intent_tension, intent_chaos, intent_release)`
- If `dominant > 0.50` and mode sustained: `intent_mode_bars += bars_this_frame`
- After 2 bars: `intent_mode_t` ramps to `1.0` at rate `0.08/frame`
- On mode change or drop below `0.30`: `intent_mode_t` ramps to `0.0` at rate `0.12/frame`, reset `intent_mode_bars`

**Mode effects** (all scaled by `intent_mode_t`):

| Parameter | Tension | Chaos | Release |
|---|---|---|---|
| `row_damping` | `× (1 + 0.04t)` | `× (1 - 0.06t)` | — |
| `glitch_prob` | `× (1 - 0.15t)` | `× (1 + 0.20t)` | — |
| warp intensity | — | `+ 0.08t` | — |
| `effective_smear` | — | — | `+ 0.10t` |
| overlay alpha | — | — | `× (1 - 0.08t)` |
| anchor pull | `× (1 + 0.3t)` | `× (1 - 0.2t)` | — |

---

### 5. Edge-Aware Brightness

**Location:** per-cell loop, after compositing, before brightness boost block.

```rust
if is_base {
    let delta = edge_brightness_delta(&self.ascii_bank, core_img, bank_col_base, src_row);
    // dark themes: add delta; light themes: subtract delta
    r = (r ± delta).clamp(0.0, 1.0);
    g = (g ± delta).clamp(0.0, 1.0);
    b = (b ± delta).clamp(0.0, 1.0);
}
```

Applied only to base-image cells. Does not affect overlays or dust.

---

### 6. Temporal Echo (SR-Driven Rhythm)

**Location:** smear lerp block for `prev_row_scroll` / `prev_col_drift`.

Replaces the existing constant lerp rate when `sr_effect > 0.30`:

```rust
let smear_rate = if sr_effect > 0.30 {
    let hold = (self.quant_frame % step_interval) != 0;
    if hold { 0.02 } else { 1.0 - effective_smear }
} else {
    1.0 - effective_smear
};
```

Hold frames: position nearly frozen. Release frames: normal smear. At `step_interval = 8` (lowest SR), creates 8-tick lock-snap rhythm — the sampler's native grid. Blended with existing smear, not replacing it entirely.

---

### 7. Per-Preset Signature Behaviors

**MPC60 grid snap** — velocity block:
```rust
if self.preset_idx == 1 {
    self.velocity_row = (self.velocity_row * 2.0).round() / 2.0;
    self.velocity_col = (self.velocity_col * 2.0).round() / 2.0;
}
```

**All other presets** — per-cell loop, after compositing, before gamma:
```rust
let (sr, sg, sb) = signature_tick(self.preset_idx, col, row, energy,
    self.visual_profile.sig_param, dust_tick, self.warp_phase, transient, self.anim_tick);
r = (r + sr).clamp(0.0, 1.0);
g = (g + sg).clamp(0.0, 1.0);
b = (b + sb).clamp(0.0, 1.0);
```

---

### 8. Global Activity Reduction

Three one-line patches applied to existing computed values:

```rust
// After glitch_prob computation:
let glitch_prob = glitch_prob * 0.95;

// After dust_density computation:
let dust_density = (dust_density - 0.02).max(0.0);

// In slot alpha computation, multiply energy_alpha by 0.96
```

---

### 9. Subtle Per-Cell Flicker

**Location:** last step in per-cell loop, just before gamma encoding.

```rust
if energy > 0.05 {
    let fh = col.wrapping_mul(2246822507)
        .wrapping_add(row.wrapping_mul(1664525))
        .wrapping_add(self.anim_tick as u32 * 6364136);
    let flicker = ((fh >> 16) & 0xFF) as f32 / 255.0 - 0.5;
    let amt = flicker * energy * 0.016;  // ±0.008 linear (~±2 RGB/255)
    r = (r + amt).clamp(0.0, 1.0);
    g = (g + amt * 0.96).clamp(0.0, 1.0);
    b = (b + amt * 1.04).clamp(0.0, 1.0);
}
```

Slight R/B divergence simulates DAC instability. Imperceptible at rest, present at high energy.

---

## Section 4: Integration touchpoints

### Initialization
All 7 new `EditorData` fields initialize to `0` / `0.0`. `sig_param` set as a constant in each `VISUAL_PROFILES` entry.

### `warp_phase` clock
```rust
// End of UpdateFrameBuffer, alongside glitch_field_phase:
self.warp_phase += 0.007;
self.glitch_field_phase += 0.01;  // unchanged
```

### VisualProfile interpolation
`sig_param` joins the `vr = 0.04` lerp block for smooth transitions.

### Insertion order in per-cell loop

The per-cell loop processing order after this change:

1. Coordinate warp offset (new — item 3)
2. Base image sampling (unchanged)
3. Structural alpha / filter (unchanged)
4. Overlay compositing (unchanged)
5. Dust (unchanged)
6. Shimmer (unchanged)
7. GlitchBloom / Collapse (unchanged)
8. Glitch field (unchanged, but prob modified by items 2, 4, 8)
9. Dust glyph (unchanged)
10. Brightness boost + phrase mod (unchanged)
11. Afterglow tint (unchanged)
12. Transient flash (unchanged)
13. Idle/recovery dampening (unchanged)
14. Color temperature (unchanged)
15. Sub-bass breathing (unchanged)
16. Scanlines (unchanged)
17. Jitter flicker (unchanged)
18. **Edge-aware brightness** (new — item 5)
19. **Signature tick** (new — item 7)
20. **Per-cell flicker** (new — item 9)
21. Gamma encoding (unchanged)

### What doesn't change

- `AsciiImageDisplay` — untouched
- `audio_feed.rs`, `audio_analysis.rs` — untouched
- `color_system.rs`, `offscreen.rs`, `ascii_bank.rs` — untouched
- `FrameBuffer` struct — untouched
- All `Moment` variants, `MomentState`, `MemoryState` — untouched
- `ANCHORS`, `SLOT_MASS`, `SLOT_DRAG`, `SLOT_PULL`, `CORE_MASS` — untouched
- Grid bounds clamping — all warp offsets clamped before `get_cell`

### File change summary

| File | Change |
|---|---|
| `editor.rs` | 7 `EditorData` fields, 1 `VisualProfile` field + `sig_param` per preset, 3 helper fns, `warp_phase` clock, 9 feature insertions |
| All other files | No changes |
