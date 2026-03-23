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

Per-preset values (added to `VISUAL_PROFILES` constant entries):

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

### `EditorData` — 6 new fields

| Field | Type | Initial | Purpose |
|---|---|---|---|
| `drop_phase_timer` | `u32` | 0 | Visual suppression countdown (1–4 frames) |
| `drop_reentry_timer` | `u32` | 0 | Post-drop glitch spike countdown (0–10 frames) |
| `warp_phase` | `f32` | 0.0 | Drives field warp; advances 0.007/frame |
| `intent_mode` | `u8` | 0 | 0=none, 1=Tension, 2=Chaos, 3=Release |
| `intent_mode_t` | `f32` | 0.0 | Smooth 0→1 interpolation for active mode |
| `intent_mode_bars` | `f32` | 0.0 | Bars elapsed under dominant intent |

Note: `sig_event_timer` is not needed — S950 bloom uses `anim_tick / 120` as its natural period;
MPC3000 flash is already rate-limited by the `transient` flag from the audio system.

---

## Section 2: Three file-scoped helper functions

Added near `glitch_field` and `select_biased_image` at the top of `editor.rs`.

### `warp_offset`

```rust
fn warp_offset(col: u32, row: u32, phase: f32, chaos: f32, energy: f32) -> (i32, i32)
```

Returns coordinate-space displacement as `(i32, i32)`. Internal math is `f32`; results are
converted with `.round() as i32` then clamped with `.clamp(-2i32, 2i32)`.

- `warp_x = (row as f32 * 0.2 + phase).sin() * intensity`
- `warp_y` = two-octave hash FBM:
  - octave 1: `hash_noise((col/8) as f32, (row/8) as f32, 7331)` × 0.6
  - octave 2: `hash_noise((col/4) as f32, (row/4) as f32, 8629)` × 0.4
  - Integer division (`col/8` grid-snaps coordinates) is intentional; cast to `f32` before passing to `hash_noise`
  - centered on 0 by subtracting 0.5 from each octave before weighting
- `intensity = (energy * 0.12 + chaos * 0.08).clamp(0.0, 0.20)` — `chaos` is used here

Applied: `base_col += wx` and `src_row_signed += wy` (both `i32` additions, safe).
Note: warp is computed immediately before the `base_col` let-binding and incorporated into it.

`hash_noise` uses the same form as `glitch_field`'s inner closure, with distinct seeds to avoid
correlated patterns. Grid bounds are safe: `get_cell` already returns 0 for OOB indices.

### `edge_brightness_delta`

```rust
fn edge_brightness_delta(bank: &AsciiBank, img_idx: usize, col: usize, row: usize) -> f32
```

Checks 4 cardinal neighbors in the source image via `get_cell`. Since `get_cell` returns 0 for
any OOB index (including the sentinel `9999` used for "not visible" cells), the function is safe
to call with any values — it naturally returns `0.0` for background or out-of-bounds inputs.

- If `get_cell(img_idx, col, row) == 0`: return `0.0` immediately
- Count non-zero neighbors: `north`, `south`, `east`, `west`
- Count < 3 → edge cell → return `+0.07`
- Count == 4 (all filled) → interior → return `-0.04`
- Count == 3 → return `0.0` (neutral)

### `signature_tick`

```rust
fn signature_tick(
    preset_idx: usize, col: u32, row: u32, energy: f32, sig_param: f32,
    dust_tick: u32, warp_phase: f32, transient: bool, anim_tick: u64,
) -> (f32, f32, f32)
```

Returns `(dr, dg, db)` linear delta added to cell color after compositing. Returns `(0,0,0)` if
`sig_param < 0.01`.

