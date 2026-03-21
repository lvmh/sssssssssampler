use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::*;
use nih_plug_vizia::{create_vizia_editor, ViziaState, ViziaTheming};
use std::sync::Arc;

use nih_plug_vizia::vizia::binding::Data;

use crate::SssssssssamplerParams;
use crate::AnimationParams;
use crate::editor_view::AsciiRenderView;
use crate::ascii_grid_view::AsciiGridDisplay;
use crate::ascii_image_display::AsciiImageDisplay;
use crate::ascii_bank::{AsciiBank, CHARSET_LEN, char_to_idx};
use crate::render::color_system::ColorPalette;
use std::sync::Mutex;

// Window sized for 46×36 grid at proper monospace aspect (0.60 w/h ratio).
// Grid area: 46 cols × ~7.2px = 331w, 36 rows × 12px = 432h
// Chrome: header(44) + preset(32) + controls(88) = 164px
// Total: ~500w × 596h — slightly wider for breathing room
// Grid: 46×36 at cell_h≈12px (same scale as original with chrome).
// cell_h = 436/36 ≈ 12.1, cell_w = 12.1 × 0.55 ≈ 6.66, total_w = 306
// Window matches old 540×436 grid area (chrome removed).
pub(crate) const WINDOW_WIDTH: u32 = 540;
pub(crate) const WINDOW_HEIGHT: u32 = 436;

// ─── Machine presets ──────────────────────────────────────────────────────────
//
// poles: 2.0 = 2-pole LP (SP-1200 / SP-12 lineage — under-filtered, gritty)
//        4.0 = 4-pole Butterworth (S612 / SP-303 / MPC3000 — clean)
//        6.0 = 6-pole Butterworth (S950 — 36 dB/oct switched-capacitor MF6CN-50)
// cutoff is always set to 1.0 (fully open) on preset load.

struct MachinePreset {
    name: &'static str,
    sr: f32,
    bits: f32,
    jitter: f32,
    poles: f32,
}

const PRESETS: &[MachinePreset] = &[
    MachinePreset { name: "SP-1200", sr: 26_040.0, bits: 12.0, jitter: 0.01, poles: 2.0 },
    MachinePreset { name: "SP-12",   sr: 27_500.0, bits: 12.0, jitter: 0.01, poles: 2.0 },
    MachinePreset { name: "S612",    sr: 32_000.0, bits: 12.0, jitter: 0.01, poles: 4.0 },
    MachinePreset { name: "SP-303",  sr: 44_100.0, bits: 16.0, jitter: 0.01, poles: 4.0 },
    MachinePreset { name: "S950",    sr: 48_000.0, bits: 12.0, jitter: 0.01, poles: 6.0 },
    MachinePreset { name: "MPC3000", sr: 44_100.0, bits: 16.0, jitter: 0.0,  poles: 4.0 },
];

// S950 — matches the default target_sr in lib.rs (48_000 Hz)
const DEFAULT_PRESET: usize = 4;

// ─── Theme ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(clippy::enum_variant_names)]
pub enum Theme {
    NoniLight,
    NoniDark,
    Paris,
    Rooney,
    BrazilLight,
}

impl Theme {
    fn css_class(self) -> &'static str {
        match self {
            Self::NoniLight   => "theme-noni-light",
            Self::NoniDark    => "theme-noni-dark",
            Self::Paris       => "theme-paris",
            Self::Rooney      => "theme-rooney",
            Self::BrazilLight => "theme-brazil-light",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::NoniLight   => "noni ☀",
            Self::NoniDark    => "noni ◉",
            Self::Paris       => "paris",
            Self::Rooney      => "rooney",
            Self::BrazilLight => "brasil ☀",
        }
    }
}

impl Data for Theme {
    fn same(&self, other: &Self) -> bool { self == other }
}

const THEMES: [Theme; 5] = [
    Theme::NoniLight,
    Theme::NoniDark,
    Theme::Paris,
    Theme::Rooney,
    Theme::BrazilLight,
];

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

// ─── Model ────────────────────────────────────────────────────────────────────

