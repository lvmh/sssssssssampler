//! Live ASCII art display with femtovg UI overlay.
//!
//! Renders FrameBuffer grid, then paints UI text on top using femtovg.
//! Always-visible: title, SR, filter+AA, machine selector.
//! Hover menu: sound section (bits, jitter, mix) + visual section (theme, mode, feel).
//! Dropdown selectors for machine and theme.

use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use std::sync::{Arc, Mutex};
use std::cell::RefCell;
use crate::render::FrameBuffer;
use crate::ascii_bank::{CHARSET, CHARSET_LEN};
use crate::SssssssssamplerParams;
use crate::editor::EditorEvent;

const MONO_ASPECT: f32 = 0.55;
const GRID_COLS: usize = 54;
const GRID_ROWS: usize = 42;
const UI_COL: usize = 3;
const UI_WIDTH: usize = 20;

const PRESET_NAMES: &[&str] = &["SP-1200", "MPC60", "S950", "Mirage", "P-2000", "MPC3000", "SP-303"];
const FEEL_NAMES: &[&str] = &["tight", "expressive", "chaotic"];

// ── Always-visible row positions ──
const ROW_SR: usize = 3;
const ROW_FILTER: usize = 4;
const ROW_AA: usize = 5;
const ROW_MACHINE: usize = 6;
const ROW_SEPARATOR: usize = 7;
const ROW_MORE: usize = 8;
// ── Expanded menu row positions (visible when "more" is open) ──
const ROW_SOUND_HDR: usize = 9;
const ROW_BITS: usize = 10;
const ROW_JITTER: usize = 11;
const ROW_MIX: usize = 12;
const ROW_VISUAL_HDR: usize = 14;
const ROW_THEME: usize = 15;
const ROW_MODE: usize = 16;
const ROW_FEEL: usize = 17;

#[derive(Clone, Copy, Debug, PartialEq)]
enum UiRow {
    SampleRate, Filter, AntiAlias, MachineSelect, MoreToggle,
    BitDepth, Jitter, Mix, ThemeSelect, Mode, Feel,
}

