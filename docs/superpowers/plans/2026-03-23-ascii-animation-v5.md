# ASCII Animation V5 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Upgrade `editor.rs` with 9 new visual animation features (DropPhase, Hero Lock, Global Field Warping, Intent Modes, Edge-Aware Brightness, Temporal Echo, Per-Preset Signatures, Activity Reduction, Per-Cell Flicker) as specified in the V5 design spec.

**Architecture:** All changes confined to `src/editor.rs`. Three new file-scoped helper fns added above the per-cell loop (`warp_offset`, `edge_brightness_delta`, `signature_tick`). One new `VisualProfile` field (`sig_param`), six new `EditorData` fields, and nine surgical insertions into `UpdateFrameBuffer`.

**Tech Stack:** Rust, nih-plug, no `rand` (hash-only determinism), no per-frame allocations.

**Spec:** `docs/superpowers/specs/2026-03-23-ascii-animation-v5-design.md`

---

## File Map

| File | Change |
|---|---|
| `src/editor.rs` | Only file modified — all 9 features + 3 helpers + state fields |

Key line references in current `editor.rs` (verify before editing — may shift):
- Line 66: `VisualProfile` struct definition
- Lines 87–130: `VISUAL_PROFILES` array
- Line 215: `fn glitch_field(...)` — add new helpers after `select_biased_image` (~line 245)
- Lines 262–398: `EditorData` struct
- Line 519: VisualProfile interpolation block (`vr = 0.04`)
- Line 582: `let overlay_visibility = mix * 0.80;`
- Lines 597–602: `glitch_prob` computation — **change to `let mut glitch_prob`**
- Lines 664–667: BPM timing / `ticks_per_bar`
- Line 671: `let bars_this_frame = ...` (inside `if playing` — to be hoisted)
- Lines 691, 847–850: Unconditional `overlay_anchors` writes (to gain `if !hero_lock` guards)
- Lines 747–748: `CORE_PULL` usage in core_pos update
- Lines 793–798: Velocity damping block (Hero Lock extra damping + MPC60 snap go here)
- Lines 803–804: Smear lerp for `prev_row_scroll`/`prev_col_drift` (Temporal Echo replaces this)
- Lines 916–993: Slot construction — make `slots` into `let mut slots`
- Lines 916–993: `slots` construction — changed to `let mut slots`; `overlay_alpha_mult` applied via post-pass `iter_mut()` AFTER suppress block (line 985 is NOT changed — the post-pass replaces inline multiplication)
- Lines 1003–1006: `dust_density` computation — **change to `let mut dust_density`**
- Lines 1080–1111: Drop detection block (DropPhase replaces the `Moment::Collapse` trigger)

**Structural note on execution order:** `core_pos` update runs at ~line 747 and the velocity block at ~793. Both run long before intent model update (~1064). To let Intent Mode and Hero Lock affect `core_pull_factor` for the current frame, both Hero Lock and Intent Mode read `self.intent_mode_t` (a smoothed value from previous frame) immediately after `glitch_prob` is computed — before all physics. This is intentional: `intent_mode_t` is always one frame behind, and since it's smoothed over ~8–12 frames, this one-frame lag is imperceptible.
- Line 1205: Per-cell loop `for row in 0..ROWS`
- Lines 1218–1232: `phase_shift`, `jitter_offset`, `base_col`, `src_row_signed` (warp inserted before line 1229)
- Line 1554: Brightness boost block (edge brightness inserted before this)
- Lines 1625–1634: Jitter flicker block (signature tick + per-cell flicker go after this, before gamma at line 1639)
- Line 1686: `self.glitch_field_phase += 0.01;` (add `self.warp_phase += 0.007;` here)

---

## Task 1: Add `sig_param` to `VisualProfile`

**Files:**
- Modify: `src/editor.rs:66-85` (`VisualProfile` struct)
- Modify: `src/editor.rs:87-130` (`VISUAL_PROFILES` array)
- Modify: `src/editor.rs:519-544` (VisualProfile interpolation block)

- [ ] **Step 1: Add field to struct**

In `VisualProfile` (line ~85, after `moment_bias`), add:
```rust
    // ── V5 (new) ──
    sig_param: f32,   // signature effect intensity (0.0 = off)
```

- [ ] **Step 2: Add `sig_param` to each `VISUAL_PROFILES` entry**

Add `sig_param: X.X` to each entry:
```
SP-1200:  sig_param: 0.7,
MPC60:    sig_param: 0.5,
S950:     sig_param: 0.4,
Mirage:   sig_param: 0.8,
P-2000:   sig_param: 0.5,
MPC3000:  sig_param: 0.6,
SP-303:   sig_param: 0.7,
```

- [ ] **Step 3: Add `sig_param` to the interpolation block**

