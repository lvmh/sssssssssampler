//! Live ASCII art display with femtovg UI overlay.
//!
//! Renders FrameBuffer grid, then paints UI text on top using femtovg.
//! Animation is never overwritten — UI floats above.
//! Menu appears when mouse is in the top-left quarter.

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
const GRID_COLS: usize = 46;
const GRID_ROWS: usize = 36;
const UI_COL: usize = 3;
const UI_ROW: usize = 3;
const UI_WIDTH: usize = 20;

const PRESET_NAMES: &[&str] = &["SP-1200", "SP-12", "S612", "SP-303", "S950", "MPC3000"];
const THEME_NAMES: &[&str] = &["noni light", "noni dark", "paris", "rooney", "brasil"];

#[derive(Clone, Copy, Debug, PartialEq)]
enum UiRow {
    SampleRate, Filter, AntiAlias, MoreToggle,
    Preset, BitDepth, Jitter, Mix, Theme,
}

impl UiRow {
    fn from_grid_row(grid_row: usize, expanded: bool) -> Option<Self> {
        if grid_row < UI_ROW { return None; }
        match grid_row - UI_ROW {
            0 => Some(Self::SampleRate),
            1 => Some(Self::Filter),
            2 => Some(Self::AntiAlias),
            3 => Some(Self::MoreToggle),
            4 if expanded => Some(Self::Preset),
            5 if expanded => Some(Self::BitDepth),
            6 if expanded => Some(Self::Jitter),
            7 if expanded => Some(Self::Mix),
            8 if expanded => Some(Self::Theme),
            _ => None,
        }
    }
    fn is_draggable(self) -> bool {
        matches!(self, Self::SampleRate | Self::Filter | Self::BitDepth | Self::Jitter | Self::Mix)
    }
}

struct DragState { row: UiRow, start_x: f32, start_y: f32, start_value: f32 }