| Preset (`preset_idx`) | Behavior detail |
|---|---|
| 0 — SP-1200 | Row-band hash (`row/3` grouped, seed changes every 30 dust_ticks). When `tear_roll < sig_param × 0.08 × energy`: small RGB offset (e.g. `+0.04R, -0.02B`). |
| 1 — MPC60 | Returns `(0,0,0)` — snap handled in velocity block |
| 2 — S950 | Period = `anim_tick / 120` (changes every 2s at 60fps). `bloom_hash = period.wrapping_mul(2654435761)`. `bloom_roll = ((bloom_hash >> 16) & 0xFF) as f32 / 255.0`. When `bloom_roll < sig_param × 0.05`: radial flash where `(col as f32 - 27.0).powi(2) + (row as f32 - 21.0).powi(2) < 9.0`. Flash intensity: `(1.0 - dist/3.0) × sig_param × 0.20 × energy`. |
| 3 — Mirage | Column-phase sine: `(col as f32 × 0.3 + warp_phase × 0.5).sin() × sig_param × 0.06 × energy`. Returns as `(0, melt × 0.5, melt)` (blue-tinted). |
| 4 — P-2000 | `(col as f32 × 0.05 + warp_phase × 0.3).sin() × sig_param × 0.04 × energy` → returned as `(-wave × 0.3, 0.0, wave)`. |
| 5 — MPC3000 | If `transient`: `let flash = sig_param × 0.15 × energy`, return `(flash, flash, flash)`. Else `(0,0,0)`. |
| 6 — SP-303 | Block: `block_h = col/6`, `block_v = row/4`. `flicker_hash = block_h.wrapping_mul(31337).wrapping_add(block_v.wrapping_mul(7919)).wrapping_add(dust_tick/8)`. If `transient` AND `((flicker_hash >> 16) & 0xFF) < (sig_param × energy × 50.0) as u32`: return `(fl, fl × 0.7, fl × 0.5)` where `fl = sig_param × energy × 0.12`. Note: the `transient` gate is intentional — SP-303's block flicker is attack-driven (fires on sample hits), not continuous. The hash threshold provides per-block variation on each hit. |

All magnitudes remain in `0.03–0.15` linear range, always energy-gated.

---

## Section 3: Feature implementations

### Insertion order in per-cell loop

The definitive ordering (new items in **bold**):

1. **Coordinate warp offset** (new — item 3) — applied before computing `base_col` and `src_row_signed`
2. Base image sampling (unchanged)
3. Structural alpha / filter (unchanged)
4. Overlay compositing (unchanged)
5. Dust (unchanged)
6. Shimmer (unchanged)
7. GlitchBloom / Collapse (unchanged)
8. Glitch field (unchanged, but `glitch_prob` modified by items 2, 4, 8 before the loop)
9. Dust glyph (unchanged)
10. **Edge-aware brightness** (new — item 5) — applied after compositing, before brightness boost
11. Brightness boost + phrase mod (unchanged)
12. Afterglow tint (unchanged)
13. Transient flash (unchanged)
14. Idle/recovery dampening (unchanged)
15. Color temperature (unchanged)
16. Sub-bass breathing (unchanged)
17. Scanlines (unchanged)
18. Jitter flicker (unchanged)
19. **Signature tick** (new — item 7)
20. **Per-cell flicker** (new — item 9)
21. Gamma encoding (unchanged)

---

### 1. DropPhase System

**Location:** Replaces the unconditional `Moment::Collapse` trigger in the existing V6 drop detection
block (the `if entering_drop && !self.drop_detected { ... }` branch).

**What changes in the existing block:**
- Remove: `self.moment.active = Some(Moment::Collapse)`
- Add: `self.drop_phase_timer = 3 + (trigger_hash & 1)` (3 or 4 frames)
- `drop_detected` and `drop_timer` remain and continue to guard against re-entry during the cycle

**Suppress phase** (new block, checked before the per-cell loop, while `drop_phase_timer > 0`):
```
drop_phase_timer -= 1
overlay_alpha_mult = 0.05        // multiplied into all slot raw_alpha values
glitch_prob = 0.0
dust_density *= 0.20
effective_smear = 0.0
```