impl UiRow {
    fn from_grid_row(grid_row: usize, _col: usize, menu_vis: bool, more_open: bool) -> Option<Self> {
        match grid_row {
            ROW_SR => Some(Self::SampleRate),
            ROW_FILTER => Some(Self::Filter),
            ROW_AA => Some(Self::AntiAlias),
            ROW_MACHINE => Some(Self::MachineSelect),
            ROW_MORE if menu_vis => Some(Self::MoreToggle),
            ROW_BITS if menu_vis && more_open => Some(Self::BitDepth),
            ROW_JITTER if menu_vis && more_open => Some(Self::Jitter),
            ROW_MIX if menu_vis && more_open => Some(Self::Mix),
            ROW_THEME if menu_vis && more_open => Some(Self::ThemeSelect),
            ROW_MODE if menu_vis && more_open => Some(Self::Mode),
            ROW_FEEL if menu_vis && more_open => Some(Self::Feel),
            _ => None,
        }
    }
    fn is_draggable(self) -> bool {
        matches!(self, Self::SampleRate | Self::Filter | Self::BitDepth | Self::Jitter | Self::Mix)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum DropdownKind { Machine, Theme }

struct DragState {
    row: UiRow, start_x: f32, start_y: f32, start_value: f32,
    /// When set: X-axis controls this param (dual-axis global drag mode)
    col_row: Option<UiRow>, start_value2: f32,
}

pub struct AsciiImageDisplay {
    pub frame_buffer: Arc<Mutex<Option<FrameBuffer>>>,
    pub params: Arc<SssssssssamplerParams>,
    pub gui_ctx: Arc<dyn GuiContext>,
    pub ui_expanded: Arc<Mutex<bool>>,
    font_id: RefCell<Option<vg::FontId>>,
    braille_font_id: RefCell<Option<vg::FontId>>,
    drag: RefCell<Option<DragState>>,
    grid_offset: RefCell<(f32, f32)>,
    grid_cell: RefCell<(f32, f32)>,
    menu_visible: RefCell<bool>,
    hover_row: RefCell<Option<usize>>,
    hover_col: RefCell<Option<usize>>,
    frame_count: RefCell<u64>,
    dropdown: RefCell<Option<DropdownKind>>,
    // V6: menu glitch transition (0.0 = hidden, 1.0 = fully visible)
    menu_reveal_t: RefCell<f32>,
    // Loader: counts up from 0; loader shows while < LOADER_FRAMES
    loader_frame: RefCell<u32>,
// "more" toggle: collapsed by default, click to expand sound+visual sections
    more_expanded: RefCell<bool>,
}

impl AsciiImageDisplay {
    pub fn new(
        cx: &mut Context,
        frame_buffer: Arc<Mutex<Option<FrameBuffer>>>,
        params: Arc<SssssssssamplerParams>,
        gui_ctx: Arc<dyn GuiContext>,
        ui_expanded: Arc<Mutex<bool>>,
    ) -> Handle<'_, Self> {
        Self {
            frame_buffer, params, gui_ctx, ui_expanded,
            font_id: RefCell::new(None),
            braille_font_id: RefCell::new(None),
            drag: RefCell::new(None),
            grid_offset: RefCell::new((0.0, 0.0)),
            grid_cell: RefCell::new((8.0, 12.0)),
            menu_visible: RefCell::new(false),
            hover_row: RefCell::new(None),
            hover_col: RefCell::new(None),
            frame_count: RefCell::new(0),
            dropdown: RefCell::new(None),
            menu_reveal_t: RefCell::new(0.0),
            loader_frame: RefCell::new(0),
            more_expanded: RefCell::new(false),
        }
        .build(cx, |_cx| {})
        .size(Stretch(1.0))
        .navigable(true)
    }

    fn ensure_font(&self, canvas: &mut Canvas) -> Option<vg::FontId> {
        let cached = *self.font_id.borrow();
        if cached.is_some() { return cached; }

        // Embedded FiraCode — works on all platforms, no install needed
        static EMBEDDED_FONT: &[u8] = include_bytes!("../assets/FiraCode-Regular.ttf");
        if let Ok(id) = canvas.add_font_mem(EMBEDDED_FONT) {
            *self.font_id.borrow_mut() = Some(id);
            return Some(id);
        }

        // Fallback: try system fonts
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_default();
        for path in &[
            // macOS
            format!("{}/Library/Fonts/FiraCodeNerdFontMono-Regular.ttf", home),
            format!("{}/Library/Fonts/FiraCode-Regular.ttf", home),
            "/System/Library/Fonts/Menlo.ttc".to_string(),
            // Windows
            format!("{}\\AppData\\Local\\Microsoft\\Windows\\Fonts\\FiraCode-Regular.ttf", home),
            "C:\\Windows\\Fonts\\consola.ttf".to_string(),
            // Linux
            "/usr/share/fonts/truetype/firacode/FiraCode-Regular.ttf".to_string(),
            "/usr/share/fonts/TTF/FiraCode-Regular.ttf".to_string(),
            "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf".to_string(),
        ] {
            if let Ok(id) = canvas.add_font(path) {
                *self.font_id.borrow_mut() = Some(id);
                return Some(id);
            }
        }
        None
    }

    /// Load braille fallback font for U+2800–U+28FF.
    /// Embedded: DejaVu Sans subset (14KB, vector outlines, public domain).
    /// Cross-platform: works on macOS, Windows, Linux — no system font required.
    fn ensure_braille_font(&self, canvas: &mut Canvas) -> Option<vg::FontId> {
        let cached = *self.braille_font_id.borrow();
        if cached.is_some() { return cached; }

        // Embedded DejaVu Sans braille subset — 14KB, pure vector, public domain
        static BRAILLE_FONT: &[u8] = include_bytes!("../assets/DejaVuSans-Braille.ttf");
        if let Ok(id) = canvas.add_font_mem(BRAILLE_FONT) {
            *self.braille_font_id.borrow_mut() = Some(id);
            return Some(id);
        }
        None
    }

    fn pixel_to_cell(&self, px: f32, py: f32) -> Option<(usize, usize)> {
        let (ox, oy) = *self.grid_offset.borrow();
        let (cw, ch) = *self.grid_cell.borrow();
        if cw <= 0.0 || ch <= 0.0 { return None; }
        let col = ((px - ox) / cw) as isize;
        let row = ((py - oy) / ch) as isize;
        if col < 0 || row < 0 || col >= GRID_COLS as isize || row >= GRID_ROWS as isize { return None; }
        Some((col as usize, row as usize))
    }

    fn get_param_value(&self, row: UiRow) -> f32 {
        match row {
            UiRow::SampleRate => self.params.target_sr.value(),
            UiRow::Filter => self.params.filter_cutoff.value(),
            UiRow::BitDepth => self.params.bit_depth.value(),
            UiRow::Jitter => self.params.jitter.value(),
            UiRow::Mix => self.params.mix.value(),
            _ => 0.0,
        }
    }

    /// Velocity-aware drag with micro-inertia.
    /// Slow drags → precise adjustments, fast drags → larger but damped changes.
    fn apply_drag_delta(&self, row: UiRow, start_val: f32, delta: f32, fine: bool) {
        let setter = ParamSetter::new(&*self.gui_ctx);
        // Fine mode (Shift held): 5× reduction
        let fine_mult = if fine { 0.2 } else { 1.0 };
        // Velocity-aware scaling: damp large deltas with sqrt curve
        let velocity_shaped = |d: f32, base_speed: f32| -> f32 {
            let sign = d.signum();
            let mag = d.abs();
            // sqrt curve: large movements grow sublinearly → prevents overshoot
            let shaped = mag.sqrt() * mag.sqrt().sqrt(); // mag^0.75
            sign * shaped * base_speed * fine_mult
        };

        match row {
            UiRow::SampleRate => {
                // Log-scaled drag for frequency-domain parameter
                let log_start = (start_val / 1000.0).max(0.001).ln();
                let speed = velocity_shaped(delta, 0.0012) * (start_val / 1000.0).max(0.5);
                let new_val = ((log_start + speed).exp() * 1000.0).clamp(4000.0, 48_000.0);
                setter.begin_set_parameter(&self.params.target_sr);
                setter.set_parameter(&self.params.target_sr, new_val);
                setter.end_set_parameter(&self.params.target_sr);
            }
            UiRow::Filter => {
                // Log-scaled drag matching SR feel — equal knob travel per octave
                let log_start = (start_val / 1000.0).max(0.0001).ln();
                let speed = velocity_shaped(delta, 0.0012) * (start_val / 1000.0).max(0.1);
                let new_val = ((log_start + speed).exp() * 1000.0).clamp(200.0, 22_050.0);
                setter.begin_set_parameter(&self.params.filter_cutoff);
                setter.set_parameter(&self.params.filter_cutoff, new_val);
                setter.end_set_parameter(&self.params.filter_cutoff);
            }
            UiRow::BitDepth => {
                // Snap to whole-bit steps for clean detents
                let adj = velocity_shaped(delta, 0.04);
                let raw = start_val + adj;
                let snapped = raw.round().clamp(1.0, 24.0);
                setter.begin_set_parameter(&self.params.bit_depth);
                setter.set_parameter(&self.params.bit_depth, snapped);
                setter.end_set_parameter(&self.params.bit_depth);
            }
            UiRow::Jitter => {
                let adj = velocity_shaped(delta, 0.0025);
                setter.begin_set_parameter(&self.params.jitter);
                setter.set_parameter(&self.params.jitter, (start_val + adj).clamp(0.0, 1.0));
                setter.end_set_parameter(&self.params.jitter);
            }
            UiRow::Mix => {
                let adj = velocity_shaped(delta, 0.004);
                setter.begin_set_parameter(&self.params.mix);
                setter.set_parameter(&self.params.mix, (start_val + adj).clamp(0.0, 1.0));
                setter.end_set_parameter(&self.params.mix);
            }
            _ => {}
        }
    }

    /// Helper: render a single row of text, with glitch substitution during menu reveal/hide.
    /// Glitch chars are built into a single string — one fill_text() call regardless of transition.
    fn draw_row(&self, canvas: &mut Canvas, fid: vg::FontId, text: &str,
                 grid_row: usize, color: vg::Color, font_size: f32,
                 cell_w: f32, cell_h: f32, offset_x: f32, offset_y: f32, wave: f32) {
        let reveal_t = *self.menu_reveal_t.borrow();
        if reveal_t < 0.01 { return; }
        let mut paint = vg::Paint::color(color);
        paint.set_font(&[fid]);
        paint.set_font_size(font_size);
        paint.set_text_align(vg::Align::Left);
        paint.set_text_baseline(vg::Baseline::Top);
        let x = offset_x + UI_COL as f32 * cell_w;
        let y = offset_y + grid_row as f32 * cell_h + wave;
        if reveal_t < 0.99 {
            // During transition: substitute random chars proportional to (1 - reveal_t).
            // Built as one string → single fill_text() call, no per-char overhead.
            let frame = *self.frame_count.borrow();
            let glitch_prob = 1.0 - reveal_t;
            const GLITCH_CHARS: &[char] = &[
                '#', '%', '$', '@', '&', '*', '+', '=', '/', '\\', '|', '<', '>', '~',
                '─', '│', '┼', '░', '▒',
            ];
            let seed = (frame as u32).wrapping_mul(2654435761)
                .wrapping_add(grid_row as u32 * 48271);
            let glitched: String = text.chars().enumerate().map(|(ci, ch)| {
                let h = seed.wrapping_add(ci as u32 * 1664525);
                if ((h >> 8) & 0xFF) as f32 / 255.0 < glitch_prob {
                    GLITCH_CHARS[((h >> 16) as usize) % GLITCH_CHARS.len()]
                } else { ch }
            }).collect();
            let _ = canvas.fill_text(x, y, &glitched, &paint);
        } else {
            let _ = canvas.fill_text(x, y, text, &paint);
        }
    }

    /// Render UI overlay
    fn render_ui_overlay(&self, canvas: &mut Canvas, fid: vg::FontId, fb: &FrameBuffer,
                          cell_w: f32, cell_h: f32, offset_x: f32, offset_y: f32) {
        let menu_vis = *self.menu_visible.borrow();
        let drag_row = self.drag.borrow().as_ref().map(|d| match d.row {
            UiRow::SampleRate => ROW_SR,
            UiRow::Filter    => ROW_FILTER,
            UiRow::BitDepth  => ROW_BITS,
            UiRow::Jitter    => ROW_JITTER,
            UiRow::Mix       => ROW_MIX,
            _                => usize::MAX,
        });
        // While dragging: lock highlight to the dragged row, suppress all other hover effects.
        let hover = drag_row.map(Some).unwrap_or(*self.hover_row.borrow());
        let font_size = (cell_h * 0.85).max(6.0);

        let frame = *self.frame_count.borrow();
        let energy = fb.energy;
        let energy_alpha = (0.88 + energy * 0.06).min(1.0);
        let t = frame as f32;
        let dropdown = *self.dropdown.borrow();

        let [pr, pg, pb] = fb.title_rgb;
        let [er, eg, eb] = fb.emphasis_rgb;
        let [br, bg_green, bb] = fb.bg_rgb;

        let scale = |v: u8, a: f32| -> u8 { (v as f32 * a).min(255.0) as u8 };

        let primary_color = |alpha: f32| vg::Color::rgb(scale(pr, alpha), scale(pg, alpha), scale(pb, alpha));
        let emphasis_color = |alpha: f32| vg::Color::rgb(scale(er, alpha), scale(eg, alpha), scale(eb, alpha));
        let dim_color = |alpha: f32| vg::Color::rgb(scale(er, alpha * 0.4), scale(eg, alpha * 0.4), scale(eb, alpha * 0.4));

        let hover_color = |row: usize, alpha: f32| -> vg::Color {
            if hover == Some(row) {
                let mix = 0.4f32;
                let blend = |base: u8, accent: u8| -> u8 {
                    (base as f32 + (accent as f32 - base as f32) * mix).clamp(0.0, 255.0) as u8
                };
                vg::Color::rgb(
                    blend(scale(er, alpha), pr),
                    blend(scale(eg, alpha), pg),
                    blend(scale(eb, alpha), pb),
                )
            } else {
                emphasis_color(alpha)
            }
        };

        let row_wave = |ri: usize| -> f32 { (t * 0.006 + ri as f32 * 0.8).sin() * energy * 0.4 };

        // ═══════════════════════════════════════════════════════════════════
        // PASS 1: Always visible — title, SR, filter+AA, machine
        // ═══════════════════════════════════════════════════════════════════

        // ── Title ──
        {
            let title = "sssssssssampler";
            let bpm = fb.bpm.clamp(40.0, 200.0);
            let ticks_per_beat = 60.0 * 60.0 / bpm;
            let beat_phase = (t / ticks_per_beat) % 1.0;
            let on_downbeat = beat_phase < 0.08;
            let on_transient = fb.transient;
            let drift_base = if on_downbeat { 0.0 } else { (t * 0.0015).sin() * (0.3 + energy * 0.8) };
            let beat_pulse = if on_downbeat { 0.08 } else { ((t * 0.035).sin() * 0.5 + 0.5) * 0.04 };
            let title_alpha = (energy_alpha + beat_pulse).min(1.0);
            let (tr, tg, tb) = if on_downbeat || on_transient {
                let mix = if on_downbeat { 0.30 } else { 0.20 };
                ((pr as f32 + (er as f32 - pr as f32) * mix) as u8,
                 (pg as f32 + (eg as f32 - pg as f32) * mix) as u8,
                 (pb as f32 + (eb as f32 - pb as f32) * mix) as u8)
            } else { (pr, pg, pb) };

            let mut paint = vg::Paint::color(vg::Color::rgb(scale(tr, title_alpha), scale(tg, title_alpha), scale(tb, title_alpha)));
            paint.set_font(&[fid]);
            paint.set_font_size(font_size);
            paint.set_text_align(vg::Align::Left);
            paint.set_text_baseline(vg::Baseline::Top);
            // Glitch chars for the "sssssssss" prefix (indices 0-8)
            const GLITCH_CHARS: &[char] = &['$', 'z', '5', '%', '2', 'S', 'Z', '&', 's'];
            let glitch_seed = (frame as u32).wrapping_mul(2654435761);
            // Glitch probability: scales with energy, occasional at rest
            let glitch_prob = 0.015 + energy * 0.06;

            let base_y = offset_y + 1.0 * cell_h + drift_base;
            // Build the full title string with glitch substitutions, then draw in one call.
            let glitched: String = title.chars().enumerate().map(|(ci, ch)| {
                if ci < 9 {
                    let per_char_hash = glitch_seed.wrapping_add(ci as u32 * 48271);
                    let roll = ((per_char_hash >> 8) & 0xFF) as f32 / 255.0;
                    if roll < glitch_prob {
                        GLITCH_CHARS[((per_char_hash >> 16) as usize) % GLITCH_CHARS.len()]
                    } else { ch }
                } else { ch }
            }).collect();
            let title_x = offset_x + UI_COL as f32 * cell_w;
            let _ = canvas.fill_text(title_x, base_y, &glitched, &paint);
        }

        // ═══════════════════════════════════════════════════════════════════
        // PASS 2: Hover menu — all controls (shown on hover or dropdown)
        // ═══════════════════════════════════════════════════════════════════

        let reveal_t = *self.menu_reveal_t.borrow();
        if reveal_t < 0.01 && dropdown.is_none() { return; }

        // Menu item color: emphasis (not primary — reserve primary for title only)
        // Apply reveal_t as alpha multiplier for smooth fade
        let menu_color = |row: usize, alpha: f32| -> vg::Color {
            if hover == Some(row) {
                let mix = 0.3f32;
                let blend = |base: u8, accent: u8| -> u8 {
                    (base as f32 + (accent as f32 - base as f32) * mix).clamp(0.0, 255.0) as u8
                };
                vg::Color::rgb(blend(scale(er, alpha), pr), blend(scale(eg, alpha), pg), blend(scale(eb, alpha), pb))
            } else {
                emphasis_color(alpha * 0.85)
            }
        };

        // ── SR (row 3) — always shown when menu active ──
        let sr = self.params.target_sr.value();
        let sr_str = if sr >= 1000.0 { format!("bw: {:.1}k", sr / 1000.0) } else { format!("bw: {:.0}", sr) };
        self.draw_row(canvas, fid, &sr_str, ROW_SR, menu_color(ROW_SR, energy_alpha), font_size, cell_w, cell_h, offset_x, offset_y, row_wave(0));

        // ── Filter (row 4) ──
        {
            let fc = self.params.filter_cutoff.value();
            let filter_str = if fc >= 1000.0 { format!("filter: {:.1}k", fc / 1000.0) } else { format!("filter: {:.0}", fc) };
            self.draw_row(canvas, fid, &filter_str, ROW_FILTER, menu_color(ROW_FILTER, energy_alpha), font_size, cell_w, cell_h, offset_x, offset_y, row_wave(1));
        }

        // ── Anti-alias (row 5) ──
        {
            let aa_on = self.params.anti_alias.value();
            let aa_str = if aa_on { "aa: on" } else { "aa: off" };
            let aa_color = if aa_on { emphasis_color(energy_alpha) } else { dim_color(energy_alpha) };
            self.draw_row(canvas, fid, aa_str, ROW_AA, aa_color, font_size, cell_w, cell_h, offset_x, offset_y, row_wave(2));
        }

        // ── Machine (row 6) ──
        {
            let pname = PRESET_NAMES.get(fb.preset_idx as usize).unwrap_or(&"???");
            let machine_str = format!("\u{25ba} {}", pname);
            let color = if dropdown == Some(DropdownKind::Machine) { emphasis_color(energy_alpha) }
                        else { menu_color(ROW_MACHINE, energy_alpha) };
            self.draw_row(canvas, fid, &machine_str, ROW_MACHINE, color, font_size, cell_w, cell_h, offset_x, offset_y, row_wave(2));
        }

        // ── "more" toggle (ROW_SEPARATOR is blank spacing) ──
        {
            let expanded = *self.more_expanded.borrow();
            let more_str = if expanded { "- more" } else { "+ more" };
            self.draw_row(canvas, fid, more_str, ROW_MORE, menu_color(ROW_MORE, energy_alpha), font_size, cell_w, cell_h, offset_x, offset_y, 0.0);
        }

        // ── Extended menu (hidden unless "more" expanded + no dropdown) ──
        let more_open = *self.more_expanded.borrow();
        if more_open && dropdown.is_none() {
            // Sound section
            self.draw_row(canvas, fid, "sound \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}", ROW_SOUND_HDR, dim_color(energy_alpha), font_size, cell_w, cell_h, offset_x, offset_y, 0.0);

            let bits_str = format!("bits: {:.0}", self.params.bit_depth.value());
            self.draw_row(canvas, fid, &bits_str, ROW_BITS, menu_color(ROW_BITS, energy_alpha), font_size, cell_w, cell_h, offset_x, offset_y, row_wave(3));

            let jitter_str = format!("jitter: {:.1}%", self.params.jitter.value() * 100.0);
            self.draw_row(canvas, fid, &jitter_str, ROW_JITTER, menu_color(ROW_JITTER, energy_alpha), font_size, cell_w, cell_h, offset_x, offset_y, row_wave(4));

            let mix_str = format!("mix: {:.0}%", self.params.mix.value() * 100.0);
            self.draw_row(canvas, fid, &mix_str, ROW_MIX, menu_color(ROW_MIX, energy_alpha), font_size, cell_w, cell_h, offset_x, offset_y, row_wave(5));

            // Visual section
            self.draw_row(canvas, fid, "visual \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}", ROW_VISUAL_HDR, dim_color(energy_alpha), font_size, cell_w, cell_h, offset_x, offset_y, 0.0);

            let tname = crate::render::color_system::THEME_NAMES.get(fb.theme_idx as usize).unwrap_or(&"???");
            let theme_str = format!("\u{25ba} {}", tname);
            self.draw_row(canvas, fid, &theme_str, ROW_THEME, menu_color(ROW_THEME, energy_alpha), font_size, cell_w, cell_h, offset_x, offset_y, row_wave(6));

            let mode_str = format!("mode: {}", if fb.dark_mode { "dark" } else { "light" });
            self.draw_row(canvas, fid, &mode_str, ROW_MODE, menu_color(ROW_MODE, energy_alpha), font_size, cell_w, cell_h, offset_x, offset_y, row_wave(7));

            let fname = FEEL_NAMES.get(fb.feel_idx as usize).unwrap_or(&"???");
            let feel_str = format!("feel: {}", fname);
            self.draw_row(canvas, fid, &feel_str, ROW_FEEL, menu_color(ROW_FEEL, energy_alpha), font_size, cell_w, cell_h, offset_x, offset_y, row_wave(8));
        } // close: if more_open && dropdown.is_none()

        // ═══════════════════════════════════════════════════════════════════
        // PASS 3: Dropdown (when open)
        // ═══════════════════════════════════════════════════════════════════

        if let Some(dd) = dropdown {
            match dd {
                DropdownKind::Machine => {
                    // 7 items starting at ROW_MACHINE + 1
                    let start_row = ROW_MACHINE + 1;
                    // Semi-transparent background
                    {
                        let mut path = vg::Path::new();
                        let x = offset_x + (UI_COL as f32 - 0.5) * cell_w;
                        let y = offset_y + start_row as f32 * cell_h - cell_h * 0.2;
                        let w = 14.0 * cell_w;
                        let h = (PRESET_NAMES.len() as f32 + 0.4) * cell_h;
                        path.rect(x, y, w, h);
                        canvas.fill_path(&mut path, &vg::Paint::color(vg::Color::rgba(br, bg_green, bb, 220)));
                    }
                    for (i, name) in PRESET_NAMES.iter().enumerate() {
                        let grid_row = start_row + i;
                        let is_current = fb.preset_idx as usize == i;
                        let is_hover = hover == Some(grid_row);
                        let prefix = if is_current { "\u{25ba} " } else { "  " };
                        let text = format!("{}{}", prefix, name);
                        let color = if is_hover { primary_color(1.0) }
                                    else if is_current { primary_color(energy_alpha) }
                                    else { emphasis_color(energy_alpha * 0.7) };
                        self.draw_row(canvas, fid, &text, grid_row, color, font_size, cell_w, cell_h, offset_x, offset_y, 0.0);
                    }
                }
                DropdownKind::Theme => {
                    let theme_names = &crate::render::color_system::THEME_NAMES;
                    let start_row = ROW_THEME + 1;
                    let rows_needed = 7;
                    // Semi-transparent background
                    {
                        let mut path = vg::Path::new();
                        let x = offset_x + (UI_COL as f32 - 0.5) * cell_w;
                        let y = offset_y + start_row as f32 * cell_h - cell_h * 0.2;
                        let w = 24.0 * cell_w;
                        let h = (rows_needed as f32 + 0.4) * cell_h;
                        path.rect(x, y, w, h);
                        canvas.fill_path(&mut path, &vg::Paint::color(vg::Color::rgba(br, bg_green, bb, 220)));
                    }
                    // 2 columns of 7
                    for i in 0..theme_names.len().min(14) {
                        let col_offset = if i < 7 { 0 } else { 12 };
                        let row_in_col = i % 7;
                        let grid_row = start_row + row_in_col;
                        let is_current = fb.theme_idx as usize == i;
                        let is_hover_item = hover == Some(grid_row) && {
                            let hc = self.hover_col.borrow();
                            if i < 7 { hc.map(|c| c < UI_COL + 12).unwrap_or(false) }
                            else { hc.map(|c| c >= UI_COL + 12).unwrap_or(false) }
                        };
                        let prefix = if is_current { "\u{25ba} " } else { "  " };
                        let text = format!("{}{}", prefix, theme_names[i]);
                        let color = if is_hover_item { primary_color(1.0) }
                                    else if is_current { primary_color(energy_alpha) }
                                    else { emphasis_color(energy_alpha * 0.7) };

                        let mut paint = vg::Paint::color(color);
                        paint.set_font(&[fid]);
                        paint.set_font_size(font_size);
                        paint.set_text_align(vg::Align::Left);
                        paint.set_text_baseline(vg::Baseline::Top);
                        let x = offset_x + (UI_COL + col_offset) as f32 * cell_w;
                        let y = offset_y + grid_row as f32 * cell_h;
                        let _ = canvas.fill_text(x, y, &text, &paint);
                    }
                }
            }
        }
    }
}

impl View for AsciiImageDisplay {
    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        if bounds.w <= 0.0 || bounds.h <= 0.0 { return; }