pub struct AsciiImageDisplay {
    pub frame_buffer: Arc<Mutex<Option<FrameBuffer>>>,
    pub params: Arc<SssssssssamplerParams>,
    pub gui_ctx: Arc<dyn GuiContext>,
    pub ui_expanded: Arc<Mutex<bool>>,
    font_id: RefCell<Option<vg::FontId>>,
    drag: RefCell<Option<DragState>>,
    grid_offset: RefCell<(f32, f32)>,
    grid_cell: RefCell<(f32, f32)>,
    menu_visible: RefCell<bool>,
    hover_row: RefCell<Option<usize>>,
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
            drag: RefCell::new(None),
            grid_offset: RefCell::new((0.0, 0.0)),
            grid_cell: RefCell::new((8.0, 12.0)),
            menu_visible: RefCell::new(false),
            hover_row: RefCell::new(None),
        }
        .build(cx, |_cx| {})
        .size(Stretch(1.0))
    }

    fn ensure_font(&self, canvas: &mut Canvas) -> Option<vg::FontId> {
        let cached = *self.font_id.borrow();
        if cached.is_some() { return cached; }
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| std::env::var("USER").map(|u| format!("/Users/{}", u)).unwrap_or_default());
        for path in &[
            format!("{}/Library/Fonts/FiraCodeNerdFontMono-Regular.ttf", home),
            format!("{}/Library/Fonts/FiraCodeNerdFont-Regular.ttf", home),
            format!("{}/Library/Fonts/FiraCode-Regular.ttf", home),
            "/Users/calmingwaterpad/Library/Fonts/FiraCodeNerdFontMono-Regular.ttf".to_string(),
            "/Users/calmingwaterpad/Library/Fonts/FiraCode-Regular.ttf".to_string(),
            "/Library/Fonts/FiraCodeNerdFontMono-Regular.ttf".to_string(),
            "/System/Library/Fonts/Menlo.ttc".to_string(),
            "/System/Library/Fonts/Monaco.ttf".to_string(),
        ] {
            if let Ok(id) = canvas.add_font(path) {
                *self.font_id.borrow_mut() = Some(id);
                return Some(id);
            }
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

    fn apply_drag_delta(&self, row: UiRow, start_val: f32, delta: f32) {
        let setter = ParamSetter::new(&*self.gui_ctx);
        match row {
            UiRow::SampleRate => {
                let log_start = (start_val / 1000.0).max(0.001).ln();
                let new_val = ((log_start + delta * 0.008).exp() * 1000.0).clamp(1000.0, 96000.0);
                setter.begin_set_parameter(&self.params.target_sr);
                setter.set_parameter(&self.params.target_sr, new_val);
                setter.end_set_parameter(&self.params.target_sr);
            }
            UiRow::Filter => {
                setter.begin_set_parameter(&self.params.filter_cutoff);
                setter.set_parameter(&self.params.filter_cutoff, (start_val + delta * 0.004).clamp(0.0, 1.0));
                setter.end_set_parameter(&self.params.filter_cutoff);
            }
            UiRow::BitDepth => {
                setter.begin_set_parameter(&self.params.bit_depth);
                setter.set_parameter(&self.params.bit_depth, (start_val + delta * 0.06).clamp(1.0, 24.0));
                setter.end_set_parameter(&self.params.bit_depth);
            }
            UiRow::Jitter => {
                setter.begin_set_parameter(&self.params.jitter);
                setter.set_parameter(&self.params.jitter, (start_val + delta * 0.004).clamp(0.0, 1.0));
                setter.end_set_parameter(&self.params.jitter);
            }
            UiRow::Mix => {
                setter.begin_set_parameter(&self.params.mix);
                setter.set_parameter(&self.params.mix, (start_val + delta * 0.004).clamp(0.0, 1.0));
                setter.end_set_parameter(&self.params.mix);
            }
            _ => {}
        }
    }

    /// Render UI text overlay using femtovg (not into framebuffer)
    fn render_ui_overlay(&self, canvas: &mut Canvas, fid: vg::FontId, fb: &FrameBuffer, cell_w: f32, cell_h: f32, offset_x: f32, offset_y: f32) {
        let menu_vis = *self.menu_visible.borrow();
        let hover = *self.hover_row.borrow();
        let expanded = self.ui_expanded.lock().map(|e| *e).unwrap_or(false);
        let font_size = (cell_h * 0.85).max(6.0);

        let [pr, pg, pb] = fb.primary_rgb;
        let [er, eg, eb] = fb.emphasis_rgb;

        // Helper: render text at grid position
        let draw_text = |canvas: &mut Canvas, row: usize, col: usize, text: &str, r: u8, g: u8, b: u8, highlight: bool| {
            let base_x = offset_x + col as f32 * cell_w;
            let base_y = offset_y + row as f32 * cell_h;
            let (cr, cg, cb) = if highlight { (r.saturating_add(40), g.saturating_add(40), b.saturating_add(40)) } else { (r, g, b) };
            let mut paint = vg::Paint::color(vg::Color::rgb(cr, cg, cb));
            paint.set_font(&[fid]);
            paint.set_font_size(font_size);
            paint.set_text_align(vg::Align::Left);
            paint.set_text_baseline(vg::Baseline::Top);
            let _ = canvas.fill_text(base_x, base_y, text, &paint);
        };

        // Title — always visible
        draw_text(canvas, 1, UI_COL, "sssssssssampler", pr, pg, pb, false);

        // Menu — only when hovering in top-left quarter
        if !menu_vis { return; }

        let sr = self.params.target_sr.value();
        let sr_str = if sr >= 1000.0 { format!("sr: {:.1}k", sr / 1000.0) } else { format!("sr: {:.0}", sr) };
        let filter_str = format!("filter: {:.0}%", self.params.filter_cutoff.value() * 100.0);
        let aa_str = if self.params.anti_alias.value() { "aa: on" } else { "aa: off" };

        let rows: Vec<(usize, String)> = {
            let mut v = vec![
                (UI_ROW + 0, sr_str),
                (UI_ROW + 1, filter_str),
                (UI_ROW + 2, aa_str.to_string()),
            ];
            if expanded {
                v.push((UI_ROW + 3, "[ less ]".to_string()));
                let pname = PRESET_NAMES.get(fb.preset_idx as usize).unwrap_or(&"???");
                v.push((UI_ROW + 4, format!("< {} >", pname)));
                v.push((UI_ROW + 5, format!("bits: {:.1}", self.params.bit_depth.value())));
                v.push((UI_ROW + 6, format!("jitter: {:.1}%", self.params.jitter.value() * 100.0)));
                v.push((UI_ROW + 7, format!("mix: {:.0}%", self.params.mix.value() * 100.0)));
                let tname = THEME_NAMES.get(fb.theme_idx as usize).unwrap_or(&"???");
                v.push((UI_ROW + 8, format!("theme: {}", tname)));
            } else {
                v.push((UI_ROW + 3, "[ more ]".to_string()));
            }
            v
        };

        for (grid_row, text) in &rows {
            let is_hover = hover == Some(*grid_row);
            draw_text(canvas, *grid_row, UI_COL, text, er, eg, eb, is_hover);
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
            *self.menu_visible.borrow_mut() = in_zone || dragging;
        }

        let font = self.ensure_font(canvas);

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
                        if pix + 3 >= fb.pixels.len() { continue; }
                        let r = fb.pixels[pix];
                        let g = fb.pixels[pix + 1];
                        let b = fb.pixels[pix + 2];
                        let char_idx = (fb.pixels[pix + 3] as usize).min(CHARSET_LEN - 1);

                        let x = offset_x + col as f32 * cell_w;
                        let y = offset_y + row as f32 * cell_h;

                        if let Some(fid) = font {
                            if char_idx > 0 {
                                let ch = CHARSET[char_idx];
                                let mut buf = [0u8; 4];
                                let s = ch.encode_utf8(&mut buf);
                                let mut paint = vg::Paint::color(vg::Color::rgb(r, g, b));
                                paint.set_font(&[fid]);
                                paint.set_font_size(font_size);
                                paint.set_text_align(vg::Align::Center);
                                paint.set_text_baseline(vg::Baseline::Top);
                                let _ = canvas.fill_text(x + cell_w * 0.5, y, s, &paint);
                            }
                        } else {
                            let mut path = vg::Path::new();
                            path.rect(x, y, cell_w.ceil(), cell_h.ceil());
                            canvas.fill_path(&mut path, &vg::Paint::color(vg::Color::rgb(r, g, b)));
                        }
                    }
                }

                // Render UI overlay on top (never overwrites framebuffer)
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
            let expanded = self.ui_expanded.lock().map(|e| *e).unwrap_or(false);
            match window_event {
                WindowEvent::MouseMove(_mx, _my) => {
                    let mx = cx.mouse().cursorx;
                    let my = cx.mouse().cursory;
                    *self.hover_row.borrow_mut() = self.pixel_to_cell(mx, my).map(|(_, r)| r);

                    let drag = self.drag.borrow();
                    if let Some(ds) = drag.as_ref() {
                        let delta = (mx - ds.start_x) + (ds.start_y - my);
                        let row = ds.row;
                        let sv = ds.start_value;
                        drop(drag);
                        self.apply_drag_delta(row, sv, delta);
                    }
                }
                WindowEvent::MouseDown(MouseButton::Left) => {
                    let mx = cx.mouse().cursorx;
                    let my = cx.mouse().cursory;
                    if let Some((col, row)) = self.pixel_to_cell(mx, my) {
                        if col < UI_COL || col >= UI_COL + UI_WIDTH || row < UI_ROW { return; }
                        if !*self.menu_visible.borrow() { return; }
                        if let Some(ui_row) = UiRow::from_grid_row(row, expanded) {
                            if ui_row.is_draggable() {
                                *self.drag.borrow_mut() = Some(DragState {
                                    row: ui_row, start_x: mx, start_y: my,
                                    start_value: self.get_param_value(ui_row),
                                });
                                cx.capture();
                                cx.lock_cursor_icon();
                            } else if ui_row == UiRow::AntiAlias {
                                let setter = ParamSetter::new(&*self.gui_ctx);
                                setter.begin_set_parameter(&self.params.anti_alias);
                                setter.set_parameter(&self.params.anti_alias, !self.params.anti_alias.value());
                                setter.end_set_parameter(&self.params.anti_alias);
                            } else if ui_row == UiRow::MoreToggle {
                                cx.emit(EditorEvent::ToggleUiExpand);
                            } else if ui_row == UiRow::Theme {
                                cx.emit(EditorEvent::CycleTheme);
                            } else if ui_row == UiRow::Preset {
                                let (ox, _) = *self.grid_offset.borrow();
                                let (cw, _) = *self.grid_cell.borrow();
                                if mx < ox + (UI_COL as f32 + 8.0) * cw {
                                    cx.emit(EditorEvent::PrevPreset);
                                } else {
                                    cx.emit(EditorEvent::NextPreset);
                                }
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
                _ => {}
            }
        });
    }
}
