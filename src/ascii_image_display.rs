//! Live ASCII art display in Vizia using pixel grid rendering
//!
//! This renders the FrameBuffer as a grid of colored elements.
//! Each pixel from the frame buffer becomes a visual element.

use nih_plug_vizia::vizia::prelude::*;
use std::sync::{Arc, Mutex};
use crate::render::FrameBuffer;

/// Displays a live FrameBuffer as a colored grid in Vizia
pub struct AsciiImageDisplay {
    /// Current frame buffer to display
    pub frame_buffer: Arc<Mutex<Option<FrameBuffer>>>,
}

impl AsciiImageDisplay {
    /// Create a new ASCII image display showing the FrameBuffer
    pub fn new(
        cx: &mut Context,
        frame_buffer: Arc<Mutex<Option<FrameBuffer>>>,
    ) -> Handle<'_, Self> {
        // Clone to capture in the closure
        let fb_clone = frame_buffer.clone();

        // Build the display with dynamic grid based on frame buffer
        Self {
            frame_buffer,
        }
        .build(cx, move |cx| {
            // Create a VStack to hold rows
            VStack::new(cx, {
                let fb = fb_clone.clone();
                move |cx| {
                    // Try to render frame buffer as colored grid
                    if let Ok(frame_buffer_lock) = fb.lock() {
                        if let Some(frame_buffer) = frame_buffer_lock.as_ref() {
                            // Render rows
                            for row in 0..frame_buffer.height {
                                HStack::new(cx, {
                                    let fb = fb.clone();
                                    move |cx| {
                                        if let Ok(frame_buffer_lock) = fb.lock() {
                                            if let Some(frame_buffer) = frame_buffer_lock.as_ref() {
                                                // Render columns
                                                for col in 0..frame_buffer.width {
                                                    let idx = ((row * frame_buffer.width + col) * 4) as usize;
                                                    if idx + 3 < frame_buffer.pixels.len() {
                                                        let r = frame_buffer.pixels[idx];
                                                        let g = frame_buffer.pixels[idx + 1];
                                                        let b = frame_buffer.pixels[idx + 2];

                                                        Element::new(cx)
                                                            .size(Stretch(1.0))
                                                            .background_color(Color::rgb(r, g, b));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                })
                                .height(Stretch(1.0))
                                .row_between(Pixels(0.0))
                                .child_space(Pixels(0.0));
                            }
                        }
                    }
                }
            })
            .size(Stretch(1.0))
            .col_between(Pixels(0.0))
            .child_space(Pixels(0.0));
        })
        .size(Stretch(1.0))
        .background_color(Color::rgb(20, 20, 30))
    }
}

impl View for AsciiImageDisplay {}