        // Update menu visibility
        {
            let mx = cx.mouse().cursorx - bounds.x;
            let my = cx.mouse().cursory - bounds.y;
            let in_zone = mx < bounds.w * 0.5 && my < bounds.h * 0.5;
            let dragging = self.drag.borrow().is_some();
            let dd_open = self.dropdown.borrow().is_some();
            // Auto-close "more" when mouse leaves and nothing is holding the menu open
            if !in_zone && !dragging && !dd_open {
                *self.more_expanded.borrow_mut() = false;
            }
            let more_open = *self.more_expanded.borrow();
            *self.menu_visible.borrow_mut() = in_zone || dragging || dd_open || more_open;
        }

        let frame = {
            let mut fc = self.frame_count.borrow_mut();
            *fc += 1;
            *fc
        };
        // ── Loader ────────────────────────────────────────────────────────────
        // Three pulsing dots in the top-left corner while the GPU atlas warms up.
        // Pure path rendering — no fill_text, no font dependency.
        const LOADER_FRAMES: u32 = 50;
        const LOADER_FADE: u32 = 8;
        {
            let mut lf = self.loader_frame.borrow_mut();
            if *lf <= LOADER_FRAMES + LOADER_FADE { *lf += 1; }
        }
        let lf = *self.loader_frame.borrow();

