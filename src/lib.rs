use nih_plug::prelude::*;
use std::sync::Arc;

// ─── Machine presets ────────────────────────────────────────────────────────
// Classic rates for reference (used in preset descriptions):
//   SP-1200      26,040 Hz  12-bit
//   Akai S950    39,375 Hz  12-bit
//   Akai S612    31,250 Hz  12-bit
//   MPC3000      44,100 Hz  16-bit (still counts)
//   E-mu SP-12   27,500 Hz  12-bit

// ─── Plugin struct ────────────────────────────────────────────────────────────

struct Sssssssssampler {
    params: Arc<SssssssssamplerParams>,
    sample_rate: f32,
    /// Phase accumulator per channel — when it crosses 1.0 we latch a new sample.
    phase: [f32; 2],
    /// The held (latched) sample value per channel.
    held: [f32; 2],
}

impl Default for Sssssssssampler {
    fn default() -> Self {
        Self {
            params: Arc::new(SssssssssamplerParams::default()),
            sample_rate: 44_100.0,
            phase: [1.0; 2], // start at 1.0 so the very first sample is latched immediately
            held: [0.0; 2],
        }
    }
}

// ─── Parameters ──────────────────────────────────────────────────────────────

#[derive(Params)]
struct SssssssssamplerParams {
    /// The effective playback sample rate.
    /// SP-1200 ≈ 26 kHz, S950 ≈ 39 kHz, S612 ≈ 31 kHz, SP-12 ≈ 27.5 kHz.
    #[id = "target_sr"]
    pub target_sr: FloatParam,

    /// Bit depth for quantisation.  12-bit = MPC/SP magic, lower = absolute carnage.
    #[id = "bit_depth"]
    pub bit_depth: FloatParam,

    /// Adds random timing jitter to the sample-clock — dirtier/more organic.
    #[id = "jitter"]
    pub jitter: FloatParam,

    /// Dry/wet blend.
    #[id = "mix"]
    pub mix: FloatParam,
}

impl Default for SssssssssamplerParams {
    fn default() -> Self {
        Self {
            target_sr: FloatParam::new(
                "Sample Rate",
                26_040.0, // SP-1200 out of the box
                FloatRange::Skewed {
                    min: 1_000.0,
                    max: 96_000.0,
                    // skew lets you spend more resolution in the lo-fi zone
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
            // fractional bit depths sound wild — keep the resolution
            .with_step_size(0.1)
            .with_unit(" bit")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),

            jitter: FloatParam::new(
                "Jitter",
                0.0,
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
        }
    }
}

// ─── DSP helpers ─────────────────────────────────────────────────────────────

/// Quantise `sample` to `bits` bits of resolution.
/// With 12 bits: 2^11 = 2048 steps per side → matches classic 12-bit machines.
#[inline]
fn crush(sample: f32, bits: f32) -> f32 {
    // Allow fractional bits for extra weirdness
    let levels = (2.0_f32).powf(bits - 1.0);
    (sample * levels).round() / levels
}

/// Cheap deterministic pseudo-random float in [-1, 1] from a u64 seed.
/// LCG — good enough for jitter, costs nothing.
#[inline]
fn lcg_rand(state: &mut u64) -> f32 {
    *state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
    let bits = 0x3F80_0000u32 | ((*state >> 41) as u32 & 0x007F_FFFF);
    (f32::from_bits(bits) - 1.5) * 2.0 // [-1, 1]
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

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
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
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let host_sr = self.sample_rate;
        let mut rng_state: u64 = 0x123456789ABCDEF;

        for channel_samples in buffer.iter_samples() {
            let target_sr = self.params.target_sr.smoothed.next();
            let bit_depth = self.params.bit_depth.smoothed.next();
            let jitter = self.params.jitter.smoothed.next();
            let mix = self.params.mix.smoothed.next();

            // Nominal step; clamp to 1.0 so turning SR all the way up = bypass
            let step = (target_sr / host_sr).min(1.0);

            for (ch, sample) in channel_samples.into_iter().enumerate() {
                let ch = ch.min(1);
                let dry = *sample;

                // ── Sample-and-hold SR reduction ──────────────────────────
                // Check phase BEFORE advancing so the very first sample latches.
                if self.phase[ch] >= 1.0 {
                    self.phase[ch] -= 1.0;
                    self.held[ch] = dry;
                }

                // Jitter: randomly nudge the phase accumulator a little so the
                // sample clock wobbles — gives that organic, slightly-drunk feel
                // of aging electrolytic caps in a 40 year old drum machine.
                let jitter_amount = jitter * step * 0.5 * lcg_rand(&mut rng_state);
                self.phase[ch] += step + jitter_amount;

                // ── Bit crushing ──────────────────────────────────────────
                let crushed = crush(self.held[ch], bit_depth);

                // ── Dry/wet ───────────────────────────────────────────────
                *sample = dry + (crushed - dry) * mix;
            }
        }

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

// ─── VST3 metadata ────────────────────────────────────────────────────────────

impl Vst3Plugin for Sssssssssampler {
    // 16-byte unique class ID — change this if you fork the plugin
    const VST3_CLASS_ID: [u8; 16] = *b"sssssssssampleer";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Filter];
}

// ─── Exports ─────────────────────────────────────────────────────────────────
// Export both CLAP (preferred) and VST3.
nih_export_clap!(Sssssssssampler);
nih_export_vst3!(Sssssssssampler);