After the last existing lerp line in the `vr = 0.04` block (after `for i in 0..6 { ... }`), add:
```rust
vp.sig_param += (vp_target.sig_param - vp.sig_param) * vr;
```

- [ ] **Step 4: Verify compile**

```bash
cargo build 2>&1 | head -40
```
Expected: no errors about `sig_param`.

- [ ] **Step 5: Commit**

```bash
git add src/editor.rs
git commit -m "feat(v5): add sig_param field to VisualProfile and VISUAL_PROFILES"
```

---

## Task 2: Add 6 new `EditorData` fields

**Files:**
- Modify: `src/editor.rs:364-398` (`EditorData` struct, after `first_load_img1`)

- [ ] **Step 1: Add fields at the end of `EditorData` (before the closing `}`)**

After the `first_load_img1` field (line ~397), add:
```rust
    // ── V5 (new): DropPhase system ──
    #[lens(ignore)]
    pub drop_phase_timer: u32,
    #[lens(ignore)]
    pub drop_reentry_timer: u32,
    // ── V5 (new): Field warp ──
    #[lens(ignore)]
    pub warp_phase: f32,
    // ── V5 (new): Intent rendering modes ──
    #[lens(ignore)]
    pub intent_mode: u8,
    #[lens(ignore)]
    pub intent_mode_t: f32,
    #[lens(ignore)]
    pub intent_mode_bars: f32,
```

- [ ] **Step 2: Initialize new fields in `EditorData` construction**

`EditorData` is constructed in `src/editor.rs` inside the `create_editor` function (around line 1761 — search for `EditorData {`). All other files, including `lib.rs`, are untouched per the spec. Add:
```rust
drop_phase_timer: 0,
drop_reentry_timer: 0,
warp_phase: 0.0,
intent_mode: 0,
intent_mode_t: 0.0,
intent_mode_bars: 0.0,
```

- [ ] **Step 3: Verify compile**

```bash
cargo build 2>&1 | head -40
```
Expected: no missing field errors.

- [ ] **Step 4: Commit**

```bash
git add src/editor.rs
git commit -m "feat(v5): add 6 new EditorData fields for drop phase, warp, and intent modes"
```

---

## Task 3: Add three helper functions

**Files:**
- Modify: `src/editor.rs` — add after `select_biased_image` (~line 245)

- [ ] **Step 1: Add `warp_offset`**

After the closing `}` of `select_biased_image` and before `get_composition_anchors`, insert:

