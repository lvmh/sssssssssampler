use nih_plug::prelude::*;
use nih_plug_vizia::ViziaState;
use std::sync::Arc;

mod editor;
mod editor_view;
mod ascii_grid_view;
mod ascii_image_display;
mod render;
mod parameter_remapping;
mod audio_feed;
mod anim_state;
pub mod ascii_bank;
pub use parameter_remapping::*;
pub use audio_feed::AnimationParams;

// ─── Plugin struct ────────────────────────────────────────────────────────────

struct Sssssssssampler {
    params: Arc<SssssssssamplerParams>,
    sample_rate: f32,
    phase: [f32; 2],
    held: [f32; 2],
    filter: FilterState,
    last_filter_step: f32,
    last_filter_cutoff: f32,
    last_filter_poles: i32,
    audio_feed: audio_feed::AudioFeed,
}

impl Default for Sssssssssampler {
    fn default() -> Self {
        Self {
            params: Arc::new(SssssssssamplerParams::default()),
            sample_rate: 44_100.0,
            phase: [1.0; 2],
            held: [0.0; 2],
            filter: FilterState::new(),
            last_filter_step: -1.0,
            last_filter_cutoff: -1.0,
            last_filter_poles: -1,
            audio_feed: audio_feed::AudioFeed::default(),
        }
    }
}

// ─── Parameters ──────────────────────────────────────────────────────────────

#[derive(Params)]
pub struct SssssssssamplerParams {
    #[persist = "editor-state"]
    pub editor_state: Arc<ViziaState>,

    #[id = "target_sr"]
    pub target_sr: FloatParam,

    #[id = "bit_depth"]
    pub bit_depth: FloatParam,

    #[id = "jitter"]
    pub jitter: FloatParam,

    #[id = "mix"]
    pub mix: FloatParam,

    /// Filter cutoff as a fraction of target Nyquist.
    /// 1.0 = fully open (tracks the machine's own Nyquist — transparent reconstruction).
    /// Sweep down to add machine-specific filter character.
    #[id = "filter_cutoff"]
    pub filter_cutoff: FloatParam,

    /// Filter pole count — 2.0 = 2-pole LP (SP-1200/SP-12 style),
    /// 4.0 = 4th-order Butterworth (S950/S612/MPC3000/SP-303 style).
    #[id = "filter_poles"]
    pub filter_poles: FloatParam,

    /// Enable anti-aliasing reconstruction filter
    /// When ON: applies aggressive LP filtering at Nyquist to prevent aliasing
    /// When OFF: allows raw aliasing (lo-fi aesthetic)
    #[id = "anti_alias"]
    pub anti_alias: BoolParam,
}

impl Default for SssssssssamplerParams {
    fn default() -> Self {
        Self {
            editor_state: editor::default_state(),

            target_sr: FloatParam::new(
                "Sample Rate",
                39_375.0,
                FloatRange::Skewed {
                    min: 1_000.0,
                    max: 96_000.0,
                    factor: FloatRange::skew_factor(-2.0),
                },
            )
            .with_smoother(SmoothingStyle::Logarithmic(30.0))
            .with_unit(" Hz")
            .with_value_to_string(formatters::v2s_f32_rounded(0)),

            bit_depth: FloatParam::new(
                "Bit Depth",
                12.0,
                FloatRange::Linear { min: 1.0, max: 24.0 },
            )
            .with_smoother(SmoothingStyle::Linear(30.0))
            .with_step_size(0.1)
            .with_unit(" bit")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),

            jitter: FloatParam::new(
                "Jitter",
                0.01,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(30.0))
            .with_unit("%")
            .with_value_to_string(formatters::v2s_f32_percentage(1))
            .with_string_to_value(formatters::s2v_f32_percentage()),

            mix: FloatParam::new(
                "Mix",
                1.0,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(5.0))
            .with_unit("%")
            .with_value_to_string(formatters::v2s_f32_percentage(1))
            .with_string_to_value(formatters::s2v_f32_percentage()),

            filter_cutoff: FloatParam::new(
                "Filter Cutoff",
                1.0,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_unit("%")
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage()),

            filter_poles: FloatParam::new(
                "Filter Poles",
                4.0,
                FloatRange::Linear { min: 2.0, max: 4.0 },
            )
            .with_step_size(2.0)
            .with_value_to_string(formatters::v2s_f32_rounded(0))
            .with_unit("-pole"),

            anti_alias: BoolParam::new("Anti-Aliasing", true),
        }
    }
}

