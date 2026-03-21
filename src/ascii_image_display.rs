//! Live ASCII art display in Vizia with real-time frame buffer rendering
//!
//! Renders a 36×46 pixel grid driven by the frame buffer.
//! Updates continuously as audio arrives and frame buffer is updated.

use nih_plug_vizia::vizia::prelude::*;
use std::sync::{Arc, Mutex};
use crate::render::FrameBuffer;
use std::time::{SystemTime, UNIX_EPOCH};

/// Displays the live FrameBuffer as a continuously updating pixel grid
pub struct AsciiImageDisplay {
    pub frame_buffer: Arc<Mutex<Option<FrameBuffer>>>,
    last_render_time: std::cell::RefCell<u64>,
}

impl AsciiImageDisplay {
    /// Create 36×46 grid that updates from live frame buffer
    pub fn new(
        cx: &mut Context,
        frame_buffer: Arc<Mutex<Option<FrameBuffer>>>,
    ) -> Handle<'_, Self> {
        let fb_clone = frame_buffer.clone();

        Self {
            frame_buffer,
            last_render_time: std::cell::RefCell::new(0),
        }
        .build(cx, move |cx| {
            // Create static grid structure (36×46 cells)
            for row in 0..46usize {
                HStack::new(cx, {
                    let fb = fb_clone.clone();
                    move |cx| {
                        for col in 0..36usize {
                            let fb = fb.clone();
                            let row_idx = row;
                            let col_idx = col;

                            // Each cell reads live color from frame buffer
                            let pixel_index = (row_idx * 36 + col_idx) as usize;
                            let default_color = Color::rgb(60, 40, 80);

                            let color = if let Ok(fb_lock) = fb.lock() {
                                if let Some(fb_data) = fb_lock.as_ref() {
                                    let pix_idx = pixel_index * 4;
                                    if pix_idx + 3 < fb_data.pixels.len() {
                                        Color::rgb(
                                            fb_data.pixels[pix_idx],
                                            fb_data.pixels[pix_idx + 1],
                                            fb_data.pixels[pix_idx + 2],
                                        )
                                    } else {
                                        default_color
                                    }
                                } else {
                                    default_color
                                }
                            } else {
                                default_color
                            };

                            Element::new(cx)
                                .size(Stretch(1.0))
                                .background_color(color);
                        }
                    }
                })
                .height(Stretch(1.0))
                .row_between(Pixels(0.0))
                .child_space(Pixels(0.0));
            }
        })
        .size(Stretch(1.0))
        .col_between(Pixels(0.0))
        .child_space(Pixels(0.0))
    }
}

impl View for AsciiImageDisplay {}