```rust
/// V5: Global field warp — returns (wx, wy) coordinate displacement
fn warp_offset(col: u32, row: u32, phase: f32, chaos: f32, energy: f32) -> (i32, i32) {
    let hash_noise = |cx: f32, cy: f32, seed: u32| -> f32 {
        let ix = cx as i32 as u32;
        let iy = cy as i32 as u32;
        let s = ix.wrapping_mul(2246822507)
            .wrapping_add(iy.wrapping_mul(1664525))
            .wrapping_add(seed);
        ((s >> 16) & 0xFF) as f32 / 255.0
    };
    let intensity = (energy * 0.12 + chaos * 0.08).clamp(0.0, 0.20);
    let warp_x = (row as f32 * 0.2 + phase).sin() * intensity;
    let oct1 = (hash_noise((col / 8) as f32, (row / 8) as f32, 7331) - 0.5) * 0.6;
    let oct2 = (hash_noise((col / 4) as f32, (row / 4) as f32, 8629) - 0.5) * 0.4;
    let warp_y = (oct1 + oct2) * intensity;
    let wx = warp_x.round() as i32;
    let wy = warp_y.round() as i32;
    (wx.clamp(-2, 2), wy.clamp(-2, 2))
}

/// V5: Edge-aware brightness — detects edge vs interior cells via neighbor sampling
fn edge_brightness_delta(bank: &crate::ascii_bank::AsciiBank, img_idx: usize, col: usize, row: usize) -> f32 {
    if bank.get_cell(img_idx, col, row) == 0 { return 0.0; }
    let n = (bank.get_cell(img_idx, col, row.wrapping_sub(1)) > 0) as u32;
    let s = (bank.get_cell(img_idx, col, row + 1) > 0) as u32;
    let e = (bank.get_cell(img_idx, col + 1, row) > 0) as u32;
    let w = (bank.get_cell(img_idx, col.wrapping_sub(1), row) > 0) as u32;
    let filled = n + s + e + w;
    match filled {
        4 => -0.04,  // interior
        3 => 0.0,    // neutral
        _ => 0.07,   // edge (0, 1, or 2 filled neighbors)
    }
}

/// V5: Per-preset signature tick — returns (dr, dg, db) linear color delta
fn signature_tick(
    preset_idx: usize,
    col: u32,
    row: u32,
    energy: f32,
    sig_param: f32,
    dust_tick: u32,
    warp_phase: f32,
    transient: bool,
    anim_tick: u64,
) -> (f32, f32, f32) {
    if sig_param < 0.01 { return (0.0, 0.0, 0.0); }
    match preset_idx {
        0 => {
            // SP-1200: horizontal tearing bands
            let band = row / 3;
            let seed = band.wrapping_mul(2246822507).wrapping_add(dust_tick / 30);
            let tear_roll = ((seed >> 16) & 0xFF) as f32 / 255.0;
            if tear_roll < sig_param * 0.08 * energy {
                (0.04, 0.0, -0.02)
            } else {
                (0.0, 0.0, 0.0)
            }
        }
        1 => (0.0, 0.0, 0.0), // MPC60: snap handled in velocity block
        2 => {
            // S950: rare symmetric bloom (period = 2s at 60fps)
            let period = anim_tick / 120;
            let bloom_hash = period.wrapping_mul(2654435761);
            let bloom_roll = ((bloom_hash >> 16) & 0xFF) as f32 / 255.0;
            if bloom_roll < sig_param * 0.05 {
                let dist2 = (col as f32 - 27.0).powi(2) + (row as f32 - 21.0).powi(2);
                if dist2 < 9.0 {
                    let dist = dist2.sqrt();
                    let flash = (1.0 - dist / 3.0) * sig_param * 0.20 * energy;
                    (flash, flash, flash)
                } else {
                    (0.0, 0.0, 0.0)
                }
            } else {
                (0.0, 0.0, 0.0)
            }
        }
        3 => {
            // Mirage: vertical melt shimmer (blue-tinted)
            let melt = (col as f32 * 0.3 + warp_phase * 0.5).sin() * sig_param * 0.06 * energy;
            (0.0, melt * 0.5, melt)
        }
        4 => {
            // P-2000: analog wave drift
            let wave = (col as f32 * 0.05 + warp_phase * 0.3).sin() * sig_param * 0.04 * energy;
            (-wave * 0.3, 0.0, wave)
        }
        5 => {
            // MPC3000: sharp transient flash
            if transient {
                let flash = sig_param * 0.15 * energy;
                (flash, flash, flash)
            } else {
                (0.0, 0.0, 0.0)
            }
        }
        6 => {
            // SP-303: attack-driven block flicker
            let block_h = col / 6;
            let block_v = row / 4;
            let flicker_hash = block_h.wrapping_mul(31337)
                .wrapping_add(block_v.wrapping_mul(7919))
                .wrapping_add(dust_tick / 8);
            if transient && ((flicker_hash >> 16) & 0xFF) < (sig_param * energy * 50.0) as u32 {
                let fl = sig_param * energy * 0.12;
                (fl, fl * 0.7, fl * 0.5)
            } else {
                (0.0, 0.0, 0.0)
            }
        }
        _ => (0.0, 0.0, 0.0),
    }
}
```

- [ ] **Step 2: Verify compile**

```bash
cargo build 2>&1 | head -40
```
Expected: no errors. `warp_offset`, `edge_brightness_delta`, `signature_tick` compile clean.

- [ ] **Step 3: Commit**

```bash
git add src/editor.rs
git commit -m "feat(v5): add warp_offset, edge_brightness_delta, signature_tick helpers"
```

---

## Task 4: Feature 8 — Global Activity Reduction

**Files:**
- Modify: `src/editor.rs` — three one-line patches before the per-cell loop

This is the simplest patch — do it first so all later features build on reduced baseline values.

- [ ] **Step 1: Patch `glitch_prob`**

Find the `glitch_prob` computation (line ~597–602). Change `let glitch_prob =` to `let mut glitch_prob =` and multiply the entire expression by `* 0.95` at the end. The result should look like:
```rust
let mut glitch_prob = (... original expression ...) * 0.95;  // V5: mutable; global activity reduction applied at declaration
```

This single change makes `glitch_prob` mutable for all subsequent Tasks (6, 7, 8) without extra shadow bindings.

- [ ] **Step 2: Patch `dust_density`**

Find the dust density lines (~1003–1006). After the transient dust boost line:
```rust
let dust_density = if transient { (dust_density + 0.20).min(0.90) } else { dust_density };
```
Add:
```rust
let dust_density = (dust_density - 0.02).max(0.0);  // V5: global activity reduction
```

- [ ] **Step 3: Patch `overlay_visibility`**

Find line ~582: `let overlay_visibility = mix * 0.80;`. Change to:
```rust
let overlay_visibility = mix * 0.80 * 0.96;  // V5: global activity reduction (×0.96)
```

- [ ] **Step 4: Verify compile and run**

```bash
cargo build 2>&1 | head -20
```

- [ ] **Step 5: Commit**