// ─── DSP helpers ─────────────────────────────────────────────────────────────

#[inline]
fn crush(sample: f32, bits: f32) -> f32 {
    let levels = (2.0_f32).powf(bits - 1.0);
    (sample * levels).round() / levels
}

#[inline]
fn lcg_rand(state: &mut u64) -> f32 {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    let bits = 0x3F80_0000u32 | ((*state >> 41) as u32 & 0x007F_FFFF);
    (f32::from_bits(bits) - 1.5) * 2.0
}

// ─── Biquad filter (Direct Form II transposed) ────────────────────────────────

struct BiquadState {
    b: [f32; 3],
    a: [f32; 2],
    z: [[f32; 2]; 2], // per stereo channel
}

impl BiquadState {
    fn new() -> Self {
        Self { b: [1.0, 0.0, 0.0], a: [0.0, 0.0], z: [[0.0; 2]; 2] }
    }

    /// Audio EQ Cookbook bilinear LPF. fc_norm ∈ (0, 1) where 1 = Nyquist.
    fn update(&mut self, fc_norm: f32, q: f32) {
        let w     = std::f32::consts::PI * fc_norm.clamp(0.001, 0.999);
        let cos_w = w.cos();
        let sin_w = w.sin();
        let alpha = sin_w / (2.0 * q);
        let a0    = 1.0 / (1.0 + alpha);
        self.b[0] = (1.0 - cos_w) * 0.5 * a0;
        self.b[1] = (1.0 - cos_w) * a0;
        self.b[2] = self.b[0];
        self.a[0] = -2.0 * cos_w * a0;
        self.a[1] = (1.0 - alpha) * a0;
    }

    #[inline]
    fn process(&mut self, x: f32, ch: usize) -> f32 {
        let y = self.b[0] * x + self.z[ch][0];
        self.z[ch][0] = self.b[1] * x - self.a[0] * y + self.z[ch][1];
        self.z[ch][1] = self.b[2] * x - self.a[1] * y;
        y
    }

    fn reset(&mut self) { self.z = [[0.0; 2]; 2]; }
}

// ─── Filter topology ──────────────────────────────────────────────────────────
//
// 2-pole (SP-1200/SP-12): single biquad, Q = 1/√2 ≈ 0.7071 (Butterworth 2nd order)
// 4-pole (S950/S612/MPC3000/SP-303): two biquad cascade (4th-order Butterworth)
//   Stage 1 Q = 1/(2·sin π/8)  ≈ 1.3066
//   Stage 2 Q = 1/(2·sin 3π/8) ≈ 0.5412

struct FilterState {
    stage1: BiquadState,
    stage2: BiquadState,
}

impl FilterState {
    fn new() -> Self {
        Self { stage1: BiquadState::new(), stage2: BiquadState::new() }
    }

    fn update(&mut self, step: f32, cutoff: f32, poles: i32) {
        let fc = (step * cutoff).min(0.99);
        if poles >= 4 {
            self.stage1.update(fc, 1.3066);
            self.stage2.update(fc, 0.5412);
        } else {
            self.stage1.update(fc, 0.7071);
        }
    }

    #[inline]
    fn process(&mut self, x: f32, ch: usize, poles: i32) -> f32 {
        let y = self.stage1.process(x, ch);
        if poles >= 4 { self.stage2.process(y, ch) } else { y }
    }

    fn reset(&mut self) {
        self.stage1.reset();
        self.stage2.reset();
    }
}

// ─── Plugin impl ─────────────────────────────────────────────────────────────