`overlay_alpha_mult` integration: Introduce `let mut overlay_alpha_mult: f32 = 1.0;` before slot
construction (default value suppresses nothing). In the slot alpha assignment, multiply by it:
`slot.alpha = (if overlay_recovery { raw_alpha * 0.7 } else { raw_alpha }) * overlay_alpha_mult;`
The suppress phase sets it to `0.05`. Outside the suppress phase it stays `1.0`, having no effect.

**Re-entry** (new block, checked when `drop_phase_timer` just reached 0):
```
drop_reentry_timer = 10
force Moment::GlitchBloom (sets moment.active, timer, duration, bloom_center)
```

**Re-entry amplification** (checked while `drop_reentry_timer > 0`, before per-cell loop):
```
drop_reentry_timer -= 1
glitch_prob *= (1.8 + self.intent_chaos * 0.4)   // up to ×2.2
```

The existing V6 re-entry path (energy > 0.5 && drop_timer > 10 → GlitchBloom) is kept as-is for cases where DropPhase was not triggered.

---

### 2. Hero Lock

**Location:** Physics block, before the core velocity update.

**Condition:** `let hero_lock = visual_state == 3 || lockin_active;`

When `hero_lock`:
- `CORE_PULL` is a module-level const and cannot be modified directly. Introduce a local before the
  core_pos update block: `let mut core_pull_factor = CORE_PULL;` Apply Intent Mode multipliers to
  this local (see item 4 table row "CORE_PULL multiplier (local)"). Then replace the `CORE_PULL`
  literal in the core_pos lines with `core_pull_factor`. When `hero_lock`, set
  `core_pull_factor = CORE_PULL * 5.0` (= 0.15 if unchanged — applied before Intent Mode modifiers).
- After normal damping: `self.velocity_row *= 0.85; self.velocity_col *= 0.85;`
- Set `self.overlay_anchors` to 4 rotationally-symmetric positions around `self.core_anchor` at radius 8:
  ```rust
  for i in 0..4 {
      let angle = i as f32 * std::f32::consts::FRAC_PI_2;
      self.overlay_anchors[i] = (
          self.core_anchor.0 + angle.sin() * 8.0,
          self.core_anchor.1 + angle.cos() * 8.0,
      );
  }
  ```
  This writes permanently to `self.overlay_anchors` for the duration of PEAK/LockIn. The existing
  per-slot physics (pull toward anchor, soft collision avoidance) continues running normally and
  will resolve overlapping positions organically.
  **Guard unconditional anchor rewrites:** Any existing code that unconditionally overwrites
  `self.overlay_anchors[i]` (e.g., ANCHORS lookup on transient for slot 1, `prev_core_anchor`
  assignment for slot 3) must be wrapped with `if !hero_lock { ... }` so that Hero Lock's
  rotationally-symmetric positions are not immediately clobbered.
- `glitch_prob *= 0.90` — applied after its computation, before per-cell loop

---

### 3. Global Field Warping

**Location:** Per-cell loop, step 1 (before `base_col` / `src_row_signed` computation).

```rust
let (wx, wy) = warp_offset(col, row, self.warp_phase, self.intent_chaos, energy);
let base_col = cu as i32 + col_drift - core_col_off + phase_shift + jitter_offset + wx;
let src_row_signed = bank_row as i32 + row_scroll as i32 - core_row_off + wy;
```

Chaos Mode (item 4) adds `0.08` to `chaos` passed into `warp_offset` via `self.intent_chaos`.

`warp_phase` advances by `0.007` per frame, added at the end of `UpdateFrameBuffer` alongside
`glitch_field_phase`:
```rust
self.warp_phase += 0.007;
```

---

### 4. Intent → Rendering Modes

**Location:** After intent model update (`self.intent_tension`, `self.intent_chaos`,
`self.intent_release` are computed), before per-cell loop.

**`bars_this_frame` scoping:** Add the outer declaration before the `if playing` block:
```rust
let bars_this_frame = if playing { 1.0 / ticks_per_bar.max(1.0) } else { 0.0 };
```
Then remove the existing `let bars_this_frame = ...` declaration inside the `if playing` block and
use the outer one instead. `ticks_per_bar` is already computed before the `if playing` block so the
hoist is valid.