```bash
git add src/editor.rs
git commit -m "feat(v5): global activity reduction (glitch ×0.95, dust −0.02, overlay ×0.96)"
```

---

## Task 5: Feature 6 — Temporal Echo (SR-Driven Rhythm)

**Files:**
- Modify: `src/editor.rs:803-804` — the smear lerp for `prev_row_scroll` / `prev_col_drift`

- [ ] **Step 1: Replace the smear lerp**

Find lines 803–804:
```rust
self.prev_row_scroll += (new_row_scroll_f - self.prev_row_scroll) * (1.0 - effective_smear);
self.prev_col_drift  += (new_col_drift_f  - self.prev_col_drift)  * (1.0 - effective_smear);
```

Replace with:
```rust
// V5: Temporal Echo — SR-driven lock-then-snap rhythm
let smear_rate = if sr_effect > 0.30 && step_interval > 1 {
    let hold = (self.quant_frame % step_interval) != 0;
    if hold { 0.02 } else { 1.0 - effective_smear }
} else {
    1.0 - effective_smear
};
self.prev_row_scroll += (new_row_scroll_f - self.prev_row_scroll) * smear_rate;
self.prev_col_drift  += (new_col_drift_f  - self.prev_col_drift)  * smear_rate;
```

Note: `step_interval` is a `u64` local. If `self.quant_frame` is also `u64` (confirmed: line 290), the `%` operation is `u64 % u64` — no cast needed.

- [ ] **Step 2: Verify compile**

```bash
cargo build 2>&1 | head -20
```

- [ ] **Step 3: Commit**

```bash
git add src/editor.rs
git commit -m "feat(v5): temporal echo — SR-driven lock-then-snap smear rhythm"
```

---

## Task 6: Feature 1 — DropPhase System

**Files:**
- Modify: `src/editor.rs:1080-1111` — drop detection block
- Modify: `src/editor.rs:916-993` — slot construction (make `slots` mutable, apply `overlay_alpha_mult` after)

This is the most structurally complex feature. Read the spec section carefully before editing.

**Key ordering insight:** `overlay_alpha_mult` must be applied *after* slot construction (so the suppress block can run after slots are built and then retroactively scale their alphas). This requires making `slots` a `let mut` array.

- [ ] **Step 1: Make `slots` mutable and add `overlay_alpha_mult` post-pass**

Find: `let slots: [OverlaySlot; NUM_SLOTS] = std::array::from_fn(|i| {`

Change to: `let mut slots: [OverlaySlot; NUM_SLOTS] = std::array::from_fn(|i| {`

After the closing `});` of the slots construction block (~line 993), add:
```rust
let mut overlay_alpha_mult: f32 = 1.0;  // V5: DropPhase suppress (1.0 = no effect, 0.05 = suppressed)
```
This variable will be set in the suppress block and then applied to slots just before the per-cell loop.

**Important:** Do NOT change line 985 (`alpha: if overlay_recovery { raw_alpha * 0.7 } else { raw_alpha }`). The File Map note about "multiply by `overlay_alpha_mult`" at line 985 is superseded by this post-pass approach. The post-pass in Step 3 replaces any need for an inline multiplication at construction time. Changing both would double-suppress the alpha (e.g., 0.05 × 0.05 = 0.0025 during a suppress frame).

- [ ] **Step 2: Replace the DropPhase trigger in the drop detection block**

Find lines 1082–1091 — the `if entering_drop && !self.drop_detected { ... }` block:
```rust
if entering_drop && !self.drop_detected {
    self.drop_detected = true;
    self.drop_timer = 0;
    // Force collapse on drop
    self.moment.active = Some(Moment::Collapse);
    self.moment.timer = 0;
    self.moment.duration = 30;
    self.moment.seed = trigger_hash;
    self.moment.cooldown = 0;
}
```

Replace with:
```rust
if entering_drop && !self.drop_detected {
    self.drop_detected = true;
    self.drop_timer = 0;
    // V5: DropPhase — visual suppress first, then GlitchBloom re-entry
    self.drop_phase_timer = 3 + (trigger_hash & 1);  // 3 or 4 frames
    // Moment::Collapse is NOT triggered — DropPhase suppress handles the visual gap
}
```

- [ ] **Step 3: Add suppress phase + re-entry blocks**

After the `dust_density` activity reduction line (Task 4) and before the per-cell loop (`for row in 0..ROWS`), add:

