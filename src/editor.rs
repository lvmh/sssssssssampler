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
use crate::ascii_bank::{AsciiBank, CHARSET_LEN};
use crate::render::color_system::ColorPalette;
use std::sync::Mutex;

// Window sized for 46×36 grid at proper monospace aspect (0.60 w/h ratio).
// Grid area: 46 cols × ~7.2px = 331w, 36 rows × 12px = 432h
// Chrome: header(44) + preset(32) + controls(88) = 164px
// Total: ~500w × 596h — slightly wider for breathing room
pub(crate) const WINDOW_WIDTH: u32 = 540;
pub(crate) const WINDOW_HEIGHT: u32 = 600;

// ─── Machine presets ──────────────────────────────────────────────────────────
//
// poles: 2.0 = 2-pole LP (SP-1200 / SP-12 lineage — under-filtered, gritty)
//        4.0 = 4-pole Butterworth (S950 / S612 / SP-303 / MPC3000 — clean)
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
    MachinePreset { name: "S612",    sr: 31_250.0, bits: 12.0, jitter: 0.01, poles: 4.0 },
    MachinePreset { name: "SP-303",  sr: 32_000.0, bits: 12.0, jitter: 0.01, poles: 4.0 },
    MachinePreset { name: "S950",    sr: 39_375.0, bits: 12.0, jitter: 0.01, poles: 4.0 },
    MachinePreset { name: "MPC3000", sr: 44_100.0, bits: 16.0, jitter: 0.0,  poles: 4.0 },
];

// S950 — matches the default target_sr in lib.rs (39_375 Hz)
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
}

pub enum EditorEvent {
    SetTheme(Theme),
    PrevPreset,
    NextPreset,
    UpdateFrameBuffer,
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
                    const BANK_COLS: usize = 46;
                    const BANK_ROWS: usize = 36;
                    const BASE_MARGIN: usize = 2;
                    const OVERLAY_MARGIN: usize = 6;

                    let mut frame_buffer = crate::render::FrameBuffer::new(COLS, ROWS);

                    let instability = anim_params.instability;
                    let playing = anim_params.playing;

                    // ── Read DSP params for visual effects ──
                    let filter_val = self.params.filter_cutoff.value(); // 0.0–1.0
                    let mix_val    = self.params.mix.value();           // 0.0–1.0
                    let bit_depth  = self.params.bit_depth.value();     // 4.0–16.0
                    let jitter_val = self.params.jitter.value();        // 0.0–1.0

                    // Filter controls base image visibility (lower filter = base fades)
                    let base_visibility = filter_val.clamp(0.0, 1.0);
                    // Mix controls overlay: 100% mix → 80% opacity, 0% mix → 0%
                    // No filter interaction for overlays
                    let overlay_visibility = mix_val.clamp(0.0, 1.0) * 0.80;

                    // Glitch: ONLY when bit depth < 11. Scales 0% at 11 → 20% at 4.
                    let glitch_intensity = if bit_depth < 11.0 {
                        ((11.0 - bit_depth) / 7.0).clamp(0.0, 1.0) * 0.20
                    } else {
                        0.0
                    };

                    let palette = match self.theme {
                        Theme::NoniLight   => ColorPalette::noni_light(),
                        Theme::NoniDark    => ColorPalette::noni_dark(),
                        Theme::Paris       => ColorPalette::paris(),
                        Theme::Rooney      => ColorPalette::rooney(),
                        Theme::BrazilLight => ColorPalette::brazil_light(),
                    };

                    // Store theme background sRGB for display canvas
                    let to_u8 = |v: f32| (v.powf(1.0 / 2.2) * 255.0) as u8;
                    frame_buffer.bg_rgb = [
                        to_u8(palette.background.r),
                        to_u8(palette.background.g),
                        to_u8(palette.background.b),
                    ];

                    // ── Play/stop state ──
                    // anim_tick: drives image cycling/scrolling — freezes when stopped
                    // dust_tick: always advances — dust keeps playing when paused
                    let dust_tick = self.frame_update_counter as u32;
                    if playing {
                        self.anim_tick = self.anim_tick.wrapping_add(1);
                    }
                    // When stopped, anim_tick stays frozen — images pause
                    let t = self.anim_tick as f32;

