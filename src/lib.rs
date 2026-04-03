use nih_plug::prelude::*;
use nih_plug_vizia::ViziaState;
use std::sync::atomic::AtomicU8;
use std::sync::Arc;

mod editor;
mod ascii_image_display;
mod render;
mod parameter_remapping;
mod audio_feed;
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
    last_filter_cutoff: f32,
    last_filter_poles: i32,
    audio_feed: audio_feed::AudioFeed,
    // ── Hardware emulation state ──
    drift_phase: [f32; 2],   // per-channel clock drift LFO (slightly different rates)
    quant_error: [f32; 2],   // noise-shaping error per channel
    prev_dac: [f32; 2],      // DAC output stage previous sample
    pre_filter: FilterState, // bandwidth/AA pre-filter (pole count matches machine)
    dc_x: [f32; 2],          // DC blocker: previous input sample
    dc_y: [f32; 2],          // DC blocker: previous output sample
    jitter_walk: [f32; 2],   // correlated (Brownian) clock jitter state per channel
    rng_state: u64,          // persistent LCG state — must not reset per-block
}

impl Default for Sssssssssampler {
    fn default() -> Self {
        Self {
            params: Arc::new(SssssssssamplerParams::default()),
            sample_rate: 44_100.0,
            phase: [1.0; 2],
            held: [0.0; 2],
            filter: FilterState::new(),
            last_filter_cutoff: -1.0,
            last_filter_poles: -1,
            audio_feed: audio_feed::AudioFeed::default(),
            drift_phase: [0.0; 2],
            quant_error: [0.0; 2],
            prev_dac: [0.0; 2],
            pre_filter: FilterState::new(),
            dc_x: [0.0; 2],
            dc_y: [0.0; 2],
            jitter_walk: [0.0; 2],
            rng_state: 0x123456789ABCDEF,
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
    /// 4.0 = 4th-order Butterworth (S612/MPC3000/SP-303 style),
    /// 6.0 = 6th-order Butterworth (S950 — 36 dB/oct switched-capacitor).
    #[id = "filter_poles"]
    pub filter_poles: FloatParam,

    /// Enable anti-aliasing reconstruction filter
    /// When ON: applies aggressive LP filtering at Nyquist to prevent aliasing
    /// When OFF: allows raw aliasing (lo-fi aesthetic)
    #[id = "anti_alias"]
    pub anti_alias: BoolParam,

    /// Persisted theme index (0–13)
    #[persist = "theme-id"]
    pub theme_id: Arc<AtomicU8>,

    /// Persisted dark mode flag (0 = light, 1 = dark)
    #[persist = "dark-mode"]
    pub dark_mode_persisted: Arc<AtomicU8>,
}

impl Default for SssssssssamplerParams {
    fn default() -> Self {
        Self {
            editor_state: editor::default_state(),

            // ── Perceptual parameter mappings ──────────────────────────────────
            //
            // Every parameter uses nonlinear (skewed) ranges tuned so that:
            //   • The musically useful zone sits in the middle of the knob
            //   • Extreme/transparent ends are compressed to small travel
            //   • Smoothers match the perceptual domain (log for freq, exp for intensity)

            target_sr: FloatParam::new(
                "Bandwidth",
                26_040.0,
                FloatRange::Skewed {
                    min: 4_000.0,
                    max: 48_000.0,
                    // -2.0 skew: mid-knob sits ~12kHz (vintage sweet spot).
                    // 48kHz ceiling covers S950 (48kHz max) and all other machines.
                    factor: FloatRange::skew_factor(-2.0),
                },
            )
            .with_smoother(SmoothingStyle::Logarithmic(50.0))
            .with_unit(" Hz")
            .with_value_to_string(formatters::v2s_f32_rounded(0)),

            bit_depth: FloatParam::new(
                "Bit Depth",
                12.0,
                FloatRange::Skewed {
                    min: 1.0,
                    max: 24.0,
                    // Negative skew expands 4–12 bit zone where reduction is audible;
                    // 14–24 bit (perceptually identical) compresses to top of range
                    factor: FloatRange::skew_factor(-1.5),
                },
            )
            .with_smoother(SmoothingStyle::Exponential(40.0))
            .with_step_size(1.0)
            .with_unit(" bit")
            .with_value_to_string(formatters::v2s_f32_rounded(0)),

            jitter: FloatParam::new(
                "Jitter",
                0.01,
                FloatRange::Skewed {
                    min: 0.0,
                    max: 1.0,
                    // Positive skew: most knob travel in subtle jitter range (0–0.1),
                    // extreme chaos compressed to top end
                    factor: FloatRange::skew_factor(1.5),
                },
            )
            .with_smoother(SmoothingStyle::Exponential(25.0))
            .with_unit("%")
            .with_value_to_string(formatters::v2s_f32_percentage(1))
            .with_string_to_value(formatters::s2v_f32_percentage()),

            mix: FloatParam::new(
                "Mix",
                1.0,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(15.0))
            .with_unit("%")
            .with_value_to_string(formatters::v2s_f32_percentage(1))
            .with_string_to_value(formatters::s2v_f32_percentage()),

            filter_cutoff: FloatParam::new(
                "Filter Cutoff",
                22_050.0,
                FloatRange::Skewed {
                    min: 200.0,
                    max: 22_050.0,
                    // -2.0 skew: logarithmic feel matching human pitch perception.
                    // Mid-knob sits ~2kHz. Floor at 200Hz allows extreme alias sculpting
                    // when AA is off — low enough to mangle, high enough to not go silent.
                    factor: FloatRange::skew_factor(-2.0),
                },
            )
            .with_smoother(SmoothingStyle::Logarithmic(15.0))
            .with_unit(" Hz")
            .with_value_to_string(formatters::v2s_f32_rounded(0)),

            filter_poles: FloatParam::new(
                "Filter Poles",
                6.0,
                FloatRange::Linear { min: 2.0, max: 6.0 },
            )
            .with_step_size(2.0)
            .with_value_to_string(formatters::v2s_f32_rounded(0))
            .with_unit("-pole"),

            anti_alias: BoolParam::new("Anti-Aliasing", false),

            // Kanagawa dark (theme 8, dark=1)
            theme_id: Arc::new(AtomicU8::new(8)),
            dark_mode_persisted: Arc::new(AtomicU8::new(1)),
        }
    }
}

// ─── DSP helpers ─────────────────────────────────────────────────────────────

/// Bit crush with first-order noise shaping (reduces audible quantization noise)
#[inline]
fn crush_shaped(sample: f32, bits: f32, error: &mut f32) -> f32 {
    let levels = (2.0_f32).powf(bits - 1.0);
    let shaped = sample + *error * 0.5;
    let quantized = (shaped * levels).round() / levels;
    *error = sample - quantized;
    quantized
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
        // Guard against NaN/inf from filter instability during rapid param changes
        if !y.is_finite() {
            self.z[ch] = [0.0; 2];
            return x;
        }
        y
    }

    /// Process with subtle per-stage op-amp saturation (for cascaded stages 2+)
    #[inline]
    fn process_saturated(&mut self, x: f32, ch: usize) -> f32 {
        let y = self.process(x, ch);
        let sat = y - y * y * y * 0.02;
        if !sat.is_finite() { return x; }
        sat
    }

    fn reset(&mut self) { self.z = [[0.0; 2]; 2]; }
}

// ─── Filter topology ──────────────────────────────────────────────────────────
//
// 2-pole (SP-1200/SP-12): single biquad, Q = 1/√2 ≈ 0.7071 (Butterworth 2nd order)
// 4-pole (S612/MPC3000/SP-303): two biquad cascade (4th-order Butterworth)
//   Stage 1 Q = 1/(2·sin π/8)  ≈ 1.3066
//   Stage 2 Q = 1/(2·sin 3π/8) ≈ 0.5412
// 6-pole (S950): three biquad cascade (6th-order Butterworth, 36 dB/oct)
//   Stage 1 Q = 1/(2·sin π/12)  ≈ 1.9319
//   Stage 2 Q = 1/(2·sin 3π/12) ≈ 1.0000 (= 1/2·sin(π/4))
//   Stage 3 Q = 1/(2·sin 5π/12) ≈ 0.5176

struct FilterState {
    stage1: BiquadState,
    stage2: BiquadState,
    stage3: BiquadState,
}

impl FilterState {
    fn new() -> Self {
        Self {
            stage1: BiquadState::new(),
            stage2: BiquadState::new(),
            stage3: BiquadState::new(),
        }
    }