```rust
// V5: DropPhase suppress phase (mutates overlay_alpha_mult, glitch_prob, dust_density, effective_smear)
if self.drop_phase_timer > 0 {
    self.drop_phase_timer -= 1;
    overlay_alpha_mult = 0.05;
    glitch_prob = 0.0;
    dust_density *= 0.20;
    effective_smear = 0.0;
    // When timer just reached 0: force GlitchBloom re-entry
    if self.drop_phase_timer == 0 {
        self.drop_reentry_timer = 10;
        // Compute local hash — trigger_hash is out of scope here (inside moment trigger block)
        let reentry_hash = (self.anim_tick as u32).wrapping_mul(2654435761);
        self.moment.active = Some(Moment::GlitchBloom);
        self.moment.timer = 0;
        self.moment.duration = 15;
        self.moment.seed = reentry_hash;
        self.moment.bloom_center = (
            ((reentry_hash >> 4) as usize % 54),
            ((reentry_hash >> 14) as usize % 42),
        );
    }
}
// V5: DropPhase re-entry amplification
if self.drop_reentry_timer > 0 {
    self.drop_reentry_timer -= 1;
    glitch_prob *= 1.8 + self.intent_chaos * 0.4;  // up to ×2.2
}

// V5: Apply overlay_alpha_mult to slots (must happen after suppress block has a chance to set it)
if overlay_alpha_mult < 1.0 {
    for s in slots.iter_mut() {
        s.alpha *= overlay_alpha_mult;
    }
}
```

Note: `glitch_prob`, `dust_density`, `effective_smear` must be `let mut` at their declaration sites (handled by Task 4 and the `mut` changes from Tasks 7/8). The `overlay_alpha_mult` application is a post-pass over the `mut slots` array, which ensures the suppress value is applied regardless of when slots were constructed.

- [ ] **Step 4: Verify compile**

```bash
cargo build 2>&1 | head -40
```

- [ ] **Step 5: Commit**

```bash
git add src/editor.rs
git commit -m "feat(v5): DropPhase system — suppress + GlitchBloom re-entry"
```

---

## Task 7: Feature 2 — Hero Lock + Pre-physics Intent Mode locals

**Files:**
- Modify: `src/editor.rs:597-603` — after `glitch_prob` computation (pre-physics block)
- Modify: `src/editor.rs:747-799` — core_pos update and velocity block
- Modify: `src/editor.rs:691, 845-850` — unconditional anchor overwrites

**Ordering insight:** `core_pull_factor` and `row_damping` are consumed by the core_pos update (~747) and velocity block (~793) respectively — both before the intent model update at ~1064. To let Intent Mode affect these for the current frame, Intent Mode must read `self.intent_mode_t` (smoothed from the previous frame — one frame of lag is imperceptible given the 8–12 frame smoothing window) and apply its physics multipliers right after `glitch_prob` is computed, before all physics loops.

- [ ] **Step 1: Add pre-physics block immediately after `glitch_prob` computation**

After the `glitch_prob` line (line ~602: `} * self.glitch_scale * self.visual_profile.glitch_mult;`), add:

```rust
let mut glitch_prob = glitch_prob;  // V5: make mutable for modifications below

// V5: Pre-physics locals for Hero Lock + Intent Mode
let hero_lock = visual_state == 3 || lockin_active;
let mut core_pull_factor = if hero_lock { CORE_PULL * 5.0 } else { CORE_PULL };
let mut row_damping = self.visual_profile.row_damping;

// Apply Intent Mode effects from previous frame's intent_mode_t (one-frame lag is imperceptible)
let imt = self.intent_mode_t;
match self.intent_mode {
    1 => { // Tension
        row_damping *= 1.0 + 0.04 * imt;
        glitch_prob *= 1.0 - 0.15 * imt;
        core_pull_factor *= 1.0 + 0.3 * imt;
    }
    2 => { // Chaos
        row_damping *= 1.0 - 0.06 * imt;
        glitch_prob *= 1.0 + 0.20 * imt;
        // self.intent_chaos boost for warp applied later in Intent Mode update block
        core_pull_factor *= 1.0 - 0.2 * imt;
    }
    3 => { // Release — overlay_visibility and effective_smear modified later
        // row_damping and core_pull_factor unchanged for Release
    }
    _ => {}
}

if hero_lock { glitch_prob *= 0.90; }  // V5: Hero Lock glitch reduction
```

Note: `overlay_visibility` and `effective_smear` Intent Mode modifications for Release mode are applied later (see Task 8), since they are declared after this point.

- [ ] **Step 2: Replace `CORE_PULL` literal with `core_pull_factor` in core_pos update**

Find lines 747–748:
```rust
self.core_pos.0 += (self.core_anchor.0 - self.core_pos.0) * CORE_PULL;
self.core_pos.1 += (self.core_anchor.1 - self.core_pos.1) * CORE_PULL;
```
Change to:
```rust
self.core_pos.0 += (self.core_anchor.0 - self.core_pos.0) * core_pull_factor;
self.core_pos.1 += (self.core_anchor.1 - self.core_pos.1) * core_pull_factor;
```

- [ ] **Step 3: Replace `self.visual_profile.row_damping` with `row_damping` in velocity block**