                    // ── BPM-synced timing ──
                    let bpm = anim_params.bpm.clamp(40.0, 200.0);
                    let ticks_per_beat = 60.0 * 60.0 / bpm;
                    let ticks_per_bar = ticks_per_beat * 4.0;
                    let ticks_per_half = ticks_per_beat * 2.0;

                    // When stopped, images stay visible but frozen (dust keeps moving)
                    let play_factor = 1.0f32;

                    let img_count = self.ascii_bank.len(); // 20 images

                    // ── Core image cycling: new image every 4 bars ──
                    let core_cycle_len = ticks_per_bar * 4.0;
                    let core_cycle = (t / core_cycle_len) as usize;
                    let core_img = core_cycle % img_count;
                    let prev_core_img = if core_cycle > 0 { (core_cycle - 1) % img_count } else { 0 };

                    // Transition: scatter-dissolve over half a bar when core changes
                    let time_in_cycle = t % core_cycle_len;
                    let transition_window = ticks_per_bar * 0.5;
                    let transition_progress = (time_in_cycle / transition_window).min(1.0);
                    let in_transition = transition_progress < 1.0;

                    // Base image: ping-pong scroll over 4 bars
                    let base_phase = (t % core_cycle_len) / core_cycle_len;
                    let base_pos = if base_phase < 0.5 { base_phase * 2.0 } else { 2.0 - base_phase * 2.0 };
                    let row_scroll = (base_pos * (BANK_ROWS - 1) as f32) as usize;

                    // Horizontal drift: ±6 columns, compound sinusoidal
                    let drift_phase = t / core_cycle_len;
                    let col_drift = ((drift_phase * std::f32::consts::TAU).sin() * 6.0
                        + (drift_phase * 2.7).sin() * 3.0) as i32;

                    // ── Overlay slots: 4, 6, 8 bar cycles ──
                    const NUM_SLOTS: usize = 3;
                    const SLOT_BARS: [f32; NUM_SLOTS] = [4.0, 6.0, 8.0];
                    const HOLD_THRESHOLD: f32 = 0.15;

                    struct OverlaySlot {
                        img_idx: usize,
                        alpha: f32,
                        row_shift: usize,
                        col_shift: i32,
                        color_idx: usize,
                    }

                    let slots: [OverlaySlot; NUM_SLOTS] = std::array::from_fn(|i| {
                        let slot_period = ticks_per_bar * SLOT_BARS[i];
                        let phase_offset = i as f32 * (slot_period / NUM_SLOTS as f32);
                        let sin_val = ((t + phase_offset) / slot_period * std::f32::consts::TAU).sin();
                        let raw_alpha = ((sin_val - HOLD_THRESHOLD) / (1.0 - HOLD_THRESHOLD))
                            .clamp(0.0, 1.0) * overlay_visibility * play_factor;

                        let cycle = ((t + phase_offset) / slot_period) as usize;
                        // Pick image, skip current core image
                        let mut img_idx = cycle % img_count;
                        if img_idx == core_img { img_idx = (img_idx + 1) % img_count; }

                        // Independent movement per slot
                        let row_speed = 1.0 / ticks_per_half * (1.0 + i as f32 * 0.5);
                        let row_shift = ((t * row_speed) as usize) % BANK_ROWS;

                        let hash = (cycle as u32).wrapping_mul(2654435761).wrapping_add(i as u32 * 1013904223);
                        let base_col_shift = ((hash >> 16) % 31) as i32 - 15;
                        let slot_drift = ((t * 0.01 + i as f32 * 2.1).sin() * 8.0) as i32;
                        let col_shift = base_col_shift + slot_drift;

                        OverlaySlot {
                            img_idx,
                            alpha: raw_alpha,
                            row_shift,
                            col_shift,
                            color_idx: i % 4,
                        }
                    });