#[derive(Lens)]
pub struct EditorData {
    pub params: Arc<SssssssssamplerParams>,
    pub theme: Theme,
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
}

#[derive(Debug, Clone)]
pub enum EditorEvent {
    SetTheme(Theme),
    PrevPreset,
    NextPreset,
    UpdateFrameBuffer,
    CycleTheme,
    ToggleUiExpand,
    Tick,  // Timer event for continuous updates
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
            EditorEvent::SetTheme(t) => self.theme = *t,
            EditorEvent::ToggleUiExpand => {
                self.ui_expanded = !self.ui_expanded;
                if let Ok(mut e) = self.shared_ui_expanded.lock() { *e = self.ui_expanded; }
            }
            EditorEvent::CycleTheme => {
                let idx = THEMES.iter().position(|t| *t == self.theme).unwrap_or(0);
                self.theme = THEMES[(idx + 1) % THEMES.len()];
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
            EditorEvent::Tick => {
                // Trigger frame buffer update on every tick (continuous animation)
                // This makes the display feel alive even without audio
            }
            EditorEvent::UpdateFrameBuffer => {
                if let Ok(anim_params) = self.anim_params.lock() {
                    const COLS: u32 = 46;
                    const ROWS: u32 = 36;
                    const BASE_MARGIN: usize = 1;
                    // Images stored at native resolution — get_cell returns 0 for out-of-bounds
                    // Use COLS/ROWS as display grid, native image coords for lookups

                    let mut frame_buffer = crate::render::FrameBuffer::new(COLS, ROWS);

                    let playing = anim_params.playing;
                    let raw_energy = anim_params.energy;
                    let transient = anim_params.transient;
                    let motion_speed = anim_params.motion_speed;

                    // ── V2: Smooth energy (exponential moving average) ──
                    let smooth_rate = 0.08; // lower = slower response
                    self.smoothed_energy += (raw_energy - self.smoothed_energy) * smooth_rate;
                    let energy = self.smoothed_energy;

                    // ── V2: Visual state from energy ──
                    // IDLE=0, FLOW=1, BUILD=2, PEAK=3
                    let visual_state = if energy < 0.25 { 0u8 }
                        else if energy < 0.60 { 1 }
                        else if energy < 0.85 { 2 }
                        else { 3 };

                    // ── Read DSP params ──
                    let filter_val = self.params.filter_cutoff.value();
                    let mix_val    = self.params.mix.value();
                    let bit_depth  = self.params.bit_depth.value();
                    let _jitter_val = self.params.jitter.value();

                    // ── V2: Filter → structural visibility (probabilistic reveal) ──
                    // filter_val is used as a per-cell threshold, not just alpha
                    let base_alpha = filter_val.clamp(0.0, 1.0);

                    // ── V2: Mix → overlay aggression ──
                    let mix = mix_val.clamp(0.0, 1.0);
                    let overlay_visibility = mix * 0.80;
                    // Overlay density: mix controls how many cells show
                    let overlay_density_threshold = 0.02 + mix * 0.98; // 2%→100%
                    // Overlay scroll rate scales with mix + energy
                    let overlay_speed_mult = 1.0 + mix * 0.5 + energy * 0.5;

                    // ── V2: Tiered corruption from bit depth ──
                    // 16-12: none. 11-9: point glitch. 8-6: cluster. 5-4: structural.
                    let corruption_tier = if bit_depth >= 12.0 { 0u8 }
                        else if bit_depth >= 9.0 { 1 }  // point
                        else if bit_depth >= 6.0 { 2 }  // cluster
                        else { 3 };                      // structural
                    // Probability scales with energy in BUILD/PEAK states
                    let fatigue_mult = (1.0 - self.memory.fatigue).clamp(0.2, 1.0);
                    let glitch_prob = match corruption_tier {
                        0 => 0.0,
                        1 => 0.002 * fatigue_mult,
                        2 => (0.005 + energy * 0.01) * fatigue_mult,
                        _ => (0.01 + energy * 0.02) * fatigue_mult,
                    };

                    let palette = match self.theme {
                        Theme::NoniLight   => ColorPalette::noni_light(),
                        Theme::NoniDark    => ColorPalette::noni_dark(),
                        Theme::Paris       => ColorPalette::paris(),
                        Theme::Rooney      => ColorPalette::rooney(),
                        Theme::BrazilLight => ColorPalette::brazil_light(),
                    };

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
                    frame_buffer.theme_idx = THEMES.iter().position(|t| *t == self.theme).unwrap_or(0) as u8;

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
                    let sr_norm = (target_sr / 96_000.0).clamp(0.0, 1.0);
                    // step_interval: 1 frame at max SR → 8 frames at min SR
                    let step_interval = (1.0 + (1.0 - sr_norm) * 7.0) as u64;
                    let should_update = self.quant_frame % step_interval == 0;

                    // ── PHASE 2: Temporal Smearing (low SR only) ──
                    let smear_factor = (1.0 - sr_norm) * 0.3; // 0.0 at max SR → 0.3 at min SR
                    // V3: Effective smear includes afterglow + memory afterimage
                    let afterglow_smear_early = if matches!(self.moment.active, Some(Moment::Afterglow)) { 0.5 } else { 0.0 };
                    let effective_smear = (smear_factor + afterglow_smear_early + self.memory.afterimage * 0.15).min(0.8);

                    // ── BPM timing ──
                    let bpm = anim_params.bpm.clamp(40.0, 200.0);
                    let ticks_per_beat = 60.0 * 60.0 / bpm;
                    let ticks_per_bar = ticks_per_beat * 4.0;

                    let img_count = self.ascii_bank.len();

                    // ── Core image cycling: every 2 bars, randomized order ──
                    let core_cycle_len = ticks_per_bar * 2.0;
                    let core_cycle = (t / core_cycle_len) as usize;
                    let core_hash = (core_cycle as u32).wrapping_mul(2654435761);
                    let core_img = (core_hash as usize) % img_count;
                    let prev_hash = (core_cycle.wrapping_sub(1) as u32).wrapping_mul(2654435761);
                    let prev_core_img = (prev_hash as usize) % img_count;

                    // Random position offset for small core images
                    let core_w = if core_img < img_count { self.ascii_bank.images[core_img].grid.width as i32 } else { COLS as i32 };
                    let core_h = if core_img < img_count { self.ascii_bank.images[core_img].grid.height as i32 } else { ROWS as i32 };
                    // Allow core to drift partially off-screen (min 30% visible)
                    let min_vis_c = (core_w * 3 / 10).max(1); // 30% of image width
                    let min_vis_r = (core_h * 3 / 10).max(1); // 30% of image height
                    let min_off_c = -(core_w - min_vis_c);     // most negative offset
                    let max_off_c = COLS as i32 - min_vis_c;   // most positive offset
                    let min_off_r = -(core_h - min_vis_r);
                    let max_off_r = ROWS as i32 - min_vis_r;
                    let core_range_c = (max_off_c - min_off_c + 1).max(1);
                    let core_range_r = (max_off_r - min_off_r + 1).max(1);
                    let core_pos_hash = (core_cycle as u32).wrapping_mul(2654435761);
                    let core_col_off = min_off_c + ((core_pos_hash >> 4) as i32 % core_range_c).abs();
                    let core_row_off = min_off_r + ((core_pos_hash >> 12) as i32 % core_range_r).abs();

                    // ── Transition with directional wave bias ──
                    let time_in_cycle = t % core_cycle_len;
                    let transition_window = ticks_per_bar * 0.5;
                    let transition_progress = (time_in_cycle / transition_window).min(1.0);
                    let in_transition = transition_progress < 1.0;

                    // V3: Freeze state (from previous frame's moment)
                    let is_frozen = matches!(self.moment.active, Some(Moment::FreezeCut))
                        || self.micro_freeze_frames > 0;

                    // ── Velocity-based scroll — only update on quantized frames ──
                    let bpm_force = (t / ticks_per_bar * std::f32::consts::TAU).sin() * 0.3;
                    let energy_force = if transient { energy * 2.0 } else { energy * 0.1 };
                    if should_update && !is_frozen {
                        self.velocity_row += bpm_force + energy_force * motion_speed;
                        self.velocity_row *= 0.92;
                        self.velocity_row = self.velocity_row.clamp(-8.0, 8.0);
                        let col_force = (t / (ticks_per_bar * 2.0) * std::f32::consts::TAU).cos() * 0.15;
                        self.velocity_col += col_force + energy_force * 0.2;
                        self.velocity_col *= 0.90;
                        self.velocity_col = self.velocity_col.clamp(-4.0, 4.0);
                    }
                    // Smeared positions: lerp between previous and current
                    let new_row_scroll_f = (self.velocity_row.abs() * 2.0) % ROWS as usize as f32;
                    let new_col_drift_f = self.velocity_col;
                    self.prev_row_scroll += (new_row_scroll_f - self.prev_row_scroll) * (1.0 - effective_smear);
                    self.prev_col_drift += (new_col_drift_f - self.prev_col_drift) * (1.0 - effective_smear);
                    let row_scroll = self.prev_row_scroll as usize;
                    let col_drift = self.prev_col_drift as i32;

                    // ── V2: Structural distortion (tier 3 corruption) ──
                    // At very low bit depth, rows/cols shift by ±1–2
                    let structural_offset = if corruption_tier >= 3 {
                        let s = ((t * 0.07).sin() * 2.0) as i32;
                        s.clamp(-2, 2)
                    } else { 0 };

                    // ── Overlay slots: 4, 6, 8 bar cycles ──
                    const NUM_SLOTS: usize = 4;
                    const SLOT_BARS: [f32; NUM_SLOTS] = [1.5, 2.5, 3.0, 2.0];
                    const HOLD_THRESHOLD: f32 = 0.15;

                    struct OverlaySlot {
                        img_idx: usize,
                        prev_img_idx: usize,
                        transition_t: f32,
                        alpha: f32,
                        row_shift: usize,
                        col_shift: i32,
                        // Random position offset for small images
                        img_row_offset: i32,
                        img_col_offset: i32,
                        color_idx: usize,
                    }

                    // V3: LockIn — check if moment is active (from previous frame)
                    let lockin_active = matches!(self.moment.active, Some(Moment::LockIn));

                    let slots: [OverlaySlot; NUM_SLOTS] = std::array::from_fn(|i| {
                        let slot_period = ticks_per_bar * SLOT_BARS[i];
                        let phase_offset = i as f32 * (slot_period / NUM_SLOTS as f32);
                        let sin_val = ((t + phase_offset) / slot_period * std::f32::consts::TAU).sin();

                        // PHASE 5: Filter → layer priority
                        let filter_boost = 1.0 - filter_val * 0.5;
                        let mut raw_alpha = ((sin_val - HOLD_THRESHOLD) / (1.0 - HOLD_THRESHOLD))
                            .clamp(0.0, 1.0) * overlay_visibility * filter_boost;

                        // Ensure at least 1 overlay is ALWAYS visible:
                        // Slots 0 and 3 have alpha floors so one is always showing
                        let min_alpha = if i == 0 || i == 3 { 0.30 } else { 0.0 };
                        if raw_alpha < min_alpha * overlay_visibility.max(0.3) {
                            raw_alpha = min_alpha * overlay_visibility.max(0.3);
                        }

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
                            ((t * 0.005 * overlay_speed_mult + i as f32 * 1.7).sin() * 3.0)
                        } else { 0.0 };
                        let col_drift_ov = {
                            let base_cs = ((hash >> 16) % 9) as f32 - 4.0;
                            let sd = ((t * 0.008 * overlay_speed_mult + i as f32 * 2.1).sin() * 3.0);
                            (base_cs + sd).clamp(-6.0, 6.0)
                        };
                        let row_shift = row_drift.max(0.0) as usize;
                        let col_shift = col_drift_ov as i32;

                        // Random position for images — can be partially off-screen
                        let pos_hash = hash.wrapping_mul(1103515245).wrapping_add(cycle as u32 * 12345);
                        let img_col_offset = ((pos_hash >> 4) as i32 % (COLS as i32)).abs() - (COLS as i32 / 4);
                        let img_row_offset = ((pos_hash >> 12) as i32 % (ROWS as i32)).abs() - (ROWS as i32 / 4);

                        OverlaySlot {
                            img_idx,
                            prev_img_idx,
                            transition_t,
                            alpha: raw_alpha,
                            row_shift,
                            col_shift,
                            img_row_offset,
                            img_col_offset,
                            color_idx: i % 4,
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
                    let base_dust = 0.66;
                    let dust_density = base_dust + energy * 0.2;
                    let dust_density = if transient { (dust_density + 0.24).min(0.95) } else { dust_density };

                    // ── V2: Color temporal drift (subtle per-layer phase offset) ──
                    let color_drift = (t * 0.001).sin() * 0.05;

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
                            // Moment ended — start cooldown, check for Afterglow
                            let was = self.moment.active.take();
                            self.moment.cooldown = 30; // ~0.5s cooldown
                            // Afterglow triggers after certain moments
                            if matches!(was, Some(Moment::FreezeCut) | Some(Moment::GlitchBloom)) {
                                self.moment.active = Some(Moment::Afterglow);
                                self.moment.timer = 0;
                                self.moment.duration = 20;
                                self.moment.cooldown = 0;
                            }
                        }
                    }
                    // Trigger new moments
                    if self.moment.active.is_none() && self.moment.cooldown == 0 {
                        let trigger_hash = (self.anim_tick as u32).wrapping_mul(2654435761);
                        let trigger_roll = ((trigger_hash >> 8) & 0xFFFF) as f32 / 65535.0;

                        // Detect param changes for UserAccent
                        let filter_delta = (filter_val - self.prev_filter).abs();
                        let sr_delta = ((target_sr - self.prev_sr) / 96000.0).abs();
                        let param_spike = filter_delta > 0.1 || sr_delta > 0.05;

                        // Detect PEAK enter/exit
                        let entering_peak = visual_state == 3 && self.prev_energy_state < 3;
                        let exiting_peak = visual_state < 3 && self.prev_energy_state == 3;

                        if param_spike {
                            self.moment.active = Some(Moment::UserAccent);
                            self.moment.timer = 0;
                            self.moment.duration = 10;
                            self.moment.seed = trigger_hash;
                        } else if transient && energy > 0.8 && trigger_roll < 0.3 {
                            // FreezeCut — most impactful
                            self.moment.active = Some(Moment::FreezeCut);
                            self.moment.timer = 0;
                            self.moment.duration = 5 + ((trigger_hash >> 16) % 16) as u32;
                            self.moment.seed = trigger_hash;
                        } else if transient && energy > 0.6 && trigger_roll < 0.15 {
                            // GlitchBloom
                            self.moment.active = Some(Moment::GlitchBloom);
                            self.moment.timer = 0;
                            self.moment.duration = 15 + ((trigger_hash >> 12) % 10) as u32;
                            self.moment.seed = trigger_hash;
                            self.moment.bloom_center = (
                                ((trigger_hash >> 4) as usize % COLS as usize),
                                ((trigger_hash >> 14) as usize % ROWS as usize),
                            );
                        } else if entering_peak && trigger_roll < 0.4 {
                            // LockIn on PEAK entry
                            self.moment.active = Some(Moment::LockIn);
                            self.moment.timer = 0;
                            self.moment.duration = (ticks_per_beat * 2.0) as u32;
                            self.moment.seed = trigger_hash;
                        } else if energy > 0.7 && trigger_roll < 0.05 {
                            // PhaseWave
                            self.moment.active = Some(Moment::PhaseWave);
                            self.moment.timer = 0;
                            self.moment.duration = 20 + ((trigger_hash >> 8) % 15) as u32;
                            self.moment.seed = trigger_hash;
                        } else if exiting_peak {
                            // Collapse on PEAK exit
                            self.moment.active = Some(Moment::Collapse);
                            self.moment.timer = 0;
                            self.moment.duration = 25;
                            self.moment.seed = trigger_hash;
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
                        if ((mf_hash >> 12) & 0xFF) < 20 {
                            self.micro_freeze_frames = 3 + (mf_hash % 5) as u32;
                        }
                    }

                    // ── V3: Moment effect variables (pre-compute for per-cell loop) ──
                    let moment_brightness_boost = if matches!(self.moment.active, Some(Moment::FreezeCut)) { 0.10 } else { 0.0 };
                    let phase_wave_active = matches!(self.moment.active, Some(Moment::PhaseWave));
                    let phase_wave_t = if phase_wave_active { self.moment.timer as f32 } else { 0.0 };
                    let bloom_active = matches!(self.moment.active, Some(Moment::GlitchBloom));
                    let bloom_radius = if bloom_active {
                        ((self.moment.timer as f32 / self.moment.duration as f32) * 5.0) as i32
                    } else { 0 };
                    let collapse_active = matches!(self.moment.active, Some(Moment::Collapse));
                    let collapse_t = if collapse_active { self.moment.timer as f32 / self.moment.duration.max(1) as f32 } else { 0.0 };
                    let afterglow_active = matches!(self.moment.active, Some(Moment::Afterglow));
                    let user_accent_active = matches!(self.moment.active, Some(Moment::UserAccent));
                    let accent_boost = if user_accent_active { 0.3 } else { 0.0 };

                    // ── V3: Restraint — idle windows ──
                    let idle_dampen = if visual_state == 0 && self.moment.active.is_none() { 0.4 } else { 1.0 };
                    // Recovery after moments
                    let recovery_dampen = if self.moment.cooldown > 15 { 0.6 } else { 1.0 };

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
                                ((ru as f32 * 0.4 + phase_wave_t * 0.3).sin() * 2.5) as i32
                            } else { 0 };
                            // Apply random position offset for small core images
                            let base_col = cu as i32 + col_drift - core_col_off + phase_shift;
                            let bank_col_base = if base_col >= 0 { base_col as usize } else { 9999 }; // out-of-bounds → get_cell returns 0
                            let src_row_signed = bank_row as i32 + row_scroll as i32 - core_row_off;
                            let src_row = if src_row_signed >= 0 { src_row_signed as usize } else { 9999 };

                            let base_raw = if in_base_margin || src_row >= ROWS as usize {
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
                                if coherent_noise > filter_val { 0.15 } else { 1.0 }
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
                                        if dv < slot.transition_t {
                                            self.ascii_bank.get_cell(slot.img_idx, shifted_col as usize, r2)
                                        } else {
                                            self.ascii_bank.get_cell(slot.prev_img_idx, shifted_col as usize, r2)
                                        }
                                    } else {
                                        self.ascii_bank.get_cell(slot.img_idx, shifted_col as usize, r2)
                                    };
                                    if raw == 0 { continue; }

                                    // V2: Mix controls overlay density
                                    let ov_hash = col.wrapping_mul(7919).wrapping_add(row.wrapping_mul(104729))
                                        .wrapping_add(slot.img_idx as u32 * 31337);
                                    let ov_chance = ((ov_hash >> 16) & 0xFF) as f32 / 255.0;
                                    if ov_chance > overlay_density_threshold { continue; }

                                    if slot.alpha > best_alpha {
                                        best_alpha = slot.alpha;
                                        best_char_idx = raw as usize;
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
                            let wave_mix = ((dust_tick as f32 * 0.003).sin() * 0.5 + 0.5); // 0→1 oscillation
                            let random_val = ((noise_seed >> 8) & 0xFF) as f32 / 255.0;
                            let wave_val = ((ru as f32 * 0.3 + cu as f32 * 0.15 + dust_tick as f32 * 0.02).sin() * 0.5 + 0.5);
                            let dust_present = random_val * (1.0 - wave_mix) + wave_val * wave_mix;
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
                                if coh > filter_val { 0.15 } else { 1.0 }
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
                                    let op = 0.06 + dust_opacity.powf(0.35) * 0.44;
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
                                if dx.abs() <= bloom_radius && dy.abs() <= bloom_radius {
                                    let bloom_seed = col.wrapping_mul(31337).wrapping_add(row.wrapping_mul(7919))
                                        .wrapping_add(self.moment.seed);
                                    let gi = 87 + ((bloom_seed >> 4) as usize % (CHARSET_LEN - 87));
                                    final_density_idx = gi.min(CHARSET_LEN - 1);
                                    let gc = palette.emphasis;
                                    let bloom_alpha = 0.3 + energy * 0.3;
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

                            // ── V2: Tiered glitch corruption ──
                            if corruption_tier > 0 && final_density_idx > 0 {
                                // Mix anim_tick so glitch pattern changes with BPM-gated clock
                                let glitch_seed = noise_seed.wrapping_mul(2246822507)
                                    .wrapping_add(self.anim_tick as u32 * 1664525);
                                let glitch_roll = ((glitch_seed >> 12) & 0xFFFF) as f32 / 65535.0;

                                if glitch_roll < glitch_prob {
                                    match corruption_tier {
                                        1 => {
                                            // Point: single char from full CHARSET
                                            let gi = 1 + ((glitch_seed >> 4) as usize % (CHARSET_LEN - 1));
                                            final_density_idx = gi.min(CHARSET_LEN - 1);
                                        }
                                        2 => {
                                            // Cluster: use block elements specifically
                                            let gi = 87 + ((glitch_seed >> 4) as usize % 22);
                                            final_density_idx = gi.min(CHARSET_LEN - 1);
                                        }
                                        _ => {
                                            // Structural: heavy block + box drawing
                                            let gi = 87 + ((glitch_seed >> 4) as usize % (CHARSET_LEN - 87));
                                            final_density_idx = gi.min(CHARSET_LEN - 1);
                                        }
                                    }
                                    let gc = palette.emphasis;
                                    let gm = 0.15 + energy * 0.2;
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

                            // V3: Moment brightness boost + UserAccent
                            if moment_brightness_boost > 0.0 || accent_boost > 0.0 {
                                let boost = moment_brightness_boost + accent_boost;
                                r = (r + boost).min(1.0);
                                g = (g + boost).min(1.0);
                                b = (b + boost).min(1.0);
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

                            let to_u8 = |v: f32| (v.powf(1.0 / 2.2) * 255.0) as u8;
                            frame_buffer.pixels[idx]     = to_u8(r);
                            frame_buffer.pixels[idx + 1] = to_u8(g);
                            frame_buffer.pixels[idx + 2] = to_u8(b);
                            frame_buffer.pixels[idx + 3] = final_density_idx as u8;
                        }
                    }

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
    /// Generate grid color from RMS brightness
    fn grid_color(rms: f32, checkerboard: bool) -> Color {
        let brightness = 0.3 + (rms * 0.7);

        let (r, g, b) = if checkerboard {
            // Soft Violet: (122, 108, 255)
            (
                (brightness * 122.0 / 255.0 * 255.0) as u8,
                (brightness * 108.0 / 255.0 * 255.0) as u8,
                255,
            )
        } else {
            // Muted Green: (76, 175, 130)
            (
                (brightness * 76.0 / 255.0 * 255.0) as u8,
                (brightness * 175.0 / 255.0 * 255.0) as u8,
                (brightness * 130.0 / 255.0 * 255.0) as u8,
            )
        };

        Color::rgb(r, g, b)
    }

    fn apply_preset(&self) {
        // Clear frame buffer on preset change — UpdateFrameBuffer will fill it shortly
        if let Ok(mut fb) = self.frame_buffer.lock() {
            *fb = Some(crate::render::FrameBuffer::new(46, 36));
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
            let initial_frame = crate::render::FrameBuffer::new(46, 36);
            let frame_buffer = Arc::new(Mutex::new(Some(initial_frame)));

            EditorData {
                params: params.clone(),
                theme: Theme::NoniDark,
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
                shared_ui_expanded: Arc::new(Mutex::new(false)),
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