Find line ~793: `self.velocity_row *= self.visual_profile.row_damping;`
Change to: `self.velocity_row *= row_damping;`

- [ ] **Step 4: Add Hero Lock extra damping + symmetric anchor write**

After the velocity clamping lines (~line 798), add:
```rust
// V5: Hero Lock extra velocity damping + symmetric anchor snap
if hero_lock {
    self.velocity_row *= 0.85;
    self.velocity_col *= 0.85;
    for i in 0..4 {
        let angle = i as f32 * std::f32::consts::FRAC_PI_2;
        self.overlay_anchors[i] = (
            self.core_anchor.0 + angle.sin() * 8.0,
            self.core_anchor.1 + angle.cos() * 8.0,
        );
    }
}
```

- [ ] **Step 5: Guard unconditional anchor overwrites**

At line ~691 (phrase boundary — `for i in 0..4 { self.overlay_anchors[i] = anchors[1 + i]; }`):
```rust
if !hero_lock { for i in 0..4 { self.overlay_anchors[i] = anchors[1 + i]; } }
```

At line ~847 (transient accent retarget):
```rust
if transient && !hero_lock {
    let accent_hash = (self.anim_tick as u32).wrapping_mul(1664525);
    self.overlay_anchors[1] = ANCHORS[(accent_hash as usize) % ANCHORS.len()];
}
```

At line ~850 (ghost follows prev_core_anchor):
```rust
if !hero_lock { self.overlay_anchors[3] = self.prev_core_anchor; }
```

- [ ] **Step 6: Verify compile**

```bash
cargo build 2>&1 | head -40
```

- [ ] **Step 7: Commit**

```bash
git add src/editor.rs
git commit -m "feat(v5): hero lock — strong centering, symmetric anchors, anchor write guards"
```

---

## Task 8: Feature 4 — Intent Mode State Update + Release effects

**Files:**
- Modify: `src/editor.rs:670` — hoist `bars_this_frame`
- Modify: `src/editor.rs:662` — make `effective_smear` mutable
- Modify: `src/editor.rs:582` — make `overlay_visibility` mutable
- Modify: `src/editor.rs` — after intent model update (~line 1064), before moment trigger

**Note:** Tension/Chaos Intent Mode effects on `glitch_prob`, `row_damping`, and `core_pull_factor` are handled in Task 7's pre-physics block (reading `self.intent_mode_t` from previous frame). This task handles: (1) the mode detection + state update logic, (2) the `self.intent_chaos` boost for warp (Chaos mode), and (3) Release mode effects on `effective_smear` and `overlay_visibility` — which are declared later in the frame and can be mutated here.

- [ ] **Step 1: Hoist `bars_this_frame` outside `if playing`**

Find line ~671, inside `if playing {`:
```rust
let bars_this_frame = 1.0 / ticks_per_bar.max(1.0);
```

Before the `if playing {` block (around line 670), add:
```rust
let bars_this_frame = if playing { 1.0 / ticks_per_bar.max(1.0) } else { 0.0 };  // V5: hoisted for Intent Mode
```

Then remove the `let bars_this_frame = ...` declaration from inside the `if playing` block. The existing uses of `bars_this_frame` inside `if playing` will bind to the outer declaration.

- [ ] **Step 2: Make `effective_smear` and `overlay_visibility` mutable**

Find line ~662: `let effective_smear = ...`
Change to: `let mut effective_smear = ...`

Find line ~582: `let overlay_visibility = mix * 0.80 * 0.96;` (from Task 4)
Change to: `let mut overlay_visibility = mix * 0.80 * 0.96;`

- [ ] **Step 3: Add Intent Mode state update block**

After the intent model update block (after line ~1064: `self.intent_chaos = self.intent_chaos.clamp(0.0, 1.0);`) and before the moment trigger block, insert:

```rust
// V5: Intent → Rendering Modes — state update
// (Physics effects applied earlier via self.intent_mode_t from previous frame — see pre-physics block)
{
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
    // Intent_mode_t is written for next frame's pre-physics block

    // Apply Chaos warp boost to self.intent_chaos (feeds into warp_offset this frame via per-cell loop)
    if self.intent_mode == 2 {
        self.intent_chaos = (self.intent_chaos + 0.08 * self.intent_mode_t).clamp(0.0, 1.0);
    }

    // Release mode: apply to effective_smear and overlay_visibility (declared before this block)
    if self.intent_mode == 3 {
        let imt = self.intent_mode_t;
        effective_smear = (effective_smear + 0.10 * imt).min(0.8);
        overlay_visibility *= 1.0 - 0.08 * imt;
    }
}
```

- [ ] **Step 4: Verify compile**

```bash
cargo build 2>&1 | head -40
```

- [ ] **Step 5: Commit**