                    for row in 0..ROWS {
                        for col in 0..COLS {
                            let idx = ((row * COLS + col) * 4) as usize;
                            let ru = row as usize;
                            let cu = col as usize;

                            let bank_row = (ru * BANK_ROWS) / ROWS as usize;

                            // Base image with drift + margin
                            let in_base_margin = ru < BASE_MARGIN || ru >= (ROWS as usize - BASE_MARGIN);
                            let drifted_col = cu as i32 + col_drift;
                            let bank_col_base = if drifted_col >= 0 && drifted_col < BANK_COLS as i32 {
                                drifted_col as usize
                            } else {
                                cu // fallback to undrifted if out of bounds
                            };
                            let src_row = bank_row + row_scroll;
                            let base_raw = if in_base_margin || src_row >= BANK_ROWS {
                                0.0 // Never wrap — show empty if past image bounds
                            } else if in_transition {
                                // Scatter-dissolve: per-cell hash decides old vs new image
                                let dissolve_hash = col.wrapping_mul(48271)
                                    .wrapping_add(row.wrapping_mul(1103515245))
                                    .wrapping_add(core_cycle as u32 * 2654435761);
                                let dissolve_val = ((dissolve_hash >> 16) & 0xFF) as f32 / 255.0;
                                if dissolve_val < transition_progress {
                                    // New image
                                    self.ascii_bank.get_cell(core_img, bank_col_base, src_row) as f32
                                } else {
                                    // Old image still showing
                                    self.ascii_bank.get_cell(prev_core_img, bank_col_base, src_row) as f32
                                }
                            } else {
                                self.ascii_bank.get_cell(core_img, bank_col_base, src_row) as f32
                            };
                            let is_base = base_raw > 0.0 && base_visibility > 0.05;

                            // ── Overlay: full layer behind base, show ALL chars ──
                            let in_overlay_margin = ru < OVERLAY_MARGIN || ru >= (ROWS as usize - OVERLAY_MARGIN);
                            let mut best_alpha = 0.0f32;
                            let mut best_density = 0.0f32;
                            let mut best_char_idx = 0usize;
                            let mut best_color_idx = 0usize;
                            if !in_overlay_margin {
                                for slot in &slots {
                                    if slot.alpha < 0.01 { continue; }
                                    let r2 = bank_row + slot.row_shift;
                                    if r2 >= BANK_ROWS { continue; } // Never wrap
                                    let shifted_col = cu as i32 + slot.col_shift;
                                    if shifted_col < 0 || shifted_col >= BANK_COLS as i32 { continue; }

                                    let raw = self.ascii_bank.get_cell(slot.img_idx, shifted_col as usize, r2);
                                    if raw == 0 { continue; }

                                    // Show ALL overlay chars — no render mask
                                    if slot.alpha > best_alpha {
                                        best_alpha = slot.alpha;
                                        best_char_idx = (raw as usize).min(86); // ASCII only
                                        best_density = raw as f32 / (CHARSET_LEN - 1) as f32;
                                        best_color_idx = slot.color_idx;
                                    }
                                }
                            }
                            let has_overlay = best_alpha > 0.01;

                            // Noise values — use dust_tick so dust keeps moving when paused
                            let noise_seed = col.wrapping_mul(1664525)
                                .wrapping_add(row.wrapping_mul(22695477))
                                .wrapping_add(dust_tick * 134775813);
                            let noise        = ((noise_seed >> 16) & 0xFF) as f32 / 255.0;
                            let dust_present = ((noise_seed >>  8) & 0xFF) as f32 / 255.0;
                            let dust_opacity = ((noise_seed      ) & 0xFF) as f32 / 255.0;

                            // Only 2% of overlay chars get affected by dust
                            let dust_over_overlay = has_overlay && dust_present < 0.02;

                            // Base image: per-frame life (0.2% chance of ±1 char jitter)
                            let base_idx = if is_base {
                                let life_seed = noise_seed.wrapping_mul(1103515245).wrapping_add(12345);
                                let life_roll = ((life_seed >> 16) & 0xFFFF) as f32 / 65535.0;
                                if life_roll < 0.002 {
                                    let dir = if (life_seed & 1) == 0 { 1i32 } else { -1 };
                                    (base_raw as i32 + dir).clamp(1, 83) as usize
                                } else {
                                    base_raw as usize
                                }
                            } else {
                                0
                            };

                            // ── Compositing: overlay behind, base on top ──
                            // Layer order: background → overlay → base
                            let bg = palette.background;
                            let density;
                            let mut density_idx;
                            let (mut r, mut g, mut b);

                            if has_overlay && !dust_over_overlay {
                                // Overlay layer — chars pass through exactly as source
                                let c = palette.secondary[best_color_idx];
                                let ov_alpha = best_alpha * 0.80 * overlay_visibility;
                                r = bg.r + (c.r - bg.r) * ov_alpha;
                                g = bg.g + (c.g - bg.g) * ov_alpha;
                                b = bg.b + (c.b - bg.b) * ov_alpha;
                                // Use exact char from source — no density modification
                                density_idx = best_char_idx;
                                density = best_density;

                                // If base also has content here, composite base ON TOP
                                if is_base {
                                    let c2 = palette.primary;
                                    let base_alpha = base_visibility * (0.85 + (base_raw / (CHARSET_LEN - 1) as f32) * 0.15);
                                    r = r + (c2.r - r) * base_alpha;
                                    g = g + (c2.g - g) * base_alpha;
                                    b = b + (c2.b - b) * base_alpha;
                                    density_idx = base_idx;
                                }
                            } else if is_base {
                                // Base only (no overlay here)
                                let c = palette.primary;
                                let alpha = base_visibility * (0.85 + (base_raw / (CHARSET_LEN - 1) as f32) * 0.15);
                                r = bg.r + (c.r - bg.r) * alpha;
                                g = bg.g + (c.g - bg.g) * alpha;
                                b = bg.b + (c.b - bg.b) * alpha;
                                density_idx = base_idx;
                                density = base_raw / (CHARSET_LEN - 1) as f32;
                            } else {
                                // Empty cell — dust
                                density_idx = 0;
                                density = 0.0;
                                if dust_present < 0.66 || dust_over_overlay {
                                    let c = palette.secondary[3];
                                    let op = 0.06 + dust_opacity.powf(0.35) * 0.44;
                                    r = bg.r + (c.r - bg.r) * op;
                                    g = bg.g + (c.g - bg.g) * op;
                                    b = bg.b + (c.b - bg.b) * op;
                                } else {
                                    r = bg.r;
                                    g = bg.g;
                                    b = bg.b;
                                }
                            }
                            let _ = density; // used implicitly via density_idx

                            // ── Chars pass through exactly as source by default ──
                            let mut final_density_idx = density_idx.min(86);

                            // ── Glitch: only 2% of ANY image chars (bit depth < 11) ──
                            // This is the ONLY way characters get modified from source
                            if glitch_intensity > 0.0 && final_density_idx > 0 {
                                let glitch_seed = noise_seed.wrapping_mul(2246822507);
                                let glitch_roll = ((glitch_seed >> 12) & 0xFF) as f32 / 255.0;
                                // Hard cap: max 0.2% of chars affected
                                if glitch_roll < (glitch_intensity * 0.01).min(0.002) {
                                    let block_idx = 1 + ((glitch_seed >> 4) as usize % (CHARSET_LEN - 1));
                                    final_density_idx = block_idx.min(CHARSET_LEN - 1);
                                    let glitch_color = palette.emphasis;
                                    let gm = 0.3;
                                    r = r + (glitch_color.r - r) * gm;
                                    g = g + (glitch_color.g - g) * gm;
                                    b = b + (glitch_color.b - b) * gm;
                                }
                            }

                            // Dust glyph: only light ASCII punctuation (. ' ` , : ;)
                            // Never use block elements — they're too visually heavy for dust
                            if final_density_idx == 0 && (dust_present < 0.66 || dust_over_overlay) {
                                let pick = ((dust_opacity * 6.0) as usize).min(5);
                                final_density_idx = 1 + pick; // indices 1–6: . ' ` , : ;
                            }

                            let to_u8 = |v: f32| (v.powf(1.0 / 2.2) * 255.0) as u8;
                            frame_buffer.pixels[idx]     = to_u8(r);
                            frame_buffer.pixels[idx + 1] = to_u8(g);
                            frame_buffer.pixels[idx + 2] = to_u8(b);
                            frame_buffer.pixels[idx + 3] = final_density_idx as u8;
                        }
                    }

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
    // Build the ASCII image bank once — images are 46 wide × 36 tall natively
    let ascii_bank = AsciiBank::from_raw_images(
        &[
            include_str!("../assets/img01.txt"),
            include_str!("../assets/img02.txt"),
            include_str!("../assets/img03.txt"),
            include_str!("../assets/img04.txt"),
            include_str!("../assets/img05.txt"),
            include_str!("../assets/img06.txt"),
            include_str!("../assets/img07.txt"),
            include_str!("../assets/img08.txt"),
            include_str!("../assets/img09.txt"),
            include_str!("../assets/img10.txt"),
            include_str!("../assets/img11.txt"),
            include_str!("../assets/img12.txt"),
            include_str!("../assets/img13.txt"),
            include_str!("../assets/img14.txt"),
            include_str!("../assets/img15.txt"),
            include_str!("../assets/img16.txt"),
            include_str!("../assets/img17.txt"),
            include_str!("../assets/img18.txt"),
            include_str!("../assets/img19.txt"),
            include_str!("../assets/img20.txt"),
        ],
        46, // cols = max line width of images
        36, // rows = line count of images
    );

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
                anim_tick: 0,
                gui_ctx: gui_ctx.clone(),
                anim_params: anim_params.clone(),
                frame_buffer,
                ascii_bank: ascii_bank.clone(),
            }
            .build(cx);

            cx.add_stylesheet(include_str!("../assets/style.css"))
                .expect("Failed to load stylesheet");

            Binding::new(cx, EditorData::theme, |cx, theme_lens| {
                let theme = theme_lens.get(cx);

                VStack::new(cx, |cx| {
                    // Continuous frame buffer updates for always-alive animation
                    // Emit UpdateFrameBuffer on every frame
                    Binding::new(cx, EditorData::frame_update_counter, |cx, counter_lens| {
                        // Counter increments on every event - use it as a trigger
                        let _ = counter_lens.get(cx);
                        cx.emit(EditorEvent::UpdateFrameBuffer);
                    });

                    // ── Header ────────────────────────────────────────────────
                    HStack::new(cx, |cx| {
                        Label::new(cx, "sssssssssampler").class("plugin-title");

                        HStack::new(cx, |cx| {
                            for t in THEMES {
                                let is_active = theme == t;
                                Label::new(cx, t.label())
                                    .class("theme-pill")
                                    .toggle_class("active", is_active)
                                    .on_press(move |ex| {
                                        ex.emit(EditorEvent::SetTheme(t));
                                    });
                            }
                        })
                        .class("theme-switcher");
                    })
                    .class("header");

                    // ── Live ASCII art rendering ──────────────────────────────
                    {
                        let editor_data = cx.data::<EditorData>().unwrap();
                        let frame_buffer = editor_data.frame_buffer.clone();

                        AsciiImageDisplay::new(cx, frame_buffer);
                    }

                    // ── Preset navigator ──────────────────────────────────────
                    HStack::new(cx, |cx| {
                        Label::new(cx, "◄")
                            .class("preset-arrow")
                            .on_press(|ex| ex.emit(EditorEvent::PrevPreset));

                        Binding::new(cx, EditorData::preset_idx, |cx, idx_lens| {
                            let idx = idx_lens.get(cx);
                            Label::new(cx, PRESETS[idx].name).class("preset-name");
                        });

                        Label::new(cx, "►")
                            .class("preset-arrow")
                            .on_press(|ex| ex.emit(EditorEvent::NextPreset));
                    })
                    .class("preset-row");

                    // ── Controls ──────────────────────────────────────────────
                    HStack::new(cx, |cx| {
                        param_column(cx, "SAMPLE RATE", EditorData::params, |p| &p.target_sr);
                        param_column(cx, "BIT DEPTH",   EditorData::params, |p| &p.bit_depth);
                        param_column(cx, "JITTER",      EditorData::params, |p| &p.jitter);
                        param_column(cx, "FILTER",      EditorData::params, |p| &p.filter_cutoff);
                        param_column(cx, "MIX",         EditorData::params, |p| &p.mix);

                        // Anti-aliasing toggle
                        VStack::new(cx, |cx| {
                            Label::new(cx, "ANTI-ALIAS").class("param-label");
                            ParamButton::new(cx, EditorData::params, |p| &p.anti_alias)
                                .class("param-button");
                        })
                        .class("param-col");
                    })
                    .class("controls");
                })
                .class("plugin-root")
                .class(theme.css_class());
            });
        },
    )
}

/// A labelled parameter column: small ALL-CAPS label above, slider below.
fn param_column<L, Params, P, FMap>(
    cx: &mut Context,
    label: &str,
    lens: L,
    map: FMap,
) where
    L: Lens<Target = Arc<Params>> + Clone,
    Params: nih_plug::params::Params,
    P: nih_plug::params::Param + 'static,
    FMap: Fn(&Arc<Params>) -> &P + Copy + 'static,
{
    VStack::new(cx, |cx| {
        Label::new(cx, label).class("param-label");
        ParamSlider::new(cx, lens, map).class("param-slider");
    })
    .class("param-col");
}