    fn update(&mut self, cutoff: f32, poles: i32) {
        let fc = cutoff.clamp(0.001, 0.99);
        if poles >= 6 {
            // 6th-order Butterworth: Q = 1/(2·sin(kπ/12)) for k = 1, 3, 5
            self.stage1.update(fc, 1.9319);
            self.stage2.update(fc, 0.7071);
            self.stage3.update(fc, 0.5176);
        } else if poles >= 4 {
            self.stage1.update(fc, 1.3066);
            self.stage2.update(fc, 0.5412);
        } else {
            self.stage1.update(fc, 0.7071);
        }
    }

    #[inline]
    fn process(&mut self, x: f32, ch: usize, poles: i32) -> f32 {
        let y = self.stage1.process(x, ch);
        if poles >= 6 {
            let y2 = self.stage2.process_saturated(y, ch);
            self.stage3.process_saturated(y2, ch)
        } else if poles >= 4 {
            self.stage2.process_saturated(y, ch)
        } else {
            y
        }
    }

    fn reset(&mut self) {
        self.stage1.reset();
        self.stage2.reset();
        self.stage3.reset();
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
        self.audio_feed.host_sr = buffer_config.sample_rate;
        true
    }

    fn reset(&mut self) {
        self.phase = [1.0; 2];
        self.held = [0.0; 2];
        self.filter.reset();
        self.pre_filter.reset();
        self.last_filter_cutoff = -1.0;
        self.last_filter_poles = -1;
        self.drift_phase = [0.0; 2];
        self.quant_error = [0.0; 2];
        self.prev_dac = [0.0; 2];
        self.dc_x = [0.0; 2];
        self.dc_y = [0.0; 2];
        self.jitter_walk = [0.0; 2];
        self.rng_state = 0x123456789ABCDEF;
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let host_sr = self.sample_rate;

        // Pole count is discrete — read once per block, not per sample.
        let poles = self.params.filter_poles.value().round() as i32;
        if poles != self.last_filter_poles {
            self.filter.reset();
            self.pre_filter.reset();
            self.last_filter_poles  = poles;
            self.last_filter_cutoff = -1.0;
        }

        // Collect current parameter values for audio feed
        let target_sr          = self.params.target_sr.value();
        let bit_depth          = self.params.bit_depth.value();
        let jitter             = self.params.jitter.value();
        let anti_alias_enabled = self.params.anti_alias.value();

        // S&H capacitor droop: charge leakage during hold phase.
        // 40ms time constant — inaudible at host rate but adds organic sag on pads/tones.
        let droop = (-1.0_f32 / (0.04 * host_sr)).exp();
        // DC blocker coefficient: first-order HPF at ~15 Hz removes accumulated DC
        // from asymmetric ADC nonlinearity. Computed once — host_sr never changes mid-block.
        let dc_r = 1.0 - (std::f32::consts::TAU * 15.0 / host_sr);

        for channel_samples in buffer.iter_samples() {
            let target_sr_smooth  = self.params.target_sr.smoothed.next();
            let bit_depth_smooth  = self.params.bit_depth.smoothed.next();
            let jitter_smooth     = self.params.jitter.smoothed.next();
            let mix           = self.params.mix.smoothed.next();
            // filter_cutoff is in Hz; convert to fc_norm ∈ (0,1) re: host Nyquist.
            let filter_cutoff_hz = self.params.filter_cutoff.smoothed.next();
            let filter_cutoff = (filter_cutoff_hz / (host_sr * 0.5)).clamp(0.001, 0.999);

            // AA ON  → bandwidth mode: SR knob sets filter cutoff, S&H runs at host rate
            //          (step=1.0 = transparent S&H, AA filter does all the work)
            // AA OFF → sample reduction mode: SR knob sets S&H rate, no AA filter
            let bandwidth_fc = (target_sr_smooth / host_sr * 0.9).min(0.99);
            let step = if anti_alias_enabled { 1.0 } else { (target_sr_smooth / host_sr).min(1.0) };

            // ── Reconstruction filter — independent of AA, only updates when cutoff changes.
            if (filter_cutoff - self.last_filter_cutoff).abs() > 0.001 {
                self.filter.update(filter_cutoff, poles);
                self.last_filter_cutoff = filter_cutoff;
            }

            // ── Bandwidth filter (AA on only) — tracks SR knob, machine-matched slope ──
            // Pre-filter uses the same pole count as the reconstruction filter so the
            // AA input roll-off character matches the machine (S950: 6-pole, etc.)
            if anti_alias_enabled {
                self.pre_filter.update(bandwidth_fc, poles);
            }

            for (ch, sample) in channel_samples.into_iter().enumerate() {
                let ch  = ch.min(1);
                let dry = *sample;

                // ── Per-channel clock drift LFO ──────────────────────
                // L: ~0.29 Hz, R: ~0.31 Hz — slightly different rates create
                // subtle analog stereo widening without any correlation artifacts.
                let drift_hz = if ch == 0 { 0.29_f32 } else { 0.31_f32 };
                self.drift_phase[ch] += drift_hz / host_sr;
                if self.drift_phase[ch] > 1.0 { self.drift_phase[ch] -= 1.0; }
                let drift = (self.drift_phase[ch] * std::f32::consts::TAU).sin() * 0.0005;

                // ── Bandwidth/AA pre-filter ──────────────────────────
                let input = if anti_alias_enabled {
                    self.pre_filter.process(dry, ch, poles)
                } else {
                    dry
                };

                // ── Sample-and-hold with clock drift + jitter ────────
                if self.phase[ch] >= 1.0 {
                    self.phase[ch] -= 1.0;
                    self.held[ch] = input;
                }
                // Capacitor droop: held charge leaks every sample
                self.held[ch] *= droop;
                // Correlated (Brownian) jitter — random walk with ~15ms correlation time.
                // Real crystal oscillators wander rather than jumping: each sample's
                // phase error is correlated with the previous one. α=0.9985 gives a
                // 3dB point at ~10Hz (1/(2π·667 samples at 44.1kHz)).
                // Scale 0.000164 = 0.003/18.26 normalises walk σ to match old amplitude.
                self.jitter_walk[ch] = 0.9985 * self.jitter_walk[ch]
                    + lcg_rand(&mut self.rng_state);
                let jitter_amount = jitter_smooth * step * 0.000164 * self.jitter_walk[ch];
                self.phase[ch] += step * (1.0 + drift) + jitter_amount;

                // ── ADC nonlinearity — asymmetric soft-clip ───────────
                // Cubic term = odd-harmonic soft-clip (3rd harmonic grit).
                // s·|s| term = even-harmonic asymmetry (2nd harmonic warmth).
                // Amounts are machine-specific: SP-1200 (2-pole) is famously
                // saturated; S950 (6-pole) was designed to be comparatively clean.
                let s = self.held[ch];
                let (sat_cubic, sat_even) = match poles {
                    p if p >= 6 => (0.010, 0.004), // S950: clean, surgical
                    p if p >= 4 => (0.018, 0.008), // MPC/SP-303: reference warmth
                    _           => (0.028, 0.015), // SP-1200: crunchy, saturated
                };
                let adc_out = s - s * s * s * sat_cubic + s * s.abs() * sat_even;

                // ── Bit crush ─────────────────────────────────────────
                let mut wet = crush_shaped(adc_out, bit_depth_smooth, &mut self.quant_error[ch]);

                // ── Reconstruction filter (machine character) ─────────
                wet = self.filter.process(wet, ch, poles);

                // ── DAC output stage (after filter, light one-pole) ───
                // Models output capacitor on the analog output stage.
                // Moved post-filter and lightened (0.12 → fc ≈ 33kHz at 44.1kHz)
                // so it no longer competes with the reconstruction filter.
                self.prev_dac[ch] = 0.88 * wet + 0.12 * self.prev_dac[ch];
                wet = self.prev_dac[ch];

                // ── DC block (~15 Hz HPF) ─────────────────────────────
                // Removes DC offset accumulated from asymmetric ADC nonlinearity.
                let dc_in = wet;
                wet = dc_in - self.dc_x[ch] + dc_r * self.dc_y[ch];
                self.dc_x[ch] = dc_in;
                self.dc_y[ch] = wet;


                // ── Dry/wet ───────────────────────────────────────────
                let output = dry + (wet - dry) * mix;
                *sample = if output.is_finite() { output } else { dry };

                // ── Feed audio analyzer ───────────────────────────────
                self.audio_feed.push_sample(output, ch);
            }
        }

        // Get BPM and play state from host transport
        let transport = _context.transport();
        let host_bpm = transport.tempo.unwrap_or(120.0) as f32;
        let host_playing = transport.playing;

        // Update animation parameters with current DSP values + BPM + play state
        self.audio_feed.update(target_sr, bit_depth, jitter, host_bpm, host_playing);

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
