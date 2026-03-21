//! Live ASCII art display — renders actual characters via femtovg.
//!
//! Each cell reads colour + character index from the FrameBuffer and paints
//! the corresponding CHARSET glyph onto the canvas every frame.
//! Falls back to a coloured rectangle if the font can't be loaded.
//!
//! Cell sizing: derived from actual monospace font metrics (advance width)
//! so characters are never stretched. Display area is centered in bounds.

use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use std::sync::{Arc, Mutex};
use std::cell::RefCell;
use crate::render::FrameBuffer;
use crate::ascii_bank::{CHARSET, CHARSET_LEN};

/// Monospace width-to-height ratio. FiraCode is ~0.60, most mono fonts 0.55–0.62.
/// This ensures cells match the actual glyph advance width.
const MONO_ASPECT: f32 = 0.60;

pub struct AsciiImageDisplay {
    pub frame_buffer: Arc<Mutex<Option<FrameBuffer>>>,
    /// Lazily loaded font ID — None until first draw succeeds.
    font_id: RefCell<Option<vg::FontId>>,
}

impl AsciiImageDisplay {
    pub fn new(
        cx: &mut Context,
        frame_buffer: Arc<Mutex<Option<FrameBuffer>>>,
    ) -> Handle<'_, Self> {
        Self {
            frame_buffer,
            font_id: RefCell::new(None),
        }
        .build(cx, |_cx| {})
        .size(Stretch(1.0))
    }

    /// Load a monospace font at runtime on first draw call. Caches the FontId.
    fn ensure_font(&self, canvas: &mut Canvas) -> Option<vg::FontId> {
        let cached = *self.font_id.borrow();
        if cached.is_some() {
            return cached;
        }

        // Runtime path search — try multiple HOME detection methods since
        // VST hosts may sandbox or override environment variables.
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| {
                // macOS fallback: /Users/<current_user>
                std::env::var("USER")
                    .map(|u| format!("/Users/{}", u))
                    .unwrap_or_default()
            });
        let mut candidates = vec![
            format!("{}/Library/Fonts/FiraCodeNerdFontMono-Regular.ttf", home),
            format!("{}/Library/Fonts/FiraCodeNerdFont-Regular.ttf", home),
            format!("{}/Library/Fonts/FiraCode-Regular.ttf", home),
        ];
        // Also try the specific known user path directly
        candidates.push("/Users/calmingwaterpad/Library/Fonts/FiraCodeNerdFontMono-Regular.ttf".to_string());
        candidates.push("/Users/calmingwaterpad/Library/Fonts/FiraCode-Regular.ttf".to_string());
        // System fallbacks
        candidates.push("/Library/Fonts/FiraCodeNerdFontMono-Regular.ttf".to_string());
        candidates.push("/System/Library/Fonts/Menlo.ttc".to_string());
        candidates.push("/System/Library/Fonts/Monaco.ttf".to_string());
        for path in &candidates {
            if let Ok(id) = canvas.add_font(path) {
                *self.font_id.borrow_mut() = Some(id);
                return Some(id);
            }
        }
        None
    }
}

impl View for AsciiImageDisplay {
    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        if bounds.w <= 0.0 || bounds.h <= 0.0 {
            return;
        }

        let font = self.ensure_font(canvas);

        if let Ok(fb_lock) = self.frame_buffer.lock() {
            if let Some(fb) = fb_lock.as_ref() {
                let cols = fb.width as usize;
                let rows = fb.height as usize;
                if cols == 0 || rows == 0 {
                    return;
                }

                // Background: exact theme background color stored in framebuffer
                {
                    let [bg_r, bg_g, bg_b] = fb.bg_rgb;
                    let mut path = vg::Path::new();
                    path.rect(bounds.x, bounds.y, bounds.w, bounds.h);
                    canvas.fill_path(&mut path, &vg::Paint::color(vg::Color::rgb(bg_r, bg_g, bg_b)));
                }

                // Compute cell size from font metrics — not by dividing bounds evenly.
                // cell_h is the master dimension; cell_w = cell_h * MONO_ASPECT
                // This prevents horizontal stretching.
                let cell_h_from_height = bounds.h / rows as f32;
                let cell_w_from_height = cell_h_from_height * MONO_ASPECT;

                // Also check if width is the constraint
                let cell_w_from_width = bounds.w / cols as f32;
                let cell_h_from_width = cell_w_from_width / MONO_ASPECT;

                // Pick the smaller to fit within bounds
                let (cell_w, cell_h) = if cell_w_from_height * cols as f32 <= bounds.w {
                    (cell_w_from_height, cell_h_from_height)
                } else {
                    (cell_w_from_width, cell_h_from_width)
                };

                // Center the grid within bounds
                let total_w = cell_w * cols as f32;
                let total_h = cell_h * rows as f32;
                let offset_x = bounds.x + (bounds.w - total_w) * 0.5;
                let offset_y = bounds.y + (bounds.h - total_h) * 0.5;

                let font_size = (cell_h * 0.95).max(6.0);

                for row in 0..rows {
                    for col in 0..cols {
                        let pix = (row * cols + col) * 4;
                        if pix + 3 >= fb.pixels.len() {
                            continue;
                        }

                        let r = fb.pixels[pix];
                        let g = fb.pixels[pix + 1];
                        let b = fb.pixels[pix + 2];
                        let char_idx = (fb.pixels[pix + 3] as usize).min(CHARSET_LEN - 1);

                        let x = offset_x + col as f32 * cell_w;
                        let y = offset_y + row as f32 * cell_h;

                        if let Some(fid) = font {
                            let ch = CHARSET[char_idx];

                            if char_idx > 0 {
                                let mut buf = [0u8; 4];
                                let s = ch.encode_utf8(&mut buf);

                                let mut paint = vg::Paint::color(vg::Color::rgb(r, g, b));
                                paint.set_font(&[fid]);
                                paint.set_font_size(font_size);
                                paint.set_text_align(vg::Align::Center);
                                paint.set_text_baseline(vg::Baseline::Top);

                                let cx_pos = x + cell_w * 0.5;
                                let _ = canvas.fill_text(cx_pos, y, s, &paint);
                            }
                        } else {
                            // Fallback: coloured rectangle when font unavailable
                            let mut path = vg::Path::new();
                            path.rect(x, y, cell_w.ceil(), cell_h.ceil());
                            let paint = vg::Paint::color(vg::Color::rgb(r, g, b));
                            canvas.fill_path(&mut path, &paint);
                        }
                    }
                }
                return;
            }
        }

        // Fallback — no frame buffer yet (use near-black, will be replaced next frame)
        let mut path = vg::Path::new();
        path.rect(bounds.x, bounds.y, bounds.w, bounds.h);
        canvas.fill_path(&mut path, &vg::Paint::color(vg::Color::rgb(10, 14, 4)));
    }

    fn event(&mut self, cx: &mut EventContext, _event: &mut Event) {
        cx.needs_redraw();
    }
}