```bash
git add src/editor.rs
git commit -m "feat(v5): intent rendering modes — state update, chaos warp boost, release effects"
```

---

## Task 9: Feature 3 — Global Field Warping

**Files:**
- Modify: `src/editor.rs:1229-1232` — the `base_col` / `src_row_signed` computation
- Modify: `src/editor.rs:1686` — end of `UpdateFrameBuffer` (add `warp_phase` clock)

- [ ] **Step 1: Insert warp call before `base_col` computation**

Find lines 1229–1232:
```rust
let base_col = cu as i32 + col_drift - core_col_off + phase_shift + jitter_offset;
let bank_col_base = if base_col >= 0 { base_col as usize } else { 9999 };
let src_row_signed = bank_row as i32 + row_scroll as i32 - core_row_off;
let src_row = if src_row_signed >= 0 { src_row_signed as usize } else { 9999 };
```

Replace with:
```rust
// V5: Global field warp (step 1 — before base_col)
let (wx, wy) = warp_offset(col, row, self.warp_phase, self.intent_chaos, energy);
let base_col = cu as i32 + col_drift - core_col_off + phase_shift + jitter_offset + wx;
let bank_col_base = if base_col >= 0 { base_col as usize } else { 9999 };
let src_row_signed = bank_row as i32 + row_scroll as i32 - core_row_off + wy;
let src_row = if src_row_signed >= 0 { src_row_signed as usize } else { 9999 };
```

- [ ] **Step 2: Advance `warp_phase` at end of `UpdateFrameBuffer`**

Find line 1686: `self.glitch_field_phase += 0.01;`

Add after it:
```rust
self.warp_phase += 0.007;  // V5: field warp clock
```

- [ ] **Step 3: Verify compile**

```bash
cargo build 2>&1 | head -20
```

- [ ] **Step 4: Commit**

```bash
git add src/editor.rs
git commit -m "feat(v5): global field warping — hash FBM coordinate warp per cell"
```

---

## Task 10: Feature 5 — Edge-Aware Brightness

**Files:**
- Modify: `src/editor.rs` — per-cell loop, after compositing, before brightness boost (~line 1554)

- [ ] **Step 1: Insert edge brightness delta after compositing**

Find the brightness boost block (line ~1554):
```rust
// V3+V4: Brightness boost (moment + phrase)
```