**Mode detection:**
```rust
let dominant_intent = self.intent_tension.max(self.intent_chaos).max(self.intent_release);
let new_mode: u8 = if dominant_intent < 0.30 { 0 }
    else if self.intent_tension >= self.intent_chaos && self.intent_tension >= self.intent_release { 1 }
    else if self.intent_chaos >= self.intent_release { 2 }
    else { 3 };

if new_mode == self.intent_mode && new_mode != 0 {
    self.intent_mode_bars += bars_this_frame;
} else {
    self.intent_mode = new_mode;
    self.intent_mode_bars = 0.0;
}

let target_t = if self.intent_mode_bars >= 2.0 { 1.0f32 } else { 0.0 };
let t_rate = if target_t > self.intent_mode_t { 0.08 } else { 0.12 };
self.intent_mode_t += (target_t - self.intent_mode_t) * t_rate;
let imt = self.intent_mode_t;
```

**Mode effects** (applied to already-computed variables before per-cell loop):

| Parameter | Tension (`intent_mode == 1`) | Chaos (`intent_mode == 2`) | Release (`intent_mode == 3`) |
|---|---|---|---|
| `row_damping` (local) | `× (1.0 + 0.04 × imt)` | `× (1.0 - 0.06 × imt)` | — |
| `glitch_prob` | `× (1.0 - 0.15 × imt)` | `× (1.0 + 0.20 × imt)` | — |
| `self.intent_chaos` (passed to warp) | — | `+ 0.08 × imt` | — |
| `effective_smear` | — | — | `+ 0.10 × imt` |
| `overlay_visibility` | — | — | `× (1.0 - 0.08 × imt)` |
| `core_pull_factor` (local) | `× (1.0 + 0.3 × imt)` | `× (1.0 - 0.2 × imt)` | — |

**`row_damping` local:** `self.visual_profile.row_damping` is used directly in the velocity block.
Introduce `let mut row_damping = self.visual_profile.row_damping;` before this Intent Mode block,
apply the mode multiplier to `row_damping`, then replace `self.visual_profile.row_damping` in the
velocity block with the local `row_damping`.

**Patch ordering:** Activity Reduction patches (item 8) apply first: `glitch_prob × 0.95` and
`overlay_visibility × 0.96` are computed before the Intent Mode block runs. Intent Mode modifiers
then apply on top of those already-reduced values.

---

### 5. Edge-Aware Brightness

**Location:** Per-cell loop, step 10 — after compositing, before brightness boost block.

```rust
if is_base {
    let edge_delta = edge_brightness_delta(&self.ascii_bank, core_img, bank_col_base, src_row as usize);
    if is_light {
        r = (r - edge_delta).clamp(0.0, 1.0);
        g = (g - edge_delta).clamp(0.0, 1.0);
        b = (b - edge_delta).clamp(0.0, 1.0);
    } else {
        r = (r + edge_delta).clamp(0.0, 1.0);
        g = (g + edge_delta).clamp(0.0, 1.0);
        b = (b + edge_delta).clamp(0.0, 1.0);
    }
}
```

Applied only to base-image cells. Does not affect overlays or dust.

Note: `bank_col_base` and `src_row` may be the sentinel value `9999` for out-of-bounds cells. This
is safe — `edge_brightness_delta` calls `get_cell`, which returns 0 for OOB, so the function
returns `0.0` immediately (the `if get_cell == 0 { return 0.0 }` guard fires first).

---

### 6. Temporal Echo (SR-Driven Rhythm)

**Location:** The smear lerp block for `prev_row_scroll` / `prev_col_drift` only. Overlay smear
at lines 999–1000 is unchanged.