        if lf < LOADER_FRAMES + LOADER_FADE {
            let alpha = if lf < LOADER_FRAMES { 1.0f32 }
                        else { 1.0 - (lf - LOADER_FRAMES) as f32 / LOADER_FADE as f32 };

            // Replicate cell layout to place dots exactly where the title will appear
            let rows = GRID_ROWS as f32;
            let cols = GRID_COLS as f32;
            let cell_h_fh = bounds.h / rows;
            let cell_w_fh = cell_h_fh * MONO_ASPECT;
            let cell_w_fw = bounds.w / cols;
            let cell_h_fw = cell_w_fw / MONO_ASPECT;
            let (cell_w, cell_h) = if cell_w_fh * cols <= bounds.w {
                (cell_w_fh, cell_h_fh)
            } else {
                (cell_w_fw, cell_h_fw)
            };
            let total_h = cell_h * rows;
            let offset_x = bounds.x;
            let offset_y = bounds.y + (bounds.h - total_h) * 0.5;
            let title_x = offset_x + UI_COL as f32 * cell_w;
            let title_y = offset_y + 1.0 * cell_h;

            // Theme color from FrameBuffer if available, else warm gray fallback
            let (pr, pg, pb, br, bg_c, bb) = if let Ok(lock) = self.frame_buffer.lock() {
                if let Some(fb) = lock.as_ref() {
                    let [r, g, b] = fb.primary_rgb;
                    let [bgr, bgg, bgb] = fb.bg_rgb;
                    (r, g, b, bgr, bgg, bgb)
                } else { (180, 180, 180, 14, 14, 20) }
            } else { (180, 180, 180, 14, 14, 20) };

            // Full-window background fill so the host doesn't show through
            let mut full_bg = vg::Path::new();
            full_bg.rect(bounds.x, bounds.y, bounds.w, bounds.h);
            canvas.fill_path(&mut full_bg, &vg::Paint::color(vg::Color::rgba(br, bg_c, bb, (255.0 * alpha) as u8)));

            // Three dots, horizontally spaced like the title text chars
            let spacing = cell_w * 1.2;
            for i in 0u32..3 {
                let phase = lf as f32 * 0.22 + i as f32 * 0.8;
                let pulse = (phase.sin() * 0.5 + 0.5) * 0.75 + 0.25;
                let r = (pr as f32 * pulse * alpha) as u8;
                let g = (pg as f32 * pulse * alpha) as u8;
                let b = (pb as f32 * pulse * alpha) as u8;
                let mut p = vg::Path::new();
                p.circle(title_x + i as f32 * spacing + cell_w * 0.5, title_y + cell_h * 0.5, (cell_h * 0.18).max(1.5));
                canvas.fill_path(&mut p, &vg::Paint::color(vg::Color::rgba(r, g, b, 255)));
            }

            // Pre-warm glyph atlas invisibly near end of loader (so first real frame is instant)
            if lf == LOADER_FRAMES.saturating_sub(5) {
                if let Some(fid) = self.ensure_font(canvas) {
                    let warmup = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 !@#$%&*()[]{}<>|/\\,.;:'\"~-_=+`^";
                    let mut wp = vg::Paint::color(vg::Color::rgba(0, 0, 0, 0));
                    wp.set_font(&[fid]);
                    wp.set_font_size(14.0);
                    let _ = canvas.fill_text(-9999.0, -9999.0, warmup, &wp);
                }
            }

            if lf < LOADER_FRAMES { return; }
            // lf in [LOADER_FRAMES, LOADER_FRAMES+LOADER_FADE): fall through to draw real frame underneath
        }