Insert before it:
```rust
// V5: Edge-aware brightness (step 10 — after compositing, before brightness boost)
if is_base {
    let edge_delta = edge_brightness_delta(&self.ascii_bank, core_img, bank_col_base, src_row);
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

Note: `bank_col_base: usize` and `src_row: usize` match the function signature. OOB sentinel `9999` is safe — `get_cell` returns 0, triggering early return in `edge_brightness_delta`.

- [ ] **Step 2: Verify compile**

```bash
cargo build 2>&1 | head -20
```

- [ ] **Step 3: Commit**

```bash
git add src/editor.rs
git commit -m "feat(v5): edge-aware brightness — boost edges +0.07, dim interiors -0.04"
```

---

## Task 11: Feature 7 — Per-Preset Signature Behaviors

**Files:**
- Modify: `src/editor.rs:793-799` — velocity block (MPC60 snap)
- Modify: `src/editor.rs` — per-cell loop, step 19 (after jitter flicker ~line 1634)

- [ ] **Step 1: Add MPC60 grid snap in velocity block**

After the velocity clamping lines (~line 798: `self.velocity_col = self.velocity_col.clamp(-4.0, 4.0);`), add:
```rust
// V5: MPC60 grid snap (preset_idx 1 = MPC60, see PRESETS array)
if self.preset_idx == 1 {
    self.velocity_row = (self.velocity_row * 2.0).round() / 2.0;
    self.velocity_col = (self.velocity_col * 2.0).round() / 2.0;
}
```

- [ ] **Step 2: Add `signature_tick` call in per-cell loop**

After the jitter flicker block (line ~1634, the `if jitter_val > 0.1 { ... }` block closes), add:
```rust
// V5: Per-preset signature behaviors (step 19)
let (sr, sg, sb) = signature_tick(
    self.preset_idx, col, row, energy,
    self.visual_profile.sig_param,
    dust_tick, self.warp_phase, transient, self.anim_tick,
);
r = (r + sr).clamp(0.0, 1.0);
g = (g + sg).clamp(0.0, 1.0);
b = (b + sb).clamp(0.0, 1.0);
```

- [ ] **Step 3: Verify compile**

```bash
cargo build 2>&1 | head -20
```

- [ ] **Step 4: Commit**

```bash
git add src/editor.rs
git commit -m "feat(v5): per-preset signature behaviors (MPC60 snap, SP-1200 tear, S950 bloom, etc.)"
```

---

## Task 12: Feature 9 — Subtle Per-Cell Flicker

**Files:**
- Modify: `src/editor.rs` — per-cell loop, step 20 (after signature tick, before gamma)

- [ ] **Step 1: Insert flicker block after signature tick**

After the `signature_tick` block added in Task 11, add:
```rust
// V5: Subtle per-cell flicker (step 20 — after signature, before gamma)
if energy > 0.05 {
    let fh = col.wrapping_mul(2246822507)
        .wrapping_add(row.wrapping_mul(1664525))
        .wrapping_add(self.anim_tick as u32 * 6364136);
    let flicker = ((fh >> 16) & 0xFF) as f32 / 255.0 - 0.5; // -0.5 to +0.5
    let amt = flicker * energy * 0.016; // ±0.008 linear (~±2 RGB/255)
    r = (r + amt).clamp(0.0, 1.0);
    g = (g + amt * 0.96).clamp(0.0, 1.0);
    b = (b + amt * 1.04).clamp(0.0, 1.0);
}
```

- [ ] **Step 2: Verify compile**

```bash
cargo build 2>&1 | head -20
```

- [ ] **Step 3: Commit**

```bash
git add src/editor.rs
git commit -m "feat(v5): subtle per-cell flicker — analog DAC instability simulation"
```

---

## Task 13: Final integration check

- [ ] **Step 1: Full clean build**

```bash
cargo build 2>&1
```
Expected: 0 errors. Warnings about unused variables from shadowing are OK but should be minimal.

- [ ] **Step 2: Verify `mut` declarations**

The following locals must be `let mut` (not `let`) for Tasks 6–8 to compile:
- `glitch_prob` (~line 597)
- `dust_density` (~line 1005)
- `effective_smear` (~line 662)
- `overlay_visibility` (~line 582)
- `row_damping` (new local added in Task 8)
- `core_pull_factor` (new local added in Task 7)

If any of these are not `mut`, add `mut` to their declaration. This step verifies no accidental `let` was left in place.

- [ ] **Step 3: Check `trigger_hash` scope in DropPhase suppress block**

The suppress block (Task 6, Step 4) uses `trigger_hash` when `drop_phase_timer` reaches 0. Verify `trigger_hash` is in scope at that point — it's computed inside `if self.moment.active.is_none() && self.moment.cooldown == 0 { ... }`. The suppress block should either:
- (a) Be placed inside that same `if` block where `trigger_hash` is available, or
- (b) Compute its own hash for the GlitchBloom trigger (e.g., `let bloom_hash = (self.anim_tick as u32).wrapping_mul(2654435761);`)

Use option (b) — compute a local hash inside the suppress re-entry branch to avoid scope dependency:
```rust
if self.drop_phase_timer == 0 {
    self.drop_reentry_timer = 10;
    let reentry_hash = (self.anim_tick as u32).wrapping_mul(2654435761);
    self.moment.active = Some(Moment::GlitchBloom);
    self.moment.timer = 0;
    self.moment.duration = 15;
    self.moment.seed = reentry_hash;
    self.moment.bloom_center = (
        ((reentry_hash >> 4) as usize % 54),
        ((reentry_hash >> 14) as usize % 42),
    );
}
```

- [ ] **Step 4: Final commit**

```bash
git add src/editor.rs
git commit -m "feat(v5): V5 animation engine complete — all 9 features integrated"
```

---

## Implementation Notes

**Ordering of pre-loop patches:**
All pre-loop patches (Activity Reduction, DropPhase suppress, Hero Lock glitch reduction, Intent Mode effects) operate on the same set of locals. Apply them in this order:
1. Activity Reduction (Task 4) — base reductions
2. DropPhase suppress (Task 6) — may zero out glitch/dust for suppress frames
3. Hero Lock glitch reduction (Task 7)
4. Intent Mode effects (Task 8) — final multipliers on top

**`let mut` cascade:**
Tasks 4, 6, 7, and 8 all mutate `glitch_prob`. You must change its declaration to `let mut glitch_prob = ...` in Task 4 and leave it as `mut` for subsequent tasks. Same for `dust_density`, `effective_smear`, `overlay_visibility`, and the two new locals `row_damping` / `core_pull_factor`.

**Suppress block placement:**
The DropPhase suppress/re-entry block (Task 6) must be placed after `dust_density` and `glitch_prob` locals are declared (so they can be mutated), but before the per-cell loop. The re-entry GlitchBloom sets `self.moment.active` which is checked at the top of the next frame's moment system — this is correct (it activates one frame later, by design).

**`self.intent_chaos` mutation in Intent Mode:**
Chaos Mode adds `0.08 * imt` to `self.intent_chaos`. This mutates the field, which `warp_offset` reads in the per-cell loop via `self.intent_chaos`. The Intent Mode block runs before the per-cell loop, so the boosted value flows correctly into warp.