Replace the existing lerp rate:
```rust
// Before (existing):
self.prev_row_scroll += (new_row_scroll_f - self.prev_row_scroll) * (1.0 - effective_smear);
self.prev_col_drift  += (new_col_drift_f  - self.prev_col_drift)  * (1.0 - effective_smear);

// After (new):
// Note: `step_interval` is an existing local variable already in scope at this point (not a new
// field). Use `self.quant_frame as u64 % step_interval` if types differ, or cast as needed to
// match the existing type of `step_interval` in the codebase.
let smear_rate = if sr_effect > 0.30 && step_interval > 1 {
    let hold = (self.quant_frame % step_interval as u32) != 0;
    if hold { 0.02 } else { 1.0 - effective_smear }
} else {
    1.0 - effective_smear
};
self.prev_row_scroll += (new_row_scroll_f - self.prev_row_scroll) * smear_rate;
self.prev_col_drift  += (new_col_drift_f  - self.prev_col_drift)  * smear_rate;
```

Hold frames: position nearly frozen. Release frames: normal smear. At `step_interval = 8`
(lowest SR), creates an 8-tick lock-then-snap rhythm — the sampler's native grid period.

---

### 7. Per-Preset Signature Behaviors

**MPC60 grid snap** (`preset_idx == 1`) — velocity block, after normal velocity + damping:
```rust
// MPC60: quantized grid snap (preset_idx 1 = MPC60, see PRESETS array)
if self.preset_idx == 1 {
    self.velocity_row = (self.velocity_row * 2.0).round() / 2.0;
    self.velocity_col = (self.velocity_col * 2.0).round() / 2.0;
}
```

**All other presets** — per-cell loop, step 19 (after compositing, after signature tick):
```rust
let (sr, sg, sb) = signature_tick(
    self.preset_idx, col, row, energy,
    self.visual_profile.sig_param,
    dust_tick, self.warp_phase, transient, self.anim_tick,
);
r = (r + sr).clamp(0.0, 1.0);
g = (g + sg).clamp(0.0, 1.0);
b = (b + sb).clamp(0.0, 1.0);
```

---

### 8. Global Activity Reduction

Three one-line patches to existing computed values — applied before the per-cell loop:

```rust
// (a) After existing glitch_prob computation:
let glitch_prob = glitch_prob * 0.95;

// (b) After existing dust_density computation:
let dust_density = (dust_density - 0.02).max(0.0);

// (c) After computing overlay_visibility:
let overlay_visibility = overlay_visibility * 0.96;
```

Note: `overlay_visibility = mix * 0.80` is already a named local variable; multiplying it by 0.96
uniformly reduces all slot alphas that reference it without touching the slot construction logic.

---

### 9. Subtle Per-Cell Flicker

**Location:** Per-cell loop, step 20 — after signature tick, before gamma encoding.

```rust
if energy > 0.05 {
    let fh = col.wrapping_mul(2246822507)
        .wrapping_add(row.wrapping_mul(1664525))
        .wrapping_add(self.anim_tick as u32 * 6364136);
    let flicker = ((fh >> 16) & 0xFF) as f32 / 255.0 - 0.5;  // -0.5 to +0.5
    let amt = flicker * energy * 0.016;  // ±0.008 linear (~±2 RGB/255)
    r = (r + amt).clamp(0.0, 1.0);
    g = (g + amt * 0.96).clamp(0.0, 1.0);
    b = (b + amt * 1.04).clamp(0.0, 1.0);
}
```

Slight R/B divergence simulates analog DAC instability. Imperceptible at rest. Present at high energy.

---

## Section 4: Integration touchpoints

### Initialization

All 6 new `EditorData` fields initialize to `0` / `0.0`. `sig_param` is a constant in each
`VISUAL_PROFILES` entry.

### `warp_phase` clock

```rust
// End of UpdateFrameBuffer, alongside glitch_field_phase:
self.warp_phase += 0.007;
self.glitch_field_phase += 0.01;  // unchanged
```

### VisualProfile interpolation

`sig_param` joins the `vr = 0.04` lerp block:
```rust
vp.sig_param += (vp_target.sig_param - vp.sig_param) * vr;
```

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
| `editor.rs` | 6 `EditorData` fields, 1 `VisualProfile` field + `sig_param` per preset, 3 helper fns, `warp_phase` clock, 9 feature insertions |
| All other files | No changes |