        let font = self.ensure_font(canvas);

        // Title always fully revealed (no typewriter — avoids startup stutter)

        // Menu reveal: snap immediately (no interpolated transition — avoids per-char render cost)
        {
            let vis = *self.menu_visible.borrow();
            let mut t = self.menu_reveal_t.borrow_mut();
            let target = if vis { 1.0f32 } else { 0.0 };
            *t += (target - *t) * 0.18;
            if (*t - target).abs() < 0.005 { *t = target; }
        }

        if let Ok(fb_lock) = self.frame_buffer.lock() {
            if let Some(fb) = fb_lock.as_ref() {
                let cols = fb.width as usize;
                let rows = fb.height as usize;
                if cols == 0 || rows == 0 { return; }

                // Background
                {
                    let [bg_r, bg_g, bg_b] = fb.bg_rgb;
                    let mut path = vg::Path::new();
                    path.rect(bounds.x, bounds.y, bounds.w, bounds.h);
                    canvas.fill_path(&mut path, &vg::Paint::color(vg::Color::rgb(bg_r, bg_g, bg_b)));
                }

                let cell_h_from_height = bounds.h / rows as f32;
                let cell_w_from_height = cell_h_from_height * MONO_ASPECT;
                let cell_w_from_width = bounds.w / cols as f32;
                let cell_h_from_width = cell_w_from_width / MONO_ASPECT;

                let (cell_w, cell_h) = if cell_w_from_height * cols as f32 <= bounds.w {
                    (cell_w_from_height, cell_h_from_height)
                } else {
                    (cell_w_from_width, cell_h_from_width)
                };

                let total_h = cell_h * rows as f32;
                let offset_x = bounds.x;
                let offset_y = bounds.y + (bounds.h - total_h) * 0.5;

                *self.grid_offset.borrow_mut() = (offset_x, offset_y);
                *self.grid_cell.borrow_mut() = (cell_w, cell_h);

                let font_size = (cell_h * 0.85).max(6.0);

                // Render ASCII grid
                for row in 0..rows {
                    for col in 0..cols {
                        let pix = (row * cols + col) * 4;
                        if pix + 3 >= fb.pixels.len() || pix / 4 >= fb.char_indices.len() { continue; }
                        let r = fb.pixels[pix];
                        let g = fb.pixels[pix + 1];
                        let b = fb.pixels[pix + 2];
                        let char_idx = (fb.char_indices[pix / 4] as usize).min(CHARSET_LEN - 1);

                        let x = offset_x + col as f32 * cell_w;
                        let y = offset_y + row as f32 * cell_h;

                        if let Some(fid) = font {
                            if char_idx > 0 {
                                let ch = CHARSET[char_idx];
                                let ch_u32 = ch as u32;
                                if ch_u32 >= 0x2800 && ch_u32 <= 0x28FF {
                                    // Braille: draw as path circles — never pass to swash.
                                    // Swash may fallback to system fonts with CBDT bitmap tables
                                    // (Noto etc.) which causes abort(). Path circles are safe.
                                    let bits = (ch_u32 - 0x2800) as u8;
                                    if bits != 0 {
                                        // 8-dot grid: (x_frac, y_frac) for dots 1-8
                                        const DOTS: [(f32, f32); 8] = [
                                            (0.25, 0.14), // dot 1 upper-left
                                            (0.25, 0.38), // dot 2 mid-left
                                            (0.25, 0.62), // dot 3 lower-left
                                            (0.75, 0.14), // dot 4 upper-right
                                            (0.75, 0.38), // dot 5 mid-right
                                            (0.75, 0.62), // dot 6 lower-right
                                            (0.25, 0.86), // dot 7 ext lower-left
                                            (0.75, 0.86), // dot 8 ext lower-right
                                        ];
                                        let dot_r = (cell_w * 0.09).max(1.0);
                                        let mut path = vg::Path::new();
                                        for (i, &(fx, fy)) in DOTS.iter().enumerate() {
                                            if bits & (1 << i) != 0 {
                                                path.circle(x + cell_w * fx, y + cell_h * fy, dot_r);
                                            }
                                        }
                                        canvas.fill_path(&mut path, &vg::Paint::color(vg::Color::rgb(r, g, b)));
                                    }
                                } else {
                                    let mut buf = [0u8; 4];
                                    let s = ch.encode_utf8(&mut buf);
                                    let mut paint = vg::Paint::color(vg::Color::rgb(r, g, b));
                                    paint.set_font(&[fid]);
                                    paint.set_font_size(font_size);
                                    paint.set_text_align(vg::Align::Center);
                                    paint.set_text_baseline(vg::Baseline::Top);
                                    let _ = canvas.fill_text(x + cell_w * 0.5, y, s, &paint);
                                }
                            }
                        } else {
                            let mut path = vg::Path::new();
                            path.rect(x, y, cell_w.ceil(), cell_h.ceil());
                            canvas.fill_path(&mut path, &vg::Paint::color(vg::Color::rgb(r, g, b)));
                        }
                    }
                }

                if let Some(fid) = font {
                    self.render_ui_overlay(canvas, fid, fb, cell_w, cell_h, offset_x, offset_y);
                }
                return;
            }
        }

