/// Frame data ready for display (RGBA8 format)
#[derive(Clone)]
pub struct FrameBuffer {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>, // RGBA format, width × height × 4 bytes
    /// Theme background color in sRGB [R, G, B] for canvas fill
    pub bg_rgb: [u8; 3],
    /// Theme primary (pop) color in sRGB for UI title
    pub primary_rgb: [u8; 3],
    /// Theme emphasis color in sRGB for UI text
    pub emphasis_rgb: [u8; 3],
    /// Current preset index (for UI display)
    pub preset_idx: u8,
    /// Current theme index (for UI display)
    pub theme_idx: u8,
    /// Smoothed energy for UI brightness modulation
    pub energy: f32,
    /// True when current theme has a light background
    pub is_light: bool,
    /// Current feel preset index (0=Tight, 1=Expressive, 2=Chaotic)
    pub feel_idx: u8,
    /// Dark mode active
    pub dark_mode: bool,
    // ── V6: Bridge fields for display ──
    /// Effective BPM for beat-synced title
    pub bpm: f32,
    /// Sub-bass energy for breathing effect
    pub sub_bass_energy: f32,
    /// Transient active for title flash
    pub transient: bool,
}

impl FrameBuffer {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![0u8; (width * height * 4) as usize],
            bg_rgb: [0, 0, 0],
            primary_rgb: [200, 200, 200],
            emphasis_rgb: [180, 180, 180],
            preset_idx: 2,
            theme_idx: 1,
            energy: 0.0,
            is_light: false,
            feel_idx: 1,
            dark_mode: true,
            bpm: 120.0,
            sub_bass_energy: 0.0,
            transient: false,
        }
    }
}