impl Plugin for Sssssssssampler {
    const NAME: &'static str = "sssssssssampler";
    const VENDOR: &'static str = "sssssssssampler";
    const URL: &'static str = "https://github.com/normieai/sssssssssampler";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(1),
            main_output_channels: NonZeroU32::new(1),
            ..AudioIOLayout::const_default()
        },
    ];

    const MIDI_INPUT: MidiConfig = MidiConfig::None;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::None;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> { self.params.clone() }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        editor::create(
            self.params.clone(),
            self.params.editor_state.clone(),
            self.audio_feed.shared_params.clone(),
        )
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.sample_rate = buffer_config.sample_rate;
        true
    }

    fn reset(&mut self) {
        self.phase = [1.0; 2];
        self.held = [0.0; 2];
        self.filter.reset();
        self.last_filter_step = -1.0;
        self.last_filter_cutoff = -1.0;
        self.last_filter_poles = -1;
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let host_sr = self.sample_rate;
        let mut rng_state: u64 = 0x123456789ABCDEF;

        // Pole count is discrete — read once per block, not per sample.
        let poles = self.params.filter_poles.value().round() as i32;
        if poles != self.last_filter_poles {
            self.filter.reset();
            self.last_filter_poles  = poles;
            self.last_filter_step   = -1.0;
            self.last_filter_cutoff = -1.0;
        }

        // Collect current parameter values for audio feed
        let target_sr     = self.params.target_sr.value();
        let bit_depth     = self.params.bit_depth.value();
        let jitter        = self.params.jitter.value();

        for channel_samples in buffer.iter_samples() {
            let target_sr_smooth  = self.params.target_sr.smoothed.next();
            let bit_depth_smooth  = self.params.bit_depth.smoothed.next();
            let jitter_smooth     = self.params.jitter.smoothed.next();
            let mix           = self.params.mix.smoothed.next();
            let filter_cutoff = self.params.filter_cutoff.smoothed.next();

            let step = (target_sr_smooth / host_sr).min(1.0);

            // Lazy coefficient update when SR or cutoff shifts meaningfully.
            if (step - self.last_filter_step).abs() > 0.0002
                || (filter_cutoff - self.last_filter_cutoff).abs() > 0.001
            {
                self.filter.update(step, filter_cutoff, poles);
                self.last_filter_step   = step;
                self.last_filter_cutoff = filter_cutoff;
            }

            for (ch, sample) in channel_samples.into_iter().enumerate() {
                let ch  = ch.min(1);
                let dry = *sample;

                // ── Sample-and-hold ────────────────────────────────────────
                if self.phase[ch] >= 1.0 {
                    self.phase[ch] -= 1.0;
                    self.held[ch] = dry;
                }
                let jitter_amount = jitter_smooth * step * 0.5 * lcg_rand(&mut rng_state);
                self.phase[ch] += step + jitter_amount;

                // ── Bit crush ──────────────────────────────────────────────
                let mut wet = crush(self.held[ch], bit_depth_smooth);

                // ── Reconstruction filter ──────────────────────────────────
                let anti_alias_enabled = self.params.anti_alias.value();

                // Apply filter normally if user is controlling it (cutoff < 100%)
                if filter_cutoff < 1.0 {
                    wet = self.filter.process(wet, ch, poles);
                } else if anti_alias_enabled {
                    // At 100% cutoff with anti-aliasing ON, apply gentle filtering at Nyquist
                    // to prevent aliasing (use ~0.95 effective cutoff)
                    let gentle_cutoff = 0.95;
                    let gentle_step = (target_sr_smooth / host_sr).min(1.0);
                    self.filter.update(gentle_step, gentle_cutoff, poles);
                    wet = self.filter.process(wet, ch, poles);
                }
                // If anti_alias is OFF and cutoff is 100%, no filter applied (raw aliasing)

                // ── Dry/wet ────────────────────────────────────────────────
                let output = dry + (wet - dry) * mix;
                *sample = output;

                // ── Feed audio analyzer ────────────────────────────────────
                self.audio_feed.push_sample(output);
            }
        }

        // Update animation parameters with current DSP values
        self.audio_feed.update(target_sr, bit_depth, jitter);

        ProcessStatus::Normal
    }
}

// ─── CLAP metadata ───────────────────────────────────────────────────────────

impl ClapPlugin for Sssssssssampler {
    const CLAP_ID: &'static str = "com.sssssssssampler.sssssssssampler";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("SP-1200 / S950 style sample rate & bit depth reducer");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Stereo,
        ClapFeature::Mono,
        ClapFeature::Utility,
    ];
}

impl Vst3Plugin for Sssssssssampler {
    const VST3_CLASS_ID: [u8; 16] = *b"sssssssssampleer";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Filter];
}

nih_export_clap!(Sssssssssampler);
nih_export_vst3!(Sssssssssampler);