        let mut path = vg::Path::new();
        path.rect(bounds.x, bounds.y, bounds.w, bounds.h);
        canvas.fill_path(&mut path, &vg::Paint::color(vg::Color::rgb(10, 14, 4)));
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        cx.needs_redraw();
        event.map(|window_event: &WindowEvent, _meta| {
            let menu_vis = *self.menu_visible.borrow();
            match window_event {
                WindowEvent::MouseEnter => {
                    cx.focus_with_visibility(false);
                }
                WindowEvent::MouseMove(_mx, _my) => {
                    let mx = cx.mouse().cursorx;
                    let my = cx.mouse().cursory;
                    let cell = self.pixel_to_cell(mx, my);
                    *self.hover_row.borrow_mut() = cell.map(|(_, r)| r);
                    *self.hover_col.borrow_mut() = cell.map(|(c, _)| c);

                    let drag = self.drag.borrow();
                    if let Some(ds) = drag.as_ref() {
                        let fine = cx.modifiers().contains(Modifiers::SHIFT);
                        if let Some(col_row) = ds.col_row {
                            // Dual-axis: Y → bandwidth, X → filter
                            let dy = ds.start_y - my;
                            let dx = mx - ds.start_x;
                            let (row, sv, sv2) = (ds.row, ds.start_value, ds.start_value2);
                            drop(drag);
                            self.apply_drag_delta(row, sv, dy, fine);
                            self.apply_drag_delta(col_row, sv2, dx, fine);
                        } else {
                            let delta = (mx - ds.start_x) + (ds.start_y - my);
                            let row = ds.row;
                            let sv = ds.start_value;
                            drop(drag);
                            self.apply_drag_delta(row, sv, delta, fine);
                        }
                    }
                }
                WindowEvent::MouseDown(MouseButton::Left) => {
                    let mx = cx.mouse().cursorx;
                    let my = cx.mouse().cursory;

                    // ── Dropdown click handling (always first priority) ──
                    if let Some((col, row)) = self.pixel_to_cell(mx, my) {
                        let dropdown = *self.dropdown.borrow();
                        if let Some(dd) = dropdown {
                            match dd {
                                DropdownKind::Machine => {
                                    let start_row = ROW_MACHINE + 1;
                                    let end_row = start_row + PRESET_NAMES.len();
                                    if row >= start_row && row < end_row && col >= UI_COL && col < UI_COL + 14 {
                                        let idx = row - start_row;
                                        cx.emit(EditorEvent::SelectPreset(idx));
                                    }
                                    *self.dropdown.borrow_mut() = None;
                                }
                                DropdownKind::Theme => {
                                    let start_row = ROW_THEME + 1;
                                    let end_row = start_row + 7;
                                    if row >= start_row && row < end_row {
                                        let row_offset = row - start_row;
                                        let theme_idx = if col >= UI_COL + 12 {
                                            row_offset + 7 // right column
                                        } else {
                                            row_offset // left column
                                        };
                                        if theme_idx < crate::render::color_system::THEME_COUNT {
                                            cx.emit(EditorEvent::SelectTheme(theme_idx));
                                        }
                                    }
                                    *self.dropdown.borrow_mut() = None;
                                }
                            }
                            return; // consume click
                        }
                    }

                    // ── When menu hidden: whole-canvas dual-axis drag ──
                    // Up/down → bandwidth, left/right → filter freq
                    if !menu_vis {
                        *self.drag.borrow_mut() = Some(DragState {
                            row: UiRow::SampleRate,
                            col_row: Some(UiRow::Filter),
                            start_x: mx, start_y: my,
                            start_value: self.get_param_value(UiRow::SampleRate),
                            start_value2: self.get_param_value(UiRow::Filter),
                        });
                        cx.capture();
                        cx.lock_cursor_icon();
                        return;
                    }

                    // ── Normal click handling (menu visible) ──
                    if let Some((col, row)) = self.pixel_to_cell(mx, my) {
                        // Click outside UI column → dual-axis drag, same as menu-hidden behavior
                        if col < UI_COL || col >= UI_COL + UI_WIDTH {
                            *self.drag.borrow_mut() = Some(DragState {
                                row: UiRow::SampleRate,
                                col_row: Some(UiRow::Filter),
                                start_x: mx, start_y: my,
                                start_value: self.get_param_value(UiRow::SampleRate),
                                start_value2: self.get_param_value(UiRow::Filter),
                            });
                            cx.capture();
                            cx.lock_cursor_icon();
                            return;
                        }

                        let more_open = *self.more_expanded.borrow();
                        if let Some(ui_row) = UiRow::from_grid_row(row, col, menu_vis, more_open) {
                            if ui_row.is_draggable() {
                                *self.drag.borrow_mut() = Some(DragState {
                                    row: ui_row, start_x: mx, start_y: my,
                                    start_value: self.get_param_value(ui_row),
                                    col_row: None, start_value2: 0.0,
                                });
                                cx.capture();
                                cx.lock_cursor_icon();
                            } else if ui_row == UiRow::AntiAlias {
                                let setter = ParamSetter::new(&*self.gui_ctx);
                                setter.begin_set_parameter(&self.params.anti_alias);
                                setter.set_parameter(&self.params.anti_alias, !self.params.anti_alias.value());
                                setter.end_set_parameter(&self.params.anti_alias);
                            } else if ui_row == UiRow::MoreToggle {
                                let mut expanded = self.more_expanded.borrow_mut();
                                *expanded = !*expanded;
                            } else if ui_row == UiRow::MachineSelect {
                                let current = *self.dropdown.borrow();
                                *self.dropdown.borrow_mut() = if current == Some(DropdownKind::Machine) { None } else { Some(DropdownKind::Machine) };
                            } else if ui_row == UiRow::ThemeSelect {
                                let current = *self.dropdown.borrow();
                                *self.dropdown.borrow_mut() = if current == Some(DropdownKind::Theme) { None } else { Some(DropdownKind::Theme) };
                            } else if ui_row == UiRow::Mode {
                                cx.emit(EditorEvent::ToggleMode);
                            } else if ui_row == UiRow::Feel {
                                cx.emit(EditorEvent::CycleFeel);
                            }
                        }
                    }
                }
                WindowEvent::MouseUp(MouseButton::Left) => {
                    if self.drag.borrow().is_some() {
                        *self.drag.borrow_mut() = None;
                        cx.release();
                        cx.unlock_cursor_icon();
                    }
                }
                WindowEvent::MouseScroll(_, y) => {
                    if *y != 0.0 {
                        let more_open = *self.more_expanded.borrow();
                        if let Some(grid_row) = *self.hover_row.borrow() {
                            if let Some(ui_row) = UiRow::from_grid_row(grid_row, 0, menu_vis, more_open) {
                                if ui_row.is_draggable() {
                                    let fine = cx.modifiers().contains(Modifiers::SHIFT);
                                    let start_val = self.get_param_value(ui_row);
                                    // positive y = scroll up = increase; negate for natural feel
                                    self.apply_drag_delta(ui_row, start_val, *y * 10.0, fine);
                                }
                            }
                        }
                    }
                }
                WindowEvent::KeyDown(Code::ArrowUp, _) => {
                    let fine = cx.modifiers().contains(Modifiers::SHIFT);
                    let start_val = self.get_param_value(UiRow::SampleRate);
                    self.apply_drag_delta(UiRow::SampleRate, start_val, 8.0, fine);
                }
                WindowEvent::KeyDown(Code::ArrowDown, _) => {
                    let fine = cx.modifiers().contains(Modifiers::SHIFT);
                    let start_val = self.get_param_value(UiRow::SampleRate);
                    self.apply_drag_delta(UiRow::SampleRate, start_val, -8.0, fine);
                }
                WindowEvent::KeyDown(Code::ArrowRight, _) => {
                    let fine = cx.modifiers().contains(Modifiers::SHIFT);
                    let start_val = self.get_param_value(UiRow::Filter);
                    self.apply_drag_delta(UiRow::Filter, start_val, 8.0, fine);
                }
                WindowEvent::KeyDown(Code::ArrowLeft, _) => {
                    let fine = cx.modifiers().contains(Modifiers::SHIFT);
                    let start_val = self.get_param_value(UiRow::Filter);
                    self.apply_drag_delta(UiRow::Filter, start_val, -8.0, fine);
                }
                WindowEvent::MouseDoubleClick(MouseButton::Left) => {
                    let setter = ParamSetter::new(&*self.gui_ctx);
                    setter.begin_set_parameter(&self.params.anti_alias);
                    setter.set_parameter(&self.params.anti_alias, !self.params.anti_alias.value());
                    setter.end_set_parameter(&self.params.anti_alias);
                }
                WindowEvent::KeyDown(Code::KeyA, _) => {
                    let setter = ParamSetter::new(&*self.gui_ctx);
                    setter.begin_set_parameter(&self.params.anti_alias);
                    setter.set_parameter(&self.params.anti_alias, !self.params.anti_alias.value());
                    setter.end_set_parameter(&self.params.anti_alias);
                }
                _ => {}
            }
        });
    }
}
