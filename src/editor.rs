use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::{create_vizia_editor, ViziaState, ViziaTheming};
use std::sync::Arc;

use crate::SssssssssamplerParams;
use crate::AnimationParams;
use crate::ascii_image_display::AsciiImageDisplay;
use crate::ascii_bank::{AsciiBank, CHARSET_LEN};
use crate::render::color_system::ColorPalette;
use std::sync::Mutex;

// Window sized for 46×36 grid at proper monospace aspect (0.60 w/h ratio).
// Grid area: 46 cols × ~7.2px = 331w, 36 rows × 12px = 432h
// Chrome: header(44) + preset(32) + controls(88) = 164px
// Total: ~500w × 596h — slightly wider for breathing room
// Grid: 54×42 at cell_h≈10.4px. cell_w = 10.4 × 0.55 ≈ 5.71, total_w = 308
// Window sized to match grid (no dead space).
pub(crate) const WINDOW_WIDTH: u32 = 310;
pub(crate) const WINDOW_HEIGHT: u32 = 436;

// ─── Machine presets ──────────────────────────────────────────────────────────
//
// poles: 2.0 = 2-pole LP (SP-1200 / SP-12 lineage — under-filtered, gritty)
//        4.0 = 4-pole Butterworth (S612 / SP-303 / MPC3000 — clean)
//        6.0 = 6-pole Butterworth (S950 — 36 dB/oct switched-capacitor MF6CN-50)
// cutoff is always set to 1.0 (fully open) on preset load.

struct MachinePreset {
    _name: &'static str,
    sr: f32,
    bits: f32,
    jitter: f32,
    poles: f32,
}

const PRESETS: &[MachinePreset] = &[
    // SP-1200: 26.04 kHz, 12-bit, AD7541 DAC. Under-filtered (2-pole) → strong aliasing foldback.
    // TL084 input AA, per-channel output: SSM2044 VCF (2ch), 5-pole Chebyshev (4ch), unfiltered (2ch).
    MachinePreset { _name: "SP-1200", sr: 26_040.0, bits: 12.0, jitter: 0.01, poles: 2.0 },
    // MPC60: 40 kHz, 12-bit audio (16-bit Burr Brown PCM54HP DAC). No user filter.
    // Roger Linn design. Clean 12-bit with punch from higher SR than SP-1200.
    MachinePreset { _name: "MPC60",   sr: 40_000.0, bits: 12.0, jitter: 0.005, poles: 4.0 },
    // S950: 7.5-48 kHz variable, 12-bit, MF6CN-50 6th-order Butterworth (36 dB/oct).
    // Steep filter suppresses aliasing → smooth, warm character.
    MachinePreset { _name: "S950",    sr: 48_000.0, bits: 12.0, jitter: 0.01, poles: 6.0 },
    // Mirage: 10-33 kHz variable, 8-bit, Curtis CEM3328 4-pole resonant filter (24 dB/oct).
    // 8-bit quantization + analog resonant filter = gritty, warm, unstable character.
    MachinePreset { _name: "Mirage",  sr: 33_000.0, bits: 8.0,  jitter: 0.03, poles: 4.0 },
    // Prophet 2000: 15.6-41.7 kHz, 12-bit, Curtis CEM3379 4-pole resonant VCF (24 dB/oct).
    // Analog filter with resonance gives musical sweep character.
    MachinePreset { _name: "P-2000",  sr: 41_667.0, bits: 12.0, jitter: 0.01, poles: 4.0 },
    // MPC3000: 44.1 kHz, 16-bit in / Burr Brown PCM69A 18-bit DAC. 8× oversampling digital filter
    // + 2-pole analog AA at ~26 kHz. Clean reference — minimal coloration.
    MachinePreset { _name: "MPC3000", sr: 44_100.0, bits: 16.0, jitter: 0.0,  poles: 4.0 },
    // SP-303: 44.1 kHz, 16-bit (20-bit AD/DA), sigma-delta reconstruction, COSM DSP.
    // Character from digital effects (Vinyl Sim, Lo-Fi), not analog filtering.
    MachinePreset { _name: "SP-303",  sr: 44_100.0, bits: 16.0, jitter: 0.01, poles: 4.0 },
];

// S950 — matches the default target_sr in lib.rs (48_000 Hz)
const DEFAULT_PRESET: usize = 2;

// ── Visual profiles: per-preset motion/behavior characteristics ──────────────
#[derive(Clone)]
pub(crate) struct VisualProfile {
    row_damping: f32,
    col_damping: f32,
    bpm_force: f32,
    dust_density: f32,
    glitch_mult: f32,
    step_quant_mult: f32,
    smear_base: f32,
    transition_speed: f32,
    overlay_speed: f32,
    micro_freeze_thresh: u8,
    moment_mult: f32,
    dust_style: u8,     // 0=random, 1=grid-aligned, 2=chaotic-drift
    glitch_style: u8,   // 0=mixed, 1=horizontal-line, 2=warped-melt, 3=minimal
    bloom_shape: u8,    // 0=rectangle, 1=horizontal-scanline, 2=radial, 3=jagged
    // ── V6 ──
    scanline_amt: f32,  // 0.0–1.0: scanline visibility at low SR
    motion_echo: f32,   // 0.0–0.5: velocity trail intensity
    moment_bias: [f32; 6], // [FreezeCut, GlitchBloom, LockIn, PhaseWave, Collapse, Afterglow]
    // ── V5 (new) ──
    sig_param: f32,   // signature effect intensity (0.0 = off)
}

const VISUAL_PROFILES: &[VisualProfile] = &[
    // SP-1200: bouncy, gritty, punchy — grid dust, h-line glitch, scanline bloom
    VisualProfile { row_damping: 0.88, col_damping: 0.86, bpm_force: 0.45, dust_density: 0.68,
        glitch_mult: 1.35, step_quant_mult: 1.3, smear_base: 0.2, transition_speed: 0.3,
        overlay_speed: 1.3, micro_freeze_thresh: 18, moment_mult: 1.3,
        dust_style: 1, glitch_style: 1, bloom_shape: 1,
        scanline_amt: 0.6, motion_echo: 0.15, moment_bias: [1.5, 1.3, 0.8, 0.7, 1.0, 1.0],
        sig_param: 0.7 },
    // MPC60: rhythmic, transient-driven — standard dust/glitch/bloom
    VisualProfile { row_damping: 0.90, col_damping: 0.88, bpm_force: 0.40, dust_density: 0.56,
        glitch_mult: 0.55, step_quant_mult: 1.1, smear_base: 0.25, transition_speed: 0.4,
        overlay_speed: 1.1, micro_freeze_thresh: 14, moment_mult: 1.1,
        dust_style: 0, glitch_style: 0, bloom_shape: 0,
        scanline_amt: 0.3, motion_echo: 0.2, moment_bias: [1.0, 1.0, 1.0, 1.0, 1.0, 1.0],
        sig_param: 0.5 },
    // S950: smooth, warm, flowing — minimal glitch, radial bloom
    VisualProfile { row_damping: 0.94, col_damping: 0.93, bpm_force: 0.25, dust_density: 0.56,
        glitch_mult: 0.35, step_quant_mult: 0.8, smear_base: 0.35, transition_speed: 0.6,
        overlay_speed: 0.9, micro_freeze_thresh: 8, moment_mult: 0.8,
        dust_style: 0, glitch_style: 3, bloom_shape: 2,
        scanline_amt: 0.2, motion_echo: 0.3, moment_bias: [0.8, 0.5, 1.3, 0.7, 0.8, 1.2],
        sig_param: 0.4 },
    // Mirage: chaotic, unstable, warped — chaotic dust, warped glitch, jagged bloom
    VisualProfile { row_damping: 0.85, col_damping: 0.82, bpm_force: 0.35, dust_density: 0.74,
        glitch_mult: 1.65, step_quant_mult: 1.5, smear_base: 0.4, transition_speed: 0.35,
        overlay_speed: 1.4, micro_freeze_thresh: 20, moment_mult: 1.4,
        dust_style: 2, glitch_style: 2, bloom_shape: 3,
        scanline_amt: 0.4, motion_echo: 0.35, moment_bias: [0.8, 1.5, 0.6, 1.8, 1.2, 0.8],
        sig_param: 0.8 },
    // Prophet 2000: elastic, expressive — standard dust, mixed glitch, radial bloom
    VisualProfile { row_damping: 0.93, col_damping: 0.91, bpm_force: 0.30, dust_density: 0.51,
        glitch_mult: 0.45, step_quant_mult: 0.9, smear_base: 0.35, transition_speed: 0.7,
        overlay_speed: 1.0, micro_freeze_thresh: 10, moment_mult: 0.9,
        dust_style: 0, glitch_style: 0, bloom_shape: 2,
        scanline_amt: 0.25, motion_echo: 0.25, moment_bias: [1.0, 1.0, 1.0, 1.0, 1.0, 1.0],
        sig_param: 0.5 },
    // MPC3000: clean, stable, precise — minimal everything
    VisualProfile { row_damping: 0.95, col_damping: 0.94, bpm_force: 0.20, dust_density: 0.46,
        glitch_mult: 0.15, step_quant_mult: 0.7, smear_base: 0.25, transition_speed: 0.5,
        overlay_speed: 0.8, micro_freeze_thresh: 5, moment_mult: 0.6,
        dust_style: 0, glitch_style: 3, bloom_shape: 0,
        scanline_amt: 0.15, motion_echo: 0.1, moment_bias: [0.6, 0.6, 1.5, 0.5, 0.6, 1.0],
        sig_param: 0.6 },
    // SP-303: effect-driven, rhythmic — h-line glitch, standard bloom
    VisualProfile { row_damping: 0.91, col_damping: 0.89, bpm_force: 0.35, dust_density: 0.54,
        glitch_mult: 1.05, step_quant_mult: 1.0, smear_base: 0.30, transition_speed: 0.4,
        overlay_speed: 1.2, micro_freeze_thresh: 15, moment_mult: 1.1,
        dust_style: 0, glitch_style: 1, bloom_shape: 0,
        scanline_amt: 0.5, motion_echo: 0.2, moment_bias: [1.0, 1.2, 0.9, 1.0, 1.0, 1.0],
        sig_param: 0.7 },
];

// ─── Theme ────────────────────────────────────────────────────────────────────

use crate::render::color_system::THEME_COUNT;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Feel { Tight, Expressive, Chaotic }

const FEELS: [Feel; 3] = [Feel::Tight, Feel::Expressive, Feel::Chaotic];

impl Feel {
    fn _name(self) -> &'static str {
        match self { Feel::Tight => "tight", Feel::Expressive => "expressive", Feel::Chaotic => "chaotic" }
    }
}

// ─── V3: Moment System ───────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
enum Moment {
    FreezeCut,
    GlitchBloom,
    LockIn,
    PhaseWave,
    Collapse,
    Afterglow,
    UserAccent,
}

#[derive(Clone, Debug)]
pub(crate) struct MomentState {
    active: Option<Moment>,
    timer: u32,
    duration: u32,
    cooldown: u32,
    /// Seed for deterministic moment effects (set at trigger time)
    seed: u32,
    /// GlitchBloom center cell
    bloom_center: (usize, usize),
}

