use crate::ascii_bank::AsciiBank;

/// A single rendered layer
#[derive(Clone, Debug)]
pub struct LayerState {
    /// Index into AsciiBank.images (0 = img01.txt, always anchor)
    pub image_idx: usize,
    /// Blend weight (0.0–1.0)
    pub weight: f32,
    /// Time offset (frames)
    pub time_offset: i32,
    /// Spatial offset (x, y) in grid cells OR viewport crop offset if image > bounds
    pub spatial_offset: (i32, i32),
    /// If true, render with pop effect (1.5x brightness, fade)
    pub pop_highlight: bool,
}

/// Layer engine: manages up to 5 active layers
pub struct LayerEngine {
    layers: Vec<LayerState>,
    rng_state: u64,
}

impl LayerEngine {
    pub fn new() -> Self {
        LayerEngine {
            layers: vec![
                LayerState {
                    image_idx: 0,
                    weight: 1.0,
                    time_offset: 0,
                    spatial_offset: (0, 0),
                    pop_highlight: false,
                };
                5
            ],
            rng_state: 0x123456789ABCDEF,
        }
    }

    /// Update layer states based on audio amplitude and parameters
    /// ANCHOR DESIGN: Layer 0 is ALWAYS img01.txt (36×46 grid base)
    /// Secondary layers (1–4) are overlays with pop highlights
    pub fn update(
        &mut self,
        _rms: f32,
        layer_count: f32,
        _instability: f32,
        transient_active: bool,
        ascii_bank: &AsciiBank,
    ) {
        let active_count = (layer_count.ceil() as usize).min(5).max(1);

        // Layer 0: ALWAYS img01.txt (anchor, no change)
        self.layers[0].image_idx = 0;  // img01.txt
        self.layers[0].weight = 1.0;
        self.layers[0].time_offset = 0;
        self.layers[0].spatial_offset = (0, 0);
        self.layers[0].pop_highlight = false;

        // Secondary layers (1–4): random overlay images with pop highlight
        for i in 1..active_count {
            self.layers[i].image_idx = self.select_overlay_image(i, ascii_bank);
            self.layers[i].weight = (1.0 / (active_count as f32 * 2.0)).min(0.4);
            self.layers[i].time_offset = (i as i32 * 5);
            // Random viewport position for larger images, or small offset for smaller ones
            let offset_x = (((self.lcg_rand() as i32) % 10) as i32) - 5;
            let offset_y = (((self.lcg_rand() as i32) % 10) as i32) - 5;
            self.layers[i].spatial_offset = (offset_x, offset_y);
            self.layers[i].pop_highlight = true;  // Overlay images get pop effect
        }

        // Zero out inactive layers
        for i in active_count..5 {
            self.layers[i].weight = 0.0;
            self.layers[i].pop_highlight = false;
        }

        // On transient: spawn temporary layer spike with extra pop
        if transient_active && active_count < 5 {
            let temp_idx = active_count;
            self.layers[temp_idx].image_idx = self.select_overlay_image(temp_idx, ascii_bank);
            self.layers[temp_idx].weight = 0.5;
            self.layers[temp_idx].time_offset = 0;
            self.layers[temp_idx].pop_highlight = true;
        }
    }

    pub fn layers(&self) -> &[LayerState] {
        &self.layers
    }

    fn select_overlay_image(&mut self, _layer_idx: usize, ascii_bank: &AsciiBank) -> usize {
        // Select overlay image: skip img01 (index 0), choose from the rest
        let rand_val = self.lcg_rand();
        let idx = ((rand_val as usize) % (ascii_bank.len() - 1)) + 1;
        idx.min(ascii_bank.len() - 1)
    }

    fn lcg_rand(&mut self) -> u64 {
        self.rng_state = self.rng_state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.rng_state
    }
}