impl Default for MomentState {
    fn default() -> Self {
        Self { active: None, timer: 0, duration: 0, cooldown: 0, seed: 0, bloom_center: (0, 0) }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct MemoryState {
    heat: f32,
    fatigue: f32,
    afterimage: f32,
}

impl Default for MemoryState {
    fn default() -> Self {
        Self { heat: 0.0, fatigue: 0.0, afterimage: 0.0 }
    }
}

// ─── V4: Anchor points (grid coordinates) ────────────────────────────────────
// Anchors for 54×42 grid
const ANCHOR_CENTER: (f32, f32) = (27.0, 21.0);
const ANCHOR_UPPER: (f32, f32) = (27.0, 10.0);
const ANCHOR_LOWER: (f32, f32) = (27.0, 32.0);
const ANCHOR_LEFT: (f32, f32) = (13.0, 21.0);
const ANCHOR_RIGHT: (f32, f32) = (41.0, 21.0);
const ANCHOR_GOLDEN_TL: (f32, f32) = (20.0, 15.0);
const ANCHOR_GOLDEN_BR: (f32, f32) = (34.0, 27.0);
const ANCHOR_GOLDEN_TR: (f32, f32) = (34.0, 15.0);
const ANCHOR_GOLDEN_BL: (f32, f32) = (20.0, 27.0);

const ANCHORS: [(f32, f32); 9] = [
    ANCHOR_CENTER, ANCHOR_UPPER, ANCHOR_LOWER, ANCHOR_LEFT, ANCHOR_RIGHT,
    ANCHOR_GOLDEN_TL, ANCHOR_GOLDEN_BR, ANCHOR_GOLDEN_TR, ANCHOR_GOLDEN_BL,
];

// Slot roles: mass and drag
const SLOT_MASS: [f32; 4] = [0.5, 0.3, 5.0, 2.0];  // Pulse, Accent, Drift, Ghost
const SLOT_DRAG: [f32; 4] = [0.85, 0.80, 0.97, 0.95];
const SLOT_PULL: [f32; 4] = [0.08, 0.15, 0.005, 0.02];
const CORE_MASS: f32 = 3.0;
const CORE_PULL: f32 = 0.03;

/// V4: Coherent glitch field (FBM-style layered hash noise)
fn glitch_field(col: u32, row: u32, phase: f32) -> f32 {
    let hash_noise = |cx: f32, cy: f32, seed: u32| -> f32 {
        let ix = cx as i32 as u32;
        let iy = cy as i32 as u32;
        let s = ix.wrapping_mul(2246822507)
            .wrapping_add(iy.wrapping_mul(1664525))
            .wrapping_add(seed);
        ((s >> 16) & 0xFF) as f32 / 255.0
    };
    let coarse = hash_noise(col as f32 / 8.0 + phase, row as f32 / 8.0, 111);
    let medium = hash_noise(col as f32 / 4.0 + phase * 1.3, row as f32 / 4.0, 222);
    let fine = hash_noise(col as f32 / 2.0 + phase * 1.7, row as f32 / 2.0, 333);
    (coarse * 0.5 + medium * 0.3 + fine * 0.2).clamp(0.0, 1.0)
}

/// V4: Select image biased toward a preferred density
fn select_biased_image(bank: &crate::ascii_bank::AsciiBank, hash: u32, preferred_density: f32, count: usize) -> usize {
    if count == 0 { return 0; }
    let mut best_idx = (hash as usize) % count;
    let mut best_dist = (bank.images[best_idx].density - preferred_density).abs();
    for k in 1..3u32 {
        let candidate = ((hash.wrapping_mul(k.wrapping_add(1).wrapping_mul(2654435761))) as usize) % count;
        let dist = (bank.images[candidate].density - preferred_density).abs();
        if dist < best_dist {
            best_dist = dist;
            best_idx = candidate;
        }
    }
    best_idx
}

/// V4: Get anchors for a composition mode
fn get_composition_anchors(mode: u8) -> [(f32, f32); 5] {
    // Returns [core, pulse, accent, drift, ghost]
    match mode {
        0 => [ANCHOR_CENTER, ANCHOR_CENTER, ANCHOR_GOLDEN_TL, ANCHOR_UPPER, ANCHOR_CENTER],
        1 => [ANCHOR_GOLDEN_TL, ANCHOR_GOLDEN_BR, ANCHOR_CENTER, ANCHOR_GOLDEN_TR, ANCHOR_GOLDEN_BL],
        2 => [ANCHOR_LEFT, ANCHOR_RIGHT, ANCHOR_CENTER, ANCHOR_UPPER, ANCHOR_LOWER],
        3 => [ANCHOR_CENTER, ANCHOR_CENTER, ANCHOR_CENTER, ANCHOR_CENTER, ANCHOR_CENTER], // orbit mode — offsets added dynamically
        _ => [ANCHOR_CENTER; 5],
    }
}

// ─── Model ────────────────────────────────────────────────────────────────────

#[derive(Lens)]
pub struct EditorData {
    pub params: Arc<SssssssssamplerParams>,
    pub theme_id: usize,
    pub dark_mode: bool,
    pub preset_idx: usize,
    /// Increments on every event — used only as a binding trigger, not for time.
    pub frame_update_counter: usize,
    /// Increments exactly once per UpdateFrameBuffer — used as the animation clock.
    /// Decoupled from event frequency so audio bursts don't cause scroll jumps.
    #[lens(ignore)]
    pub anim_tick: u64,
    #[lens(ignore)]
    pub gui_ctx: Arc<dyn GuiContext>,
    #[lens(ignore)]
    pub anim_params: Arc<Mutex<AnimationParams>>,
    #[lens(ignore)]
    pub frame_buffer: Arc<Mutex<Option<crate::render::FrameBuffer>>>,
    #[lens(ignore)]
    pub ascii_bank: crate::ascii_bank::AsciiBank,
    // ── V2: Per-frame state ──
    #[lens(ignore)]
    pub smoothed_energy: f32,
    #[lens(ignore)]
    pub velocity_row: f32,
    #[lens(ignore)]
    pub velocity_col: f32,
    // ── V3: Temporal quantization state ──
    #[lens(ignore)]
    pub quant_frame: u64,           // frame counter for SR quantization
    #[lens(ignore)]
    pub prev_row_scroll: f32,       // previous positions for smearing
    #[lens(ignore)]
    pub prev_col_drift: f32,
    #[lens(ignore)]
    pub prev_overlay_rows: [f32; 4], // per-slot previous row positions
    #[lens(ignore)]
    pub prev_overlay_cols: [f32; 4],
    // ── V3: Moment & Memory state ──
    #[lens(ignore)]
    pub moment: MomentState,
    #[lens(ignore)]
    pub memory: MemoryState,
    #[lens(ignore)]
    pub micro_freeze_frames: u32,
    #[lens(ignore)]
    pub prev_energy_state: u8,      // for detecting PEAK enter/exit
    #[lens(ignore)]
    pub prev_filter: f32,           // for detecting rapid param changes
    #[lens(ignore)]
    pub prev_sr: f32,
    #[lens(ignore)]
    pub glitch_events_this_frame: u32,
    #[lens(ignore)]
    pub ui_expanded: bool,
    #[lens(ignore)]
    pub shared_ui_expanded: Arc<Mutex<bool>>,
    // ── V4: Phrase system ──
    #[lens(ignore)]
    pub phrase_bar_counter: f32,
    #[lens(ignore)]
    pub phrase_phase: f32,
    #[lens(ignore)]
    pub phrase_length_bars: f32,
    #[lens(ignore)]
    pub bpm_stable_bars: f32,
    #[lens(ignore)]
    pub prev_bpm: f32,
    // ── V4: Intent model ──
    #[lens(ignore)]
    pub intent_tension: f32,
    #[lens(ignore)]
    pub intent_release: f32,
    #[lens(ignore)]
    pub intent_chaos: f32,
    #[lens(ignore)]
    pub prev_energy_trend: f32,
    #[lens(ignore)]
    pub recent_moment_count: u32,
    #[lens(ignore)]
    pub recent_moment_decay_tick: u64,
    #[lens(ignore)]
    pub moment_recovery_timer: u32,
    // ── V4: Anchor/Composition ──
    #[lens(ignore)]
    pub composition_mode: u8,
    #[lens(ignore)]
    pub core_anchor: (f32, f32),
    #[lens(ignore)]
    pub core_pos: (f32, f32),
    #[lens(ignore)]
    pub overlay_anchors: [(f32, f32); 4],
    #[lens(ignore)]
    pub overlay_positions: [(f32, f32); 4],
    #[lens(ignore)]
    pub prev_core_anchor: (f32, f32),
    // ── V4: Overlay physics ──
    #[lens(ignore)]
    pub overlay_velocity_rows: [f32; 4],
    #[lens(ignore)]
    pub overlay_velocity_cols: [f32; 4],
    #[lens(ignore)]
    pub accent_slot_alpha: f32,
    // ── V4: Glitch field ──
    #[lens(ignore)]
    pub glitch_field_phase: f32,
    // ── V5: Signal-adaptive behavior scaling ──
    #[lens(ignore)]
    pub motion_scale: f32,
    #[lens(ignore)]
    pub glitch_scale: f32,
    #[lens(ignore)]
    pub smear_scale: f32,
    #[lens(ignore)]
    pub moment_prob_scale: f32,
    // ── V5: Feel preset ──
    #[lens(ignore)]
    pub feel: Feel,
    // ── V5: Visual profile (interpolated per-preset) ──
    #[lens(ignore)]
    pub visual_profile: VisualProfile,
    // ── V6: Phrase breath ──
    #[lens(ignore)]
    pub phrase_variant: u8,
    #[lens(ignore)]
    pub phrase_breath_t: f32,
    // ── V6: Motion echo ──
    #[lens(ignore)]
    pub motion_history: [(f32, f32); 4],
    // ── V6: Drop detection ──
    #[lens(ignore)]
    pub drop_detected: bool,
    #[lens(ignore)]
    pub drop_timer: u32,
    // V6: first load — 33% chance to show image #1 centered
    #[lens(ignore)]
    pub first_load_img1: bool,
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
}

#[derive(Debug, Clone)]
pub enum EditorEvent {
    PrevPreset,
    NextPreset,
    SelectPreset(usize),
    SelectTheme(usize),
    UpdateFrameBuffer,
    CycleTheme,
    ToggleMode,
    CycleFeel,
    ToggleUiExpand,
    Tick,
}

impl Model for EditorData {
    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        // Increment counter on EVERY event (makes binding trigger continuously)
        self.frame_update_counter = self.frame_update_counter.wrapping_add(1);

        // Broadcast Tick to entire view tree so AsciiImageDisplay receives it
        // and can call needs_redraw() to keep the canvas painting continuously.
        if self.frame_update_counter % 2 == 0 {
            cx.emit_custom(
                Event::new(EditorEvent::Tick)
                    .target(Entity::root())
                    .propagate(Propagation::Subtree),
            );
        }

        event.map(|e: &EditorEvent, _| match e {
            EditorEvent::ToggleUiExpand => {
                self.ui_expanded = !self.ui_expanded;
                if let Ok(mut e) = self.shared_ui_expanded.lock() { *e = self.ui_expanded; }
            }
            EditorEvent::CycleTheme => {
                self.theme_id = (self.theme_id + 1) % THEME_COUNT;
            }
            EditorEvent::ToggleMode => {
                self.dark_mode = !self.dark_mode;
            }
            EditorEvent::CycleFeel => {
                let idx = FEELS.iter().position(|f| *f == self.feel).unwrap_or(1);
                self.feel = FEELS[(idx + 1) % FEELS.len()];
            }
            EditorEvent::PrevPreset => {
                if self.preset_idx > 0 {
                    self.preset_idx -= 1;
                    self.apply_preset();
                }
            }
            EditorEvent::NextPreset => {
                if self.preset_idx + 1 < PRESETS.len() {
                    self.preset_idx += 1;
                    self.apply_preset();
                }
            }
            EditorEvent::SelectPreset(idx) => {
                if *idx < PRESETS.len() {
                    self.preset_idx = *idx;
                    self.apply_preset();
                }
            }
            EditorEvent::SelectTheme(idx) => {
                if *idx < THEME_COUNT {
                    self.theme_id = *idx;
                }
            }
            EditorEvent::Tick => {
                // Trigger frame buffer update on every tick (continuous animation)
                // This makes the display feel alive even without audio
            }
            EditorEvent::UpdateFrameBuffer => {
                if let Ok(anim_params) = self.anim_params.lock() {
                    const COLS: u32 = 54;
                    const ROWS: u32 = 42;
                    const BASE_MARGIN: usize = 1;
                    // Images stored at native resolution — get_cell returns 0 for out-of-bounds
                    // Use COLS/ROWS as display grid, native image coords for lookups

                    let mut frame_buffer = crate::render::FrameBuffer::new(COLS, ROWS);

                    let playing = anim_params.playing;
                    let raw_energy = anim_params.energy;
                    let transient = anim_params.transient;
                    let motion_speed = anim_params.motion_speed;
                    // V6: stereo + sub-bass
                    let sub_bass = anim_params.sub_bass_energy;
                    let lr_balance = anim_params.lr_balance;
                    let stereo_width = anim_params.stereo_width;
                    let anim_bpm = anim_params.bpm;

                    // ── V5: Signal-adaptive behavior scaling ──
                    // Feel presets (Expressive = identity):
                    //   Tight:      moment*0.7, motion*0.8, glitch*0.6, smear*0.8
                    //   Expressive: all × 1.0
                    //   Chaotic:    moment*1.5, motion*1.3, glitch*1.5, smear*0.5
                    let signal_class = anim_params.signal_class;
                    let (sc_motion, sc_glitch, sc_smear, sc_moment) = match signal_class {
                        0 => (1.2, 1.3, 0.7, 1.2),   // Percussive
                        2 => (0.7, 0.5, 1.4, 0.6),   // Ambient
                        _ => (1.0, 0.8, 1.1, 0.9),   // Tonal
                    };
                    // Signal-class transition sharpness for overlay dissolves
                    let transition_sharpness = match signal_class {
                        0 => 4.0,   // percussive: hard cuts
                        2 => 0.5,   // ambient: soft ghostly merge
                        _ => 1.0,   // tonal: normal
                    };
                    let (fm, fg, fs, fmo) = match self.feel {
                        Feel::Tight =>      (0.8, 0.6, 0.8, 0.7),
                        Feel::Expressive => (1.0, 1.0, 1.0, 1.0),
                        Feel::Chaotic =>    (1.3, 1.5, 0.5, 1.5),
                    };
                    self.motion_scale += (sc_motion * fm - self.motion_scale) * 0.05;
                    self.glitch_scale += (sc_glitch * fg - self.glitch_scale) * 0.05;
                    self.smear_scale += (sc_smear * fs - self.smear_scale) * 0.05;
                    self.moment_prob_scale += (sc_moment * fmo - self.moment_prob_scale) * 0.05;

                    // ── V5: Visual profile interpolation (~300ms transition) ──
                    {
                        let vp_target = &VISUAL_PROFILES[self.preset_idx.min(VISUAL_PROFILES.len() - 1)];
                        let vr = 0.04;
                        let vp = &mut self.visual_profile;
                        vp.row_damping += (vp_target.row_damping - vp.row_damping) * vr;
                        vp.col_damping += (vp_target.col_damping - vp.col_damping) * vr;
                        vp.bpm_force += (vp_target.bpm_force - vp.bpm_force) * vr;
                        vp.dust_density += (vp_target.dust_density - vp.dust_density) * vr;
                        vp.glitch_mult += (vp_target.glitch_mult - vp.glitch_mult) * vr;
                        vp.step_quant_mult += (vp_target.step_quant_mult - vp.step_quant_mult) * vr;
                        vp.smear_base += (vp_target.smear_base - vp.smear_base) * vr;
                        vp.transition_speed += (vp_target.transition_speed - vp.transition_speed) * vr;
                        vp.overlay_speed += (vp_target.overlay_speed - vp.overlay_speed) * vr;
                        vp.moment_mult += (vp_target.moment_mult - vp.moment_mult) * vr;
                        vp.micro_freeze_thresh = vp_target.micro_freeze_thresh; // discrete
                        vp.dust_style = vp_target.dust_style;
                        vp.glitch_style = vp_target.glitch_style;
                        vp.bloom_shape = vp_target.bloom_shape;
                        // V6: interpolate new fields
                        vp.scanline_amt += (vp_target.scanline_amt - vp.scanline_amt) * vr;
                        vp.motion_echo += (vp_target.motion_echo - vp.motion_echo) * vr;
                        for i in 0..6 { vp.moment_bias[i] += (vp_target.moment_bias[i] - vp.moment_bias[i]) * vr; }
                        vp.sig_param += (vp_target.sig_param - vp.sig_param) * vr;
                        // Global polish: nudge damping toward 1.0 (+5% restraint)
                        vp.row_damping = vp.row_damping * 0.98 + 0.02;
                        vp.col_damping = vp.col_damping * 0.98 + 0.02;
                    }

                    // ── V2: Smooth energy (exponential moving average) ──
                    let smooth_rate = 0.08; // lower = slower response
                    self.smoothed_energy += (raw_energy - self.smoothed_energy) * smooth_rate;
                    let energy = self.smoothed_energy;

                    // ── V2: Visual state from energy (with hysteresis) ──
                    // IDLE=0, FLOW=1, BUILD=2, PEAK=3
                    let visual_state = match self.prev_energy_state {
                        0 => if energy >= 0.22 { 1u8 } else { 0 },
                        1 => if energy < 0.18 { 0 } else if energy >= 0.55 { 2 } else { 1 },
                        2 => if energy < 0.50 { 1 } else if energy >= 0.82 { 3 } else { 2 },
                        3 => if energy < 0.75 { 2 } else { 3 },
                        _ => 0,
                    };

                    // ── Read DSP params ──
                    // Visual filter remap: 0-0.3 dramatic, 0.3-0.7 sweet spot, 0.7-1.0 subtle
                    let filter_raw = self.params.filter_cutoff.value();
                    let filter_val = if filter_raw < 0.3 {
                        filter_raw * 0.5 / 0.3
                    } else if filter_raw < 0.7 {
                        0.5 + (filter_raw - 0.3) * 0.35 / 0.4
                    } else {
                        0.85 + (filter_raw - 0.7) * 0.15 / 0.3
                    };
                    let mix_val    = self.params.mix.value();
                    let bit_depth  = self.params.bit_depth.value();
                    let jitter_val = self.params.jitter.value();
                    let aa_enabled = self.params.anti_alias.value();

                    // ── V2: Filter → structural visibility (probabilistic reveal) ──
                    // filter_val is used as a per-cell threshold, not just alpha
                    let base_alpha = filter_val.clamp(0.0, 1.0);

                    // ── V2: Mix → overlay aggression ──
                    let mix = mix_val.clamp(0.0, 1.0);
                    let overlay_visibility = mix * 0.80;
                    // Overlay density: mix controls how many cells show
                    let overlay_density_threshold = 0.02 + mix * 0.98; // 2%→100% (phrase mod applied later)
                    // Overlay scroll rate scales with mix + energy
                    let overlay_speed_mult = (1.0 + mix * 0.5 + energy * 0.5) * self.visual_profile.overlay_speed;

                    // ── V2: Tiered corruption from bit depth ──
                    // 16-12: none. 11-9: point glitch. 8-6: cluster. 5-4: structural.
                    let corruption_tier = if bit_depth >= 12.0 { 0u8 }
                        else if bit_depth >= 9.0 { 1 }  // point
                        else if bit_depth >= 6.0 { 2 }  // cluster
                        else { 3 };                      // structural
                    // Probability scales with energy in BUILD/PEAK states
                    let fatigue_mult = (1.0 - self.memory.fatigue).clamp(0.2, 1.0);
                    let recovery_glitch_mult = if self.moment_recovery_timer > 0 { 0.3 } else { 1.0 };
                    let glitch_prob = match corruption_tier {
                        0 => 0.0,
                        1 => 0.0015 * fatigue_mult * recovery_glitch_mult,
                        2 => (0.004 + energy * 0.008) * fatigue_mult * recovery_glitch_mult,
                        _ => (0.008 + energy * 0.016) * fatigue_mult * recovery_glitch_mult,
                    } * self.glitch_scale * self.visual_profile.glitch_mult;

                    let palette = ColorPalette::from_id_and_mode(self.theme_id, self.dark_mode);

                    let to_u8_fn = |v: f32| (v.powf(1.0 / 2.2) * 255.0) as u8;
                    frame_buffer.bg_rgb = [
                        to_u8_fn(palette.background.r),
                        to_u8_fn(palette.background.g),
                        to_u8_fn(palette.background.b),
                    ];
                    frame_buffer.primary_rgb = [
                        to_u8_fn(palette.primary.r),
                        to_u8_fn(palette.primary.g),
                        to_u8_fn(palette.primary.b),
                    ];
                    frame_buffer.emphasis_rgb = [
                        to_u8_fn(palette.emphasis.r),
                        to_u8_fn(palette.emphasis.g),
                        to_u8_fn(palette.emphasis.b),
                    ];
                    frame_buffer.preset_idx = self.preset_idx as u8;
                    frame_buffer.theme_idx = self.theme_id as u8;
                    frame_buffer.dark_mode = self.dark_mode;
                    frame_buffer.is_light = palette.is_light;
                    frame_buffer.feel_idx = FEELS.iter().position(|f| *f == self.feel).unwrap_or(1) as u8;
                    let is_light = palette.is_light;

                    // ── Clocks ──
                    // dust_tick: always advances (dust never pauses)
                    let dust_tick = self.frame_update_counter as u32;
                    self.quant_frame = self.quant_frame.wrapping_add(1);

                    if playing {
                        self.anim_tick = self.anim_tick.wrapping_add(1);
                    }
                    let t = self.anim_tick as f32;

                    // ── PHASE 1: Sample Rate → Temporal Quantization ──
                    let target_sr = self.params.target_sr.value(); // 1000–96000 Hz
                    // V6: Custom SR effect curve
                    // 96k-48k → 0-20% effect, 48k-38k → 20-40%, 38k-1k → 40-100%
                    let sr_effect = if target_sr >= 48_000.0 {
                        let t = ((96_000.0 - target_sr) / 48_000.0).clamp(0.0, 1.0);
                        t * 0.20
                    } else if target_sr >= 38_000.0 {
                        let t = ((48_000.0 - target_sr) / 10_000.0).clamp(0.0, 1.0);
                        0.20 + t * 0.20
                    } else {
                        let t = ((38_000.0 - target_sr) / 37_000.0).clamp(0.0, 1.0);
                        0.40 + t * 0.60
                    };
                    let sr_norm = 1.0 - sr_effect; // invert: sr_norm=1 means no effect
                    // step_interval: 1 frame at max SR → 8 frames at min SR
                    let step_interval = ((1.0 + sr_effect * 7.0) * self.visual_profile.step_quant_mult).max(1.0) as u64;
                    let should_update = step_interval == 0 || self.quant_frame % step_interval == 0;

                    // ── PHASE 2: Temporal Smearing (low SR only) ──
                    let smear_factor = sr_effect * self.visual_profile.smear_base;
                    // V3: Effective smear includes afterglow + memory afterimage
                    let afterglow_smear_early = if matches!(self.moment.active, Some(Moment::Afterglow)) { 0.5 } else { 0.0 };
                    let effective_smear = (smear_factor * self.smear_scale + afterglow_smear_early + self.memory.afterimage * 0.15).min(0.8);

                    // ── BPM timing ──
                    let bpm = anim_params.bpm.clamp(40.0, 200.0);
                    let ticks_per_beat = 60.0 * 60.0 / bpm;
                    let ticks_per_bar = ticks_per_beat * 4.0;

                    // ── V4: Phrase system ──
                    if playing {
                        let bars_this_frame = 1.0 / ticks_per_bar.max(1.0);
                        let bpm_delta = (bpm - self.prev_bpm).abs() / bpm.max(1.0);
                        if bpm_delta < 0.02 {
                            self.bpm_stable_bars += bars_this_frame;
                        } else {
                            self.bpm_stable_bars = 0.0;
                        }
                        self.prev_bpm = bpm;
                        self.phrase_length_bars = if self.bpm_stable_bars > 4.0 { 16.0 } else { 8.0 };
                        self.phrase_bar_counter += bars_this_frame;
                        if self.phrase_bar_counter >= self.phrase_length_bars {
                            self.phrase_bar_counter -= self.phrase_length_bars;
                            // Phrase boundary — recompose
                            let comp_hash = (self.anim_tick as u32).wrapping_mul(48271);
                            self.composition_mode = (comp_hash % 4) as u8;
                            // V6: randomized phrase breath variant
                            self.phrase_variant = ((comp_hash >> 10) % 6) as u8;
                            let anchors = get_composition_anchors(self.composition_mode);
                            self.prev_core_anchor = self.core_anchor;
                            self.core_anchor = anchors[0];
                            for i in 0..4 { self.overlay_anchors[i] = anchors[1 + i]; }
                        }
                    }
                    self.phrase_phase = self.phrase_bar_counter / self.phrase_length_bars.max(1.0);
                    let phrase_arc = if self.phrase_phase < 0.5 {
                        self.phrase_phase * 2.0
                    } else if self.phrase_phase < 0.75 {
                        1.0
                    } else {
                        1.0 - (self.phrase_phase - 0.75) * 4.0
                    };
                    // ── V6: Phrase breath (randomized 1/2/4 beat dips) ──
                    let breath_beats = match self.phrase_variant {
                        0 | 3 => 1.0f32,  // micro
                        1 | 4 => 2.0,     // subtle
                        _     => 4.0,     // full
                    };
                    let breath_window = ticks_per_beat * breath_beats;
                    let ticks_into_phrase = self.phrase_bar_counter * ticks_per_bar / self.phrase_length_bars.max(1.0);
                    let ticks_remaining = (self.phrase_length_bars * ticks_per_bar) - ticks_into_phrase;
                    let phrase_breath = if ticks_remaining < breath_window {
                        // Exit breath: fade out
                        (ticks_remaining / breath_window).clamp(0.0, 1.0)
                    } else if ticks_into_phrase < breath_window {
                        // Entry breath: fade in
                        (ticks_into_phrase / breath_window).clamp(0.0, 1.0)
                    } else {
                        1.0
                    };
                    self.phrase_breath_t += (phrase_breath - self.phrase_breath_t) * 0.15;
                    let phrase_overlay_mod = (0.7 + phrase_arc * 0.3) * self.phrase_breath_t;
                    let phrase_brightness_mod = (0.9 + phrase_arc * 0.1) * (0.7 + self.phrase_breath_t * 0.3);
                    let phrase_moment_mod = 0.5 + phrase_arc * 0.5;

                    let img_count = self.ascii_bank.len().max(1);

                    // ── Core image cycling: every 2 bars, randomized order ──
                    let core_cycle_len = ticks_per_bar * 2.0;
                    let core_cycle = (t / core_cycle_len) as usize;
                    let core_hash = (core_cycle as u32).wrapping_mul(2654435761);
                    let preferred_density = if energy < 0.3 { 0.15 } else if energy > 0.7 { 0.6 } else { 0.35 };

                    // V6: first load override — 33% chance to show image #1 (index 0) centered
                    let (core_img, prev_core_img) = if self.first_load_img1 && core_cycle < 2 {
                        (0usize, 0usize) // force image #1
                    } else {
                        if self.first_load_img1 { self.first_load_img1 = false; }
                        let ci = select_biased_image(&self.ascii_bank, core_hash, preferred_density, img_count);
                        let ph = (core_cycle.wrapping_sub(1) as u32).wrapping_mul(2654435761);
                        (ci, (ph as usize) % img_count)
                    };

                    // Random position offset for small core images
                    let core_w = if core_img < img_count { self.ascii_bank.images[core_img].grid.width as i32 } else { COLS as i32 };
                    let core_h = if core_img < img_count { self.ascii_bank.images[core_img].grid.height as i32 } else { ROWS as i32 };
                    // V4: Anchor-based core positioning
                    self.core_pos.0 += (self.core_anchor.0 - self.core_pos.0) * CORE_PULL;
                    self.core_pos.1 += (self.core_anchor.1 - self.core_pos.1) * CORE_PULL;
                    // Orbital offset
                    let core_orbital_c = (t * 0.003).sin() * 2.0 + lr_balance * 2.0; // V6: stereo drift (tighter)
                    let core_orbital_r = (t * 0.002).cos() * 1.5;
                    // Safe framing with 5% chance to break the rules
                    // V6: force centered on first load
                    let (raw_col_off, raw_row_off) = if self.first_load_img1 {
                        // Center: offset so image middle aligns with grid middle
                        ((core_w - COLS as i32) / 2, (core_h - ROWS as i32) / 2)
                    } else {
                        ((self.core_pos.0 + core_orbital_c) as i32 - COLS as i32 / 2 + core_w / 2,
                         (self.core_pos.1 + core_orbital_r) as i32 - ROWS as i32 / 2 + core_h / 2)
                    };
                    let rebel_roll = ((core_cycle as u32).wrapping_mul(2654435761) >> 24) as f32 / 255.0;
                    let (core_col_off, core_row_off) = if rebel_roll < 0.05 {
                        // 5% chance: no clamping — raw offset, image can drift anywhere
                        (raw_col_off, raw_row_off)
                    } else if core_w >= COLS as i32 && core_h >= ROWS as i32 {
                        // Large image: clamp to always fill viewport
                        (raw_col_off.clamp(0, (core_w - COLS as i32).max(0)),
                         raw_row_off.clamp(0, (core_h - ROWS as i32).max(0)))
                    } else {
                        // Small image: keep 80% visible
                        let min_vis_c = (core_w * 80 / 100).max(1);
                        let min_vis_r = (core_h * 80 / 100).max(1);
                        (raw_col_off.clamp(-(core_w - min_vis_c), COLS as i32 - min_vis_c),
                         raw_row_off.clamp(-(core_h - min_vis_r), ROWS as i32 - min_vis_r))
                    };

                    // ── Transition with directional wave bias ──
                    let time_in_cycle = t % core_cycle_len;
                    let transition_window = ticks_per_bar * self.visual_profile.transition_speed;
                    let transition_progress = (time_in_cycle / transition_window).min(1.0);
                    let in_transition = transition_progress < 1.0;

                    // V3: Freeze state (from previous frame's moment)
                    let is_frozen = matches!(self.moment.active, Some(Moment::FreezeCut))
                        || self.micro_freeze_frames > 0;

                    // ── Velocity-based scroll — only update on quantized frames ──
                    let bpm_force = (t / ticks_per_bar * std::f32::consts::TAU).sin() * self.visual_profile.bpm_force;
                    let energy_force = (if transient { energy * 2.0 } else { energy * 0.1 }) * self.motion_scale;
                    if should_update && !is_frozen {
                        let core_accel = (bpm_force + energy_force * motion_speed) / CORE_MASS;
                        self.velocity_row += core_accel;
                        self.velocity_row *= self.visual_profile.row_damping;
                        self.velocity_row = self.velocity_row.clamp(-8.0, 8.0);
                        let col_force = (t / (ticks_per_bar * 2.0) * std::f32::consts::TAU).cos() * 0.15;
                        self.velocity_col += (col_force + energy_force * 0.2) / CORE_MASS;
                        self.velocity_col *= self.visual_profile.col_damping;
                        self.velocity_col = self.velocity_col.clamp(-4.0, 4.0);
                    }
                    // Smeared positions: lerp between previous and current
                    let new_row_scroll_f = (self.velocity_row.abs() * 2.0) % ROWS as usize as f32;
                    let new_col_drift_f = self.velocity_col;
                    self.prev_row_scroll += (new_row_scroll_f - self.prev_row_scroll) * (1.0 - effective_smear);
                    self.prev_col_drift += (new_col_drift_f - self.prev_col_drift) * (1.0 - effective_smear);
                    let row_scroll = self.prev_row_scroll as usize;
                    let col_drift = self.prev_col_drift as i32;

                    // V6: Update motion history ring buffer for echo trails
                    self.motion_history[3] = self.motion_history[2];
                    self.motion_history[2] = self.motion_history[1];
                    self.motion_history[1] = self.motion_history[0];
                    self.motion_history[0] = (self.prev_row_scroll, self.prev_col_drift);

                    // ── V2: Structural distortion (tier 3 corruption) ──
                    // At very low bit depth, rows/cols shift by ±1–2
                    let structural_offset = if corruption_tier >= 3 {
                        let s = ((t * 0.07).sin() * 2.0) as i32;
                        s.clamp(-2, 2)
                    } else { 0 };

                    // ── Overlay slots: 4, 6, 8 bar cycles ──
                    const NUM_SLOTS: usize = 4;
                    const SLOT_BARS: [f32; NUM_SLOTS] = [1.0, 1.5, 6.0, 3.0];
                    struct OverlaySlot {
                        img_idx: usize,
                        prev_img_idx: usize,
                        transition_t: f32,
                        alpha: f32,
                        row_shift: usize,
                        col_shift: i32,
                        img_row_offset: i32,
                        img_col_offset: i32,
                        color_idx: usize,
                        depth: u8, // V6: 0=front(Pulse), 1=Accent, 2=Drift, 3=Ghost(back)
                    }

                    // V4: Update accent slot alpha
                    if transient { self.accent_slot_alpha = energy.max(self.accent_slot_alpha); }
                    self.accent_slot_alpha *= 0.85;
                    let accent_alpha = self.accent_slot_alpha;
                    let afterimage_val = self.memory.afterimage;

                    // V4: Update overlay positions toward anchors with role-specific pull
                    // Accent retargets on transient
                    if transient {
                        let accent_hash = (self.anim_tick as u32).wrapping_mul(1664525);
                        self.overlay_anchors[1] = ANCHORS[(accent_hash as usize) % ANCHORS.len()];
                    }
                    // Ghost follows previous core anchor
                    self.overlay_anchors[3] = self.prev_core_anchor;

                    // Orbit mode: add sine/cosine offsets
                    let orbit_offsets: [(f32, f32); 4] = if self.composition_mode == 3 {
                        std::array::from_fn(|i| {
                            let phase = i as f32 * std::f32::consts::FRAC_PI_2;
                            let radius = 8.0 + i as f32 * 3.0;
                            ((t * 0.004 + phase).sin() * radius,
                             (t * 0.003 + phase).cos() * radius)
                        })
                    } else {
                        [(0.0, 0.0); 4]
                    };

                    let mut overlay_col_offsets = [0i32; 4];
                    let mut overlay_row_offsets = [0i32; 4];
                    for i in 0..4 {
                        // Pull toward anchor
                        self.overlay_positions[i].0 += (self.overlay_anchors[i].0 - self.overlay_positions[i].0) * SLOT_PULL[i];
                        self.overlay_positions[i].1 += (self.overlay_anchors[i].1 - self.overlay_positions[i].1) * SLOT_PULL[i];

                        // Physics: apply velocity as orbital force
                        if should_update {
                            let force = (t * 0.005 + i as f32 * 1.7).sin() * 0.5;
                            self.overlay_velocity_rows[i] += force / SLOT_MASS[i];
                            self.overlay_velocity_rows[i] *= SLOT_DRAG[i];
                            self.overlay_velocity_rows[i] = self.overlay_velocity_rows[i].clamp(-4.0, 4.0);
                            let cforce = (t * 0.008 + i as f32 * 2.1).sin() * 0.5;
                            self.overlay_velocity_cols[i] += cforce / SLOT_MASS[i];
                            self.overlay_velocity_cols[i] *= SLOT_DRAG[i];
                            self.overlay_velocity_cols[i] = self.overlay_velocity_cols[i].clamp(-4.0, 4.0);
                        }

                        let final_c = self.overlay_positions[i].0 + orbit_offsets[i].0 + self.overlay_velocity_cols[i];
                        let final_r = self.overlay_positions[i].1 + orbit_offsets[i].1 + self.overlay_velocity_rows[i];
                        // Safe framing: overlay >= 50% visible (tighter framing)
                        // V6: stereo width spreads overlays L/R (even slots left, odd right)
                        let stereo_spread = if i % 2 == 0 { -stereo_width * 2.0 } else { stereo_width * 2.0 };
                        let max_off = COLS as i32 / 3; // was /2 — tighter
                        let max_off_r = ROWS as i32 / 3;
                        overlay_col_offsets[i] = ((final_c + stereo_spread) as i32 - COLS as i32 / 2).clamp(-max_off, max_off);
                        overlay_row_offsets[i] = (final_r as i32 - ROWS as i32 / 2).clamp(-max_off_r, max_off_r);
                    }

                    // V4: Soft collision avoidance between overlays
                    for i in 0..4 {
                        for j in (i+1)..4 {
                            let dx = overlay_col_offsets[i] as f32 - overlay_col_offsets[j] as f32;
                            let dy = overlay_row_offsets[i] as f32 - overlay_row_offsets[j] as f32;
                            let dist = (dx * dx + dy * dy).sqrt().max(0.1);
                            if dist < 8.0 {
                                let repulsion = (8.0 - dist) * 0.3;
                                let nx = dx / dist;
                                let ny = dy / dist;
                                overlay_col_offsets[i] = (overlay_col_offsets[i] as f32 + nx * repulsion) as i32;
                                overlay_row_offsets[i] = (overlay_row_offsets[i] as f32 + ny * repulsion) as i32;
                                overlay_col_offsets[j] = (overlay_col_offsets[j] as f32 - nx * repulsion) as i32;
                                overlay_row_offsets[j] = (overlay_row_offsets[j] as f32 - ny * repulsion) as i32;
                            }
                        }
                    }

                    // V3: LockIn — check if moment is active (from previous frame)
                    let lockin_active = matches!(self.moment.active, Some(Moment::LockIn));
                    let overlay_recovery = self.moment_recovery_timer > 0;

                    let slots: [OverlaySlot; NUM_SLOTS] = std::array::from_fn(|i| {
                        let slot_period = ticks_per_bar * SLOT_BARS[i];
                        let phase_offset = i as f32 * (slot_period / NUM_SLOTS as f32);
                        let sin_val = ((t + phase_offset) / slot_period * std::f32::consts::TAU).sin();

                        // V4: Role-based alpha
                        let filter_boost = 1.0 - filter_val * 0.5;
                        let raw_alpha = match i {
                            0 => {
                                // Pulse: beat-synced square wave
                                let beat_phase = (t / ticks_per_beat) % 1.0;
                                let pulse = if beat_phase < 0.5 { 0.8 } else { 0.3 };
                                pulse * overlay_visibility * filter_boost * phrase_overlay_mod
                            }
                            1 => {
                                // Accent: transient-reactive
                                (accent_alpha * overlay_visibility * filter_boost).min(1.0)
                            }
                            2 => {
                                // Drift: slow ambient sine
                                let drift_alpha = (sin_val * 0.5 + 0.5) * 0.5;
                                drift_alpha * overlay_visibility * filter_boost * phrase_overlay_mod
                            }
                            3 => {
                                // Ghost: afterimage-driven
                                let ghost_alpha = afterimage_val * 0.7;
                                ghost_alpha.max(0.15) * overlay_visibility * filter_boost
                            }
                            _ => 0.0,
                        };

                        let cycle = ((t + phase_offset) / slot_period) as usize;
                        let prev_cycle = if cycle > 0 { cycle - 1 } else { 0 };
                        // Hash for random image order per slot
                        let img_hash = (cycle as u32).wrapping_mul(2654435761)
                            .wrapping_add(i as u32 * 1013904223);
                        let mut img_idx = if lockin_active { core_img } else { (img_hash as usize) % img_count };
                        if !lockin_active && img_idx == core_img { img_idx = (img_idx + 1) % img_count; }
                        let prev_img_hash = (prev_cycle as u32).wrapping_mul(2654435761)
                            .wrapping_add(i as u32 * 1013904223);
                        let mut prev_img_idx = (prev_img_hash as usize) % img_count;
                        if prev_img_idx == core_img { prev_img_idx = (prev_img_idx + 1) % img_count; }

                        // Overlay scatter-dissolve: transition over first 1/4 of slot period
                        let time_in_slot = (t + phase_offset) % slot_period;
                        let ov_transition_window = slot_period * 0.25;
                        let transition_t = (time_in_slot / ov_transition_window).min(1.0);

                        // Overlay position: small drift around center, not large row jumps
                        let hash = (cycle as u32).wrapping_mul(2654435761).wrapping_add(i as u32 * 1013904223);
                        let row_drift = if should_update {
                            (t * 0.005 * overlay_speed_mult + i as f32 * 1.7).sin() * 3.0
                        } else { 0.0 };
                        let col_drift_ov = {
                            let base_cs = ((hash >> 16) % 9) as f32 - 4.0;
                            let sd = (t * 0.008 * overlay_speed_mult + i as f32 * 2.1).sin() * 3.0;
                            (base_cs + sd).clamp(-6.0, 6.0)
                        };
                        let row_shift = row_drift.max(0.0) as usize;
                        let col_shift = col_drift_ov as i32;

                        // V4: Anchor-based overlay positioning
                        let img_col_offset = overlay_col_offsets[i];
                        let img_row_offset = overlay_row_offsets[i];

                        OverlaySlot {
                            img_idx,
                            prev_img_idx,
                            transition_t,
                            alpha: if overlay_recovery { raw_alpha * 0.7 } else { raw_alpha },
                            row_shift,
                            col_shift,
                            img_row_offset,
                            img_col_offset,
                            color_idx: i % 4,
                            depth: i as u8,
                        }
                    });

                    // Apply smearing to overlay positions after slot creation
                    for i in 0..NUM_SLOTS {
                        let new_r = slots[i].row_shift as f32;
                        let new_c = slots[i].col_shift as f32;
                        self.prev_overlay_rows[i] += (new_r - self.prev_overlay_rows[i]) * (1.0 - smear_factor);
                        self.prev_overlay_cols[i] += (new_c - self.prev_overlay_cols[i]) * (1.0 - smear_factor);
                    }

                    // ── V2: Dust density couples to energy + transient bursts ──
                    let base_dust = self.visual_profile.dust_density * 0.88; // V6: 12% less dust
                    let dust_density = base_dust + energy * 0.17;
                    let dust_density = if transient { (dust_density + 0.20).min(0.90) } else { dust_density };

                    // ── V2: Color temporal drift (subtle per-layer phase offset) ──
                    let color_drift = (t * 0.001).sin() * (0.02 + phrase_arc * 0.06);

                    // ── V3: Memory system ──
                    self.memory.heat += (energy - self.memory.heat) * 0.05;
                    self.memory.afterimage += (energy - self.memory.afterimage) * 0.1;
                    self.memory.fatigue *= 0.98;

                    // ── V3: Moment system — trigger + tick ──
                    if self.moment.cooldown > 0 {
                        self.moment.cooldown -= 1;
                    }
                    // Tick active moment
                    if self.moment.active.is_some() {
                        self.moment.timer += 1;
                        if self.moment.timer >= self.moment.duration {
                            // Moment ended — per-moment cooldown, check for Afterglow
                            let was = self.moment.active.take();
                            let base_cd = match was {
                                Some(Moment::FreezeCut) => 55,
                                Some(Moment::GlitchBloom) => 70,
                                Some(Moment::LockIn) => (ticks_per_bar * 3.0) as u32,
                                Some(Moment::PhaseWave) => (ticks_per_bar * 2.5) as u32,
                                Some(Moment::Collapse) => 150,
                                Some(Moment::UserAccent) => 12,
                                _ => 30,
                            };
                            self.moment.cooldown = base_cd;
                            self.moment_recovery_timer = 90; // ~1.5s recovery bias
                            // Afterglow triggers after certain moments
                            if matches!(was, Some(Moment::FreezeCut) | Some(Moment::GlitchBloom)) {
                                self.moment.active = Some(Moment::Afterglow);
                                self.moment.timer = 0;
                                self.moment.duration = 45;
                                self.moment.cooldown = 0;
                            }
                        }
                    }
                    // V4: Intent model
                    let energy_derivative = energy - self.prev_energy_trend;
                    self.prev_energy_trend += (energy_derivative - self.prev_energy_trend) * 0.1;
                    if self.anim_tick.wrapping_sub(self.recent_moment_decay_tick) > (ticks_per_bar * 4.0) as u64 {
                        self.recent_moment_count = self.recent_moment_count.saturating_sub(1);
                        self.recent_moment_decay_tick = self.anim_tick;
                    }
                    self.intent_tension += ((self.prev_energy_trend.max(0.0) * 2.0 + phrase_arc * 0.5) - self.intent_tension) * 0.05;
                    self.intent_tension = self.intent_tension.clamp(0.0, 1.0);
                    let release_signal = (-self.prev_energy_trend).max(0.0) * 2.0
                        + if self.phrase_phase > 0.75 { (self.phrase_phase - 0.75) * 4.0 } else { 0.0 };
                    self.intent_release += (release_signal - self.intent_release) * 0.05;
                    self.intent_release = self.intent_release.clamp(0.0, 1.0);
                    let chaos_signal = (filter_val - self.prev_filter).abs() * 5.0
                        + ((target_sr - self.prev_sr) / 96000.0).abs() * 5.0
                        + self.memory.fatigue * 0.5
                        + self.recent_moment_count as f32 * 0.15;
                    self.intent_chaos += (chaos_signal - self.intent_chaos) * 0.08;
                    self.intent_chaos = self.intent_chaos.clamp(0.0, 1.0);

                    // Trigger new moments
                    if self.moment.active.is_none() && self.moment.cooldown == 0 {
                        let trigger_hash = (self.anim_tick as u32).wrapping_mul(2654435761);
                        let trigger_roll = ((trigger_hash >> 8) & 0xFFFF) as f32 / 65535.0;

                        // Detect param changes for UserAccent
                        let filter_delta = (filter_val - self.prev_filter).abs();
                        let sr_delta = ((target_sr - self.prev_sr) / 96000.0).abs();
                        let param_spike = filter_delta > 0.1 || sr_delta > 0.05;

                        // Detect PEAK enter/exit
                        let _entering_peak = visual_state == 3 && self.prev_energy_state < 3;
                        let exiting_peak = visual_state < 3 && self.prev_energy_state == 3;

                        // V6: Drop detection (PEAK → silence)
                        let entering_drop = visual_state == 0 && self.prev_energy_state >= 2;
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
                        if self.drop_detected {
                            self.drop_timer += 1;
                            // Re-entry: energy spikes back → GlitchBloom + recompose
                            if energy > 0.5 && self.drop_timer > 10 {
                                self.moment.active = Some(Moment::GlitchBloom);
                                self.moment.timer = 0;
                                self.moment.duration = 15;
                                self.moment.seed = trigger_hash;
                                self.moment.bloom_center = (
                                    ((trigger_hash >> 4) as usize % COLS as usize),
                                    ((trigger_hash >> 14) as usize % ROWS as usize),
                                );
                                // Snap overlays to new random positions
                                for i in 0..4 {
                                    self.overlay_anchors[i] = ANCHORS[((trigger_hash >> (i * 3)) as usize) % ANCHORS.len()];
                                }
                                self.drop_detected = false;
                            }
                            if self.drop_timer > 300 { self.drop_detected = false; }
                        }

                        if param_spike {
                            self.moment.active = Some(Moment::UserAccent);
                            self.moment.timer = 0;
                            self.moment.duration = 4 + ((trigger_hash >> 12) % 3) as u32;
                            self.moment.seed = trigger_hash;
                        } else {
                            // V4: Intent-driven moment selection with recovery bias
                            let recovery_mult = if self.moment_recovery_timer > 0 {
                                self.moment_recovery_timer -= 1;
                                0.5
                            } else { 1.0 };
                            let base_prob = (0.013 + energy * 0.05) * phrase_moment_mod * recovery_mult * self.moment_prob_scale * self.visual_profile.moment_mult;
                            // V6: Apply preset-specific moment bias
                            let moment_bias_idx = |m: &Moment| -> usize { match m {
                                Moment::FreezeCut => 0, Moment::GlitchBloom => 1, Moment::LockIn => 2,
                                Moment::PhaseWave => 3, Moment::Collapse => 4, Moment::Afterglow => 5,
                                _ => 0,
                            }};
                            if trigger_roll < base_prob {
                                let intent_mag = self.intent_tension.max(self.intent_release).max(self.intent_chaos);
                                let mut dominant = if self.intent_tension > self.intent_release && self.intent_tension > self.intent_chaos {
                                    if (trigger_hash >> 20) & 1 == 0 { Moment::FreezeCut } else { Moment::LockIn }
                                } else if self.intent_release > self.intent_chaos {
                                    if (trigger_hash >> 20) & 1 == 0 { Moment::Afterglow } else { Moment::Collapse }
                                } else {
                                    if (trigger_hash >> 20) & 1 == 0 { Moment::GlitchBloom } else { Moment::PhaseWave }
                                };
                                // Sequencing rules: LockIn only in PEAK, no FreezeCut during Collapse recovery
                                if matches!(dominant, Moment::LockIn) && visual_state != 3 {
                                    dominant = Moment::FreezeCut;
                                }
                                // V6: Preset moment bias — reroll if bias is low
                                let bias = self.visual_profile.moment_bias[moment_bias_idx(&dominant)];
                                let bias_roll = ((trigger_hash >> 24) & 0xFF) as f32 / 255.0;
                                if bias < 1.0 && bias_roll > bias {
                                    // Skip this moment (bias suppressed it)
                                } else {
                                let base_dur = match dominant {
                                    Moment::FreezeCut => 8, Moment::LockIn => (ticks_per_beat * 0.5) as u32,
                                    Moment::GlitchBloom => 12, Moment::PhaseWave => (ticks_per_bar * 1.5) as u32,
                                    Moment::Collapse => 30, Moment::Afterglow => 45, _ => 10,
                                };
                                let dur = base_dur + ((trigger_hash >> 12) % 4) as u32 + (intent_mag * 4.0) as u32;
                                self.moment.active = Some(dominant);
                                self.moment.timer = 0;
                                self.moment.duration = dur;
                                self.moment.seed = trigger_hash;
                                if matches!(dominant, Moment::GlitchBloom) {
                                    self.moment.bloom_center = (
                                        ((trigger_hash >> 4) as usize % COLS as usize),
                                        ((trigger_hash >> 14) as usize % ROWS as usize),
                                    );
                                }
                                self.recent_moment_count += 1;
                                } // V6: close bias else block
                            }
                        }
                    }
                    self.prev_energy_state = visual_state;
                    self.prev_filter = filter_val;
                    self.prev_sr = target_sr;

                    // ── V3: Micro-freezes (lighter FreezeCut) ──
                    if self.micro_freeze_frames > 0 {
                        self.micro_freeze_frames -= 1;
                    } else if transient && self.moment.active.is_none() {
                        let mf_hash = (self.anim_tick as u32).wrapping_mul(48271);
                        if ((mf_hash >> 12) & 0xFF) < self.visual_profile.micro_freeze_thresh as u32 {
                            self.micro_freeze_frames = 3 + (mf_hash % 5) as u32;
                        }
                    }

                    // ── V3: Moment effect variables (pre-compute for per-cell loop) ──
                    let moment_brightness_boost = if matches!(self.moment.active, Some(Moment::FreezeCut)) { 0.10 } else { 0.0 };
                    let phase_wave_active = matches!(self.moment.active, Some(Moment::PhaseWave));
                    let phase_wave_t = if phase_wave_active { self.moment.timer as f32 } else { 0.0 };
                    let bloom_active = matches!(self.moment.active, Some(Moment::GlitchBloom));
                    let bloom_radius = if bloom_active {
                        ((self.moment.timer as f32 / self.moment.duration.max(1) as f32) * 3.0) as i32
                    } else { 0 };
                    let collapse_active = matches!(self.moment.active, Some(Moment::Collapse));
                    let collapse_t = if collapse_active { self.moment.timer as f32 / self.moment.duration.max(1) as f32 } else { 0.0 };
                    let _afterglow_active = matches!(self.moment.active, Some(Moment::Afterglow));
                    let user_accent_active = matches!(self.moment.active, Some(Moment::UserAccent));
                    let accent_boost = if user_accent_active { 0.15 } else { 0.0 };

                    // ── V3: Restraint — idle windows ──
                    let idle_dampen = if visual_state == 0 && self.moment.active.is_none() { 0.35 } else { 1.0 };
                    // Recovery after moments
                    let recovery_dampen = if self.moment.cooldown > 15 { 0.5 } else { 1.0 };
                    let _in_recovery = self.moment_recovery_timer > 0;

                    for row in 0..ROWS {
                        for col in 0..COLS {
                            let idx = ((row * COLS + col) * 4) as usize;
                            let ru = row as usize;
                            let cu = col as usize;

                            // V2: structural distortion shifts row at low bit depth
                            let effective_row = (ru as i32 + structural_offset).clamp(0, (ROWS - 1) as i32) as usize;
                            let bank_row = effective_row;

                            // ── Base image ──
                            let in_base_margin = ru < BASE_MARGIN || ru >= (ROWS as usize - BASE_MARGIN);
                            // V3: PhaseWave horizontal displacement
                            let phase_shift = if phase_wave_active {
                                ((ru as f32 * 0.4 + phase_wave_t * 0.3).sin() * 1.8) as i32
                            } else { 0 };
                            // V6: Jitter → visual position displacement
                            let jitter_hash = col.wrapping_mul(2246822507)
                                .wrapping_add(row.wrapping_mul(1664525))
                                .wrapping_add(self.anim_tick as u32 * 48271);
                            let jitter_offset = if jitter_val > 0.3 {
                                let jit = ((jitter_hash >> 16) as i32 % 3) - 1; // -1, 0, or 1
                                if jitter_val > 0.7 { jit * 2 } else { jit }
                            } else { 0 };
                            let base_col = cu as i32 + col_drift - core_col_off + phase_shift + jitter_offset;
                            let bank_col_base = if base_col >= 0 { base_col as usize } else { 9999 }; // out-of-bounds → get_cell returns 0
                            let src_row_signed = bank_row as i32 + row_scroll as i32 - core_row_off;
                            let src_row = if src_row_signed >= 0 { src_row_signed as usize } else { 9999 };

                            let base_raw = if in_base_margin || src_row >= core_h as usize {
                                0.0
                            } else if in_transition {
                                // V2: wave-biased scatter-dissolve
                                let dissolve_hash = col.wrapping_mul(48271)
                                    .wrapping_add(row.wrapping_mul(1103515245))
                                    .wrapping_add(core_cycle as u32 * 2654435761);
                                let dissolve_val = ((dissolve_hash >> 16) & 0xFF) as f32 / 255.0;
                                // Direction bias: wave sweeps across rows
                                let bias = (ru as f32 * 0.15 + t * 0.02).sin() * 0.2;
                                let threshold = (transition_progress + bias).clamp(0.0, 1.0);
                                if dissolve_val < threshold {
                                    self.ascii_bank.get_cell(core_img, bank_col_base, src_row) as f32
                                } else {
                                    self.ascii_bank.get_cell(prev_core_img, bank_col_base, src_row) as f32
                                }
                            } else {
                                self.ascii_bank.get_cell(core_img, bank_col_base, src_row) as f32
                            };

                            // ── PHASE 3+4: Filter → structural visibility with coherence ──
                            // Characters ARE preserved in the char index — this only affects COLOR.
                            // At filter=1.0 all cells get full primary color.
                            // At low filter, cells get dimmed/hidden based on neighbor-coherent noise.
                            let is_base = base_raw > 0.0;
                            let structural_alpha = if is_base && filter_val < 0.98 {
                                // PHASE 4: neighbor-averaged noise for blob-like reveal
                                let sh = |c: u32, r: u32| -> f32 {
                                    let s = c.wrapping_mul(2246822507)
                                        .wrapping_add(r.wrapping_mul(1664525))
                                        .wrapping_add(self.anim_tick as u32 * 48271);
                                    ((s >> 16) & 0xFF) as f32 / 255.0
                                };
                                let center = sh(col, row);
                                let neighbor_avg = (
                                    sh(col.wrapping_add(1), row) +
                                    sh(col.wrapping_sub(1), row) +
                                    sh(col, row.wrapping_add(1)) +
                                    sh(col, row.wrapping_sub(1))
                                ) * 0.25;
                                let coherent_noise = center * 0.6 + neighbor_avg * 0.4;
                                // Below filter threshold → dimmed; above → full
                                // V6: AA OFF raises floor (harsher visibility)
                                let floor = if aa_enabled { if is_light { 0.35 } else { 0.15 } }
                                            else { if is_light { 0.50 } else { 0.25 } };
                                if coherent_noise > filter_val { floor } else { 1.0 }
                            } else {
                                1.0
                            };

                            // ── Overlay (full-screen, no margin restriction) ──
                            let mut best_alpha = 0.0f32;
                            let mut best_char_idx = 0usize;
                            let mut best_color_idx = 0usize;
                            {
                                for (si, slot) in slots.iter().enumerate() {
                                    if slot.alpha < 0.01 { continue; }
                                    // Use smeared positions for ghosting at low SR
                                    let smeared_row = self.prev_overlay_rows[si] as i32;
                                    let smeared_col = self.prev_overlay_cols[si] as i32;
                                    // Apply random position offset for small images
                                    let r2_signed = ru as i32 - smeared_row - slot.img_row_offset;
                                    if r2_signed < 0 { continue; }
                                    let r2 = r2_signed as usize;
                                    let shifted_col = cu as i32 + smeared_col - slot.img_col_offset;
                                    if shifted_col < 0 { continue; }

                                    // Scatter-dissolve between old and new overlay image
                                    let raw = if slot.transition_t < 1.0 {
                                        let dh = col.wrapping_mul(31337)
                                            .wrapping_add(row.wrapping_mul(48271))
                                            .wrapping_add(si as u32 * 7919);
                                        let dv = ((dh >> 16) & 0xFF) as f32 / 255.0;
                                        let sharp_t = (slot.transition_t * transition_sharpness).min(1.0);
                                        if dv < sharp_t {
                                            self.ascii_bank.get_cell(slot.img_idx, shifted_col as usize, r2)
                                        } else {
                                            self.ascii_bank.get_cell(slot.prev_img_idx, shifted_col as usize, r2)
                                        }
                                    } else {
                                        self.ascii_bank.get_cell(slot.img_idx, shifted_col as usize, r2)
                                    };
                                    if raw == 0 { continue; }

                                    // V6: SR reduction affects back layers (50% → 25% → 12% → 10%)
                                    if !should_update {
                                        let sr_skip_prob = match slot.depth {
                                            0 => 0.50,  // Pulse: 50% chance to still update
                                            1 => 0.25,  // Accent: 25%
                                            2 => 0.12,  // Drift: 12%
                                            _ => 0.10,  // Ghost: 10%
                                        };
                                        let sr_hash = col.wrapping_mul(2246822507)
                                            .wrapping_add(row.wrapping_mul(48271))
                                            .wrapping_add(si as u32 * 1664525)
                                            .wrapping_add(self.anim_tick as u32);
                                        let sr_roll = ((sr_hash >> 12) & 0xFF) as f32 / 255.0;
                                        if sr_roll > sr_skip_prob { continue; }
                                    }

                                    // V2: Mix controls overlay density
                                    let ov_hash = col.wrapping_mul(7919).wrapping_add(row.wrapping_mul(104729))
                                        .wrapping_add(slot.img_idx as u32 * 31337);
                                    let ov_chance = ((ov_hash >> 16) & 0xFF) as f32 / 255.0;
                                    if ov_chance > overlay_density_threshold * phrase_overlay_mod { continue; }

                                    // V6: depth-of-field alpha reduction for Ghost
                                    let depth_alpha = if slot.depth == 3 { slot.alpha * 0.7 } else { slot.alpha };
                                    if depth_alpha > best_alpha {
                                        best_alpha = depth_alpha;
                                        // V6: depth-of-field glyph restriction
                                        let clamped = match slot.depth {
                                            0 => raw as usize,                         // Pulse: full charset
                                            1 => (raw as usize).min(90),               // Accent: no heavy blocks
                                            2 => (raw as usize).min(60),               // Drift: medium chars
                                            _ => (raw as usize).min(20),               // Ghost: light chars only
                                        };
                                        best_char_idx = clamped;
                                        best_color_idx = slot.color_idx;
                                    }
                                }
                            }
                            let has_overlay = best_alpha > 0.01;

                            // Noise (dust_tick driven — always alive)
                            let noise_seed = col.wrapping_mul(1664525)
                                .wrapping_add(row.wrapping_mul(22695477))
                                .wrapping_add(dust_tick * 134775813);
                            // Dust: oscillates between random (signal) and wavey (structured)
                            // wave_mix drifts slowly so dust character changes over time
                            let wave_mix = (dust_tick as f32 * 0.003).sin() * 0.5 + 0.5;
                            let random_val = ((noise_seed >> 8) & 0xFF) as f32 / 255.0;
                            let wave_val = (ru as f32 * 0.3 + cu as f32 * 0.15 + dust_tick as f32 * 0.02).sin() * 0.5 + 0.5;
                            let dust_present = match self.visual_profile.dust_style {
                                1 => {
                                    // Grid-aligned: structured digital stepping (SP-1200)
                                    let gn = ((cu / 3) as u32 ^ (ru / 2) as u32 ^ dust_tick) as f32;
                                    (gn * 0.618).fract() * 0.7 + random_val * 0.3
                                }
                                2 => {
                                    // Chaotic drift: heavy wave, irregular (Mirage)
                                    ((ru as f32 * 0.7 + cu as f32 * 0.3 + dust_tick as f32 * 0.05).sin() * 0.7 + random_val * 0.5).clamp(0.0, 1.0)
                                }
                                _ => random_val * (1.0 - wave_mix) + wave_val * wave_mix,
                            };
                            let dust_opacity = ((noise_seed) & 0xFF) as f32 / 255.0;

                            let dust_over_overlay = has_overlay && dust_present < 0.02;

                            // PHASE 7: shimmer only on quantized frames
                            let base_idx = if is_base && should_update {
                                let life_seed = noise_seed.wrapping_mul(1103515245).wrapping_add(12345);
                                let life_roll = ((life_seed >> 16) & 0xFFFF) as f32 / 65535.0;
                                if life_roll < 0.002 {
                                    let dir = if (life_seed & 1) == 0 { 1i32 } else { -1 };
                                    (base_raw as i32 + dir).clamp(1, 83) as usize
                                } else {
                                    base_raw as usize
                                }
                            } else { 0 };

                            // ── Compositing: bg → overlay → base ──
                            let bg = palette.background;
                            let mut density_idx;
                            let (mut r, mut g, mut b);

                            // ── V3: Compute overlay structural_alpha (filter affects overlays too) ──
                            let overlay_structural = if has_overlay && filter_val < 0.98 {
                                let sh_ov = |c: u32, r: u32| -> f32 {
                                    let s = c.wrapping_mul(1664525)
                                        .wrapping_add(r.wrapping_mul(2246822507))
                                        .wrapping_add(self.anim_tick as u32 * 31337);
                                    ((s >> 16) & 0xFF) as f32 / 255.0
                                };
                                let cn = sh_ov(col, row);
                                let na = (sh_ov(col.wrapping_add(1), row) + sh_ov(col.wrapping_sub(1), row)
                                    + sh_ov(col, row.wrapping_add(1)) + sh_ov(col, row.wrapping_sub(1))) * 0.25;
                                let coh = cn * 0.6 + na * 0.4;
                                if coh > filter_val { if is_light { 0.35 } else { 0.15 } } else { 1.0 }
                            } else { 1.0 };

                            if has_overlay && !dust_over_overlay {
                                // V2: color temporal drift on overlays
                                let ci = best_color_idx;
                                let c = palette.secondary[ci];
                                let drift_ci = (ci + 1) % 4;
                                let c_drift = palette.secondary[drift_ci];
                                let cd = color_drift.abs();
                                let cr = c.r + (c_drift.r - c.r) * cd;
                                let cg = c.g + (c_drift.g - c.g) * cd;
                                let cb = c.b + (c_drift.b - c.b) * cd;

                                // V3: filter + structural alpha applies to overlays too
                                let ov_alpha = best_alpha * 0.80 * overlay_visibility * base_alpha * overlay_structural;
                                r = bg.r + (cr - bg.r) * ov_alpha;
                                g = bg.g + (cg - bg.g) * ov_alpha;
                                b = bg.b + (cb - bg.b) * ov_alpha;
                                density_idx = best_char_idx;

                                if is_base {
                                    let c2 = palette.primary;
                                    // PHASE 3: structural_alpha dims cells organically at low filter
                                    let ba = base_alpha * structural_alpha * (0.85 + (base_raw / (CHARSET_LEN - 1) as f32) * 0.15);
                                    r = r + (c2.r - r) * ba;
                                    g = g + (c2.g - g) * ba;
                                    b = b + (c2.b - b) * ba;
                                    density_idx = base_idx;
                                }
                            } else if is_base {
                                let c = palette.primary;
                                // PHASE 3: structural_alpha for organic fragmentation at low filter
                                let alpha = base_alpha * structural_alpha * (0.85 + (base_raw / (CHARSET_LEN - 1) as f32) * 0.15);
                                r = bg.r + (c.r - bg.r) * alpha;
                                g = bg.g + (c.g - bg.g) * alpha;
                                b = bg.b + (c.b - bg.b) * alpha;
                                density_idx = base_idx;
                            } else {
                                density_idx = 0;
                                // V2: energy-coupled dust density
                                if dust_present < dust_density || dust_over_overlay {
                                    let c = palette.secondary[3];
                                    let op_base = 0.06 + dust_opacity.powf(0.35) * 0.44;
                                    let op = if is_light { (op_base * 1.6).min(0.85) } else { op_base };
                                    r = bg.r + (c.r - bg.r) * op;
                                    g = bg.g + (c.g - bg.g) * op;
                                    b = bg.b + (c.b - bg.b) * op;
                                } else {
                                    r = bg.r; g = bg.g; b = bg.b;
                                }
                            }

                            // ── Chars: source-faithful by default ──
                            let mut final_density_idx = density_idx;

                            // ── V3: GlitchBloom overlay ──
                            if bloom_active {
                                let bcx = self.moment.bloom_center.0 as i32;
                                let bcy = self.moment.bloom_center.1 as i32;
                                let dx = cu as i32 - bcx;
                                let dy = ru as i32 - bcy;
                                let in_bloom = match self.visual_profile.bloom_shape {
                                    1 => dy.abs() <= 1 && dx.abs() <= bloom_radius * 2,
                                    2 => dx * dx + dy * dy <= bloom_radius * bloom_radius,
                                    3 => {
                                        let jag = ((dy.wrapping_mul(7919) ^ self.moment.seed as i32) & 3) as i32;
                                        dx.abs() <= (bloom_radius + jag) && dy.abs() <= bloom_radius
                                    }
                                    _ => dx.abs() <= bloom_radius && dy.abs() <= bloom_radius,
                                };
                                if in_bloom {
                                    let bloom_seed = col.wrapping_mul(31337).wrapping_add(row.wrapping_mul(7919))
                                        .wrapping_add(self.moment.seed);
                                    let gi = 87 + ((bloom_seed >> 4) as usize % (CHARSET_LEN - 87));
                                    final_density_idx = gi.min(CHARSET_LEN - 1);
                                    // Random theme color per cell (primary + 4 secondaries)
                                    let color_pick = ((bloom_seed >> 8) % 5) as usize;
                                    let gc = if color_pick == 0 { palette.primary } else { palette.secondary[color_pick - 1] };
                                    // Softer than main layer usage — reduced opacity
                                    let bloom_alpha = 0.15 + energy * 0.15;
                                    r = r + (gc.r - r) * bloom_alpha;
                                    g = g + (gc.g - g) * bloom_alpha;
                                    b = b + (gc.b - b) * bloom_alpha;
                                    self.glitch_events_this_frame += 1;
                                }
                            }

                            // ── V3: Collapse — remove cells using coherent noise ──
                            if collapse_active && final_density_idx > 0 {
                                let collapse_noise = col.wrapping_mul(2246822507)
                                    .wrapping_add(row.wrapping_mul(1664525))
                                    .wrapping_add(self.moment.seed);
                                let cn_val = ((collapse_noise >> 16) & 0xFF) as f32 / 255.0;
                                if cn_val < collapse_t * 0.6 {
                                    final_density_idx = 0;
                                }
                            }

                            // ── V4: Coherent glitch field ──
                            if corruption_tier > 0 && final_density_idx > 0 {
                                let glitch_seed = noise_seed.wrapping_mul(2246822507)
                                    .wrapping_add(self.anim_tick as u32 * 1664525);
                                let gf = glitch_field(col, row, self.glitch_field_phase);

                                if gf < glitch_prob * 8.0 {
                                    // Style-aware glyph ranges per preset
                                    let (pt_base, pt_range, cl_base, cl_range) = match self.visual_profile.glitch_style {
                                        1 => (94, 12, 94, 12),                  // h-line: block elements only
                                        2 => (84, 30, 87, 37),                  // warped: wide range
                                        3 => (1, 10, 94, 6),                    // minimal: light chars + thin blocks
                                        _ => (1, CHARSET_LEN - 1, 87, 22),      // mixed: full range
                                    };
                                    match corruption_tier {
                                        1 => {
                                            let gi = pt_base + ((glitch_seed >> 4) as usize % pt_range);
                                            final_density_idx = gi.min(CHARSET_LEN - 1);
                                        }
                                        2 => {
                                            let gi = cl_base + ((glitch_seed >> 4) as usize % cl_range);
                                            final_density_idx = gi.min(CHARSET_LEN - 1);
                                        }
                                        _ => {
                                            // Structural: always heavy blocks
                                            let gi = 87 + ((glitch_seed >> 4) as usize % (CHARSET_LEN - 87));
                                            final_density_idx = gi.min(CHARSET_LEN - 1);
                                        }
                                    }
                                    let gc = palette.emphasis;
                                    let gm = if is_light { 0.30 + energy * 0.35 } else { 0.15 + energy * 0.2 };
                                    r = r + (gc.r - r) * gm;
                                    g = g + (gc.g - g) * gm;
                                    b = b + (gc.b - b) * gm;
                                }
                            }

                            // Dust glyph
                            if final_density_idx == 0 && (dust_present < dust_density || dust_over_overlay) {
                                let pick = ((dust_opacity * 6.0) as usize).min(5);
                                final_density_idx = 1 + pick;
                            }

                            // V3+V4: Brightness boost (moment + phrase)
                            // On light themes: emphasis = darken (subtract), on dark: brighten (add)
                            {
                                let boost = moment_brightness_boost + accent_boost;
                                if is_light {
                                    r = (r * phrase_brightness_mod - boost).max(0.0);
                                    g = (g * phrase_brightness_mod - boost).max(0.0);
                                    b = (b * phrase_brightness_mod - boost).max(0.0);
                                } else {
                                    r = (r * phrase_brightness_mod + boost).min(1.0);
                                    g = (g * phrase_brightness_mod + boost).min(1.0);
                                    b = (b * phrase_brightness_mod + boost).min(1.0);
                                }
                            }
                            // V5: Afterglow accent tint (fades toward theme emphasis)
                            if matches!(self.moment.active, Some(Moment::Afterglow)) {
                                let glow_t = 1.0 - (self.moment.timer as f32 / self.moment.duration.max(1) as f32);
                                let tint = 0.08 * glow_t;
                                let ec = palette.emphasis;
                                r = r + (ec.r - r) * tint;
                                g = g + (ec.g - g) * tint;
                                b = b + (ec.b - b) * tint;
                            }
                            // V5: Transient emphasis flash (accent-colored)
                            if transient && accent_boost > 0.0 {
                                let ec = palette.emphasis;
                                r = r + (ec.r - r) * 0.10;
                                g = g + (ec.g - g) * 0.10;
                                b = b + (ec.b - b) * 0.10;
                            }
                            // V3: Idle/recovery dampening
                            if idle_dampen < 1.0 || recovery_dampen < 1.0 {
                                let damp = idle_dampen * recovery_dampen;
                                let bg_r2 = palette.background.r;
                                let bg_g2 = palette.background.g;
                                let bg_b2 = palette.background.b;
                                r = bg_r2 + (r - bg_r2) * damp;
                                g = bg_g2 + (g - bg_g2) * damp;
                                b = bg_b2 + (b - bg_b2) * damp;
                            }

                            // ── V6: Color temperature shift (warm on energy, cool on filter) ──
                            {
                                let color_temp = (energy * 0.12 - (1.0 - filter_val) * 0.06).clamp(-0.08, 0.12);
                                r = (r + color_temp * 0.15).clamp(0.0, 1.0);
                                b = (b - color_temp * 0.10).clamp(0.0, 1.0);
                            }

                            // ── V6: Sub-bass breathing (subtle global brightness pulse) ──
                            {
                                let bass_pulse = (sub_bass * 2.0).min(1.0) * 0.03;
                                if is_light {
                                    r = (r - bass_pulse).max(0.0);
                                    g = (g - bass_pulse).max(0.0);
                                    b = (b - bass_pulse).max(0.0);
                                } else {
                                    r = (r + bass_pulse).min(1.0);
                                    g = (g + bass_pulse).min(1.0);
                                    b = (b + bass_pulse).min(1.0);
                                }
                            }

                            // ── V6: Scanlines at low SR ──
                            {
                                let scanline_strength = sr_effect * self.visual_profile.scanline_amt;
                                if scanline_strength > 0.05 && (ru % 2) == 0 {
                                    let darken = 1.0 - scanline_strength * 0.4;
                                    r *= darken; g *= darken; b *= darken;
                                }
                            }

                            // ── V6: Jitter temporal flicker (random cell dropout) ──
                            if jitter_val > 0.1 {
                                let flicker_roll = ((jitter_hash >> 8) & 0xFF) as f32 / 255.0;
                                if flicker_roll < jitter_val * 0.12 * energy {
                                    final_density_idx = 0;
                                    r = palette.background.r;
                                    g = palette.background.g;
                                    b = palette.background.b;
                                }
                            }

                            // ── V6: AA OFF → harsher structural alpha floors ──
                            // (Already handled via structural_alpha above, but nudge dust)

                            let to_u8 = |v: f32| (v.powf(1.0 / 2.2) * 255.0) as u8;
                            frame_buffer.pixels[idx]     = to_u8(r);
                            frame_buffer.pixels[idx + 1] = to_u8(g);
                            frame_buffer.pixels[idx + 2] = to_u8(b);
                            frame_buffer.pixels[idx + 3] = final_density_idx as u8;
                        }
                    }

                    // ── V6: Motion echo pass (ghost trails at historical positions) ──
                    if self.visual_profile.motion_echo > 0.01 && energy > 0.1 {
                        let echo_color = palette.primary;
                        for echo_age in 1..4usize {
                            let (hist_row, hist_col) = self.motion_history[echo_age];
                            let echo_row_scroll = hist_row as usize;
                            let echo_col_drift = hist_col as i32;
                            let echo_alpha = (0.25 - echo_age as f32 * 0.08) * self.visual_profile.motion_echo * energy;
                            if echo_alpha < 0.01 { continue; }

                            for row in 0..ROWS {
                                for col in 0..COLS {
                                    let idx = ((row * COLS + col) * 4) as usize;
                                    // Only render echo on cells that are currently background
                                    if frame_buffer.pixels[idx + 3] > 0 { continue; }
                                    let ru = row as usize;
                                    let cu = col as usize;
                                    let echo_base_col = cu as i32 + echo_col_drift - core_col_off;
                                    if echo_base_col < 0 { continue; }
                                    let echo_src_row = ru as i32 + echo_row_scroll as i32 - core_row_off;
                                    if echo_src_row < 0 { continue; }
                                    let raw = self.ascii_bank.get_cell(core_img, echo_base_col as usize, echo_src_row as usize);
                                    if raw == 0 { continue; }

                                    let bg = palette.background;
                                    let er = bg.r + (echo_color.r - bg.r) * echo_alpha * 0.5;
                                    let eg = bg.g + (echo_color.g - bg.g) * echo_alpha * 0.5;
                                    let eb = bg.b + (echo_color.b - bg.b) * echo_alpha * 0.5;
                                    let to_u8 = |v: f32| (v.powf(1.0 / 2.2) * 255.0) as u8;
                                    frame_buffer.pixels[idx]     = to_u8(er);
                                    frame_buffer.pixels[idx + 1] = to_u8(eg);
                                    frame_buffer.pixels[idx + 2] = to_u8(eb);
                                    // Keep density_idx at 0 (echo is just color, no glyph)
                                    frame_buffer.pixels[idx + 3] = (raw as usize).min(20) as u8; // ghost chars
                                }
                            }
                        }
                    }

                    self.glitch_field_phase += 0.01;
                    frame_buffer.energy = energy;
                    frame_buffer.bpm = anim_bpm;
                    frame_buffer.sub_bass_energy = sub_bass;
                    frame_buffer.transient = transient;

                    // V3: Update fatigue from glitch events
                    self.memory.fatigue += self.glitch_events_this_frame as f32 * 0.01;
                    self.memory.fatigue = self.memory.fatigue.min(1.0);
                    self.glitch_events_this_frame = 0;

                    if let Ok(mut fb) = self.frame_buffer.lock() {
                        *fb = Some(frame_buffer);
                    }
                }
            }
        });
    }
}

impl EditorData {
    fn apply_preset(&self) {
        // Clear frame buffer on preset change — UpdateFrameBuffer will fill it shortly
        if let Ok(mut fb) = self.frame_buffer.lock() {
            *fb = Some(crate::render::FrameBuffer::new(54, 42));
        }

        let p = &PRESETS[self.preset_idx];
        let setter = ParamSetter::new(&*self.gui_ctx);

        setter.begin_set_parameter(&self.params.target_sr);
        setter.set_parameter(&self.params.target_sr, p.sr);
        setter.end_set_parameter(&self.params.target_sr);

        setter.begin_set_parameter(&self.params.bit_depth);
        setter.set_parameter(&self.params.bit_depth, p.bits);
        setter.end_set_parameter(&self.params.bit_depth);

        setter.begin_set_parameter(&self.params.jitter);
        setter.set_parameter(&self.params.jitter, p.jitter);
        setter.end_set_parameter(&self.params.jitter);

        setter.begin_set_parameter(&self.params.filter_poles);
        setter.set_parameter(&self.params.filter_poles, p.poles);
        setter.end_set_parameter(&self.params.filter_poles);

        // Cutoff always 100% on preset change — user can sweep from there
        setter.begin_set_parameter(&self.params.filter_cutoff);
        setter.set_parameter(&self.params.filter_cutoff, 1.0);
        setter.end_set_parameter(&self.params.filter_cutoff);
    }
}

// ─── Editor factory ───────────────────────────────────────────────────────────

pub(crate) fn default_state() -> Arc<ViziaState> {
    ViziaState::new(|| (WINDOW_WIDTH, WINDOW_HEIGHT))
}

pub(crate) fn create(
    params: Arc<SssssssssamplerParams>,
    editor_state: Arc<ViziaState>,
    anim_params: Arc<Mutex<AnimationParams>>,
) -> Option<Box<dyn Editor>> {
    // Parse all images from ascii.txt (#N separated format)
    let ascii_bank = AsciiBank::from_ascii_txt(include_str!("../ascii.txt"));

    create_vizia_editor(
        editor_state,
        ViziaTheming::Custom,
        move |cx, gui_ctx| {
            // Initial dark frame (46×36 matching image bank)
            let initial_frame = crate::render::FrameBuffer::new(54, 42);
            let frame_buffer = Arc::new(Mutex::new(Some(initial_frame)));

            EditorData {
                params: params.clone(),
                theme_id: 4, // Paris dark
                dark_mode: true,
                preset_idx: DEFAULT_PRESET,
                frame_update_counter: 0,
                gui_ctx: gui_ctx.clone(),
                anim_params: anim_params.clone(),
                frame_buffer,
                ascii_bank: ascii_bank.clone(),
                smoothed_energy: 0.0,
                velocity_row: 0.0,
                velocity_col: 0.0,
                quant_frame: 0,
                prev_row_scroll: 0.0,
                prev_col_drift: 0.0,
                prev_overlay_rows: [0.0; 4],
                prev_overlay_cols: [0.0; 4],
                // Random starting tick so core image doesn't always start on #1
                // Use system time as seed (deterministic per session, random across sessions)
                anim_tick: {
                    let t = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_millis() as u64)
                        .unwrap_or(42);
                    t.wrapping_mul(2654435761) % 10000
                },
                moment: MomentState::default(),
                memory: MemoryState::default(),
                micro_freeze_frames: 0,
                prev_energy_state: 0,
                prev_filter: 0.0,
                prev_sr: 0.0,
                glitch_events_this_frame: 0,
                ui_expanded: false,
                // V4
                phrase_bar_counter: 0.0,
                phrase_phase: 0.0,
                phrase_length_bars: 8.0,
                bpm_stable_bars: 0.0,
                prev_bpm: 120.0,
                intent_tension: 0.0,
                intent_release: 0.0,
                intent_chaos: 0.0,
                prev_energy_trend: 0.0,
                recent_moment_count: 0,
                recent_moment_decay_tick: 0,
                moment_recovery_timer: 0,
                composition_mode: 0,
                core_anchor: (27.0, 21.0),
                core_pos: (27.0, 21.0),
                overlay_anchors: [(27.0, 21.0); 4],
                overlay_positions: [(27.0, 21.0); 4],
                prev_core_anchor: (27.0, 21.0),
                overlay_velocity_rows: [0.0; 4],
                overlay_velocity_cols: [0.0; 4],
                accent_slot_alpha: 0.0,
                glitch_field_phase: 0.0,
                motion_scale: 1.0,
                glitch_scale: 1.0,
                smear_scale: 1.0,
                moment_prob_scale: 1.0,
                feel: Feel::Expressive,
                visual_profile: VISUAL_PROFILES[DEFAULT_PRESET].clone(),
                shared_ui_expanded: Arc::new(Mutex::new(false)),
                phrase_variant: 0,
                phrase_breath_t: 1.0,
                motion_history: [(0.0, 0.0); 4],
                drop_detected: false,
                drop_timer: 0,
                first_load_img1: {
                    let seed = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_millis() as u32)
                        .unwrap_or(42);
                    (seed % 3) == 0 // 33% chance
                },
                drop_phase_timer: 0,
                drop_reentry_timer: 0,
                warp_phase: 0.0,
                intent_mode: 0,
                intent_mode_t: 0.0,
                intent_mode_bars: 0.0,
            }
            .build(cx);

            cx.add_stylesheet(include_str!("../assets/style.css"))
                .expect("Failed to load stylesheet");

                VStack::new(cx, |cx| {
                    // Continuous frame buffer updates for always-alive animation
                    // Emit UpdateFrameBuffer on every frame
                    Binding::new(cx, EditorData::frame_update_counter, |cx, counter_lens| {
                        // Counter increments on every event - use it as a trigger
                        let _ = counter_lens.get(cx);
                        cx.emit(EditorEvent::UpdateFrameBuffer);
                    });

                    // ── Live ASCII art rendering + interactive UI ─────────────
                    {
                        let editor_data = cx.data::<EditorData>().unwrap();
                        let frame_buffer = editor_data.frame_buffer.clone();
                        let params = editor_data.params.clone();
                        let gui_ctx = editor_data.gui_ctx.clone();
                        let ui_expanded = editor_data.shared_ui_expanded.clone();

                        AsciiImageDisplay::new(cx, frame_buffer, params, gui_ctx, ui_expanded);
                    }


                })
                .class("plugin-root");
        },
    )
}

