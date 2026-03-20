# Filter Research — sssssssssampler

Per-machine anti-aliasing and reconstruction filter characteristics, and how they're implemented in the plugin.

---

## Machine Specs

| Machine | Sample Rate | Bit Depth | Filter Type | Cutoff (% of Nyquist) |
|---|---|---|---|---|
| E-mu SP-1200 | 26,040 Hz | 12-bit | 2-pole simple LP | ~37% |
| Akai S950 | 39,375 Hz | 12-bit | 4-pole Butterworth | ~90% |
| Akai S612 | 31,250 Hz | 12-bit | 4-pole Butterworth-ish | ~75% |
| E-mu SP-12 | 27,500 Hz | 12-bit | 2-pole (SP-1200 lineage) | ~45% |
| Akai MPC3000 | 44,100 Hz | 16-bit | High-order FIR | ~99% |
| Boss SP-303 | 32,000 Hz | 12-bit | Sigma-delta DAC | ~95% |

---

## Per-Machine Detail

### E-mu SP-1200
The SP-1200's most distinctive feature is its **under-filtering**. The analog lowpass cuts at roughly 37% of its own Nyquist (~9.5 kHz), well below what's needed to suppress aliasing. Alias components fold back into the audible range and become part of the sound — the "grit" and pitch-dependent crunch SP-1200 users describe. The filter itself is a simple 2-pole (RC or op-amp) design with no resonance and ~12 dB/octave rolloff. This is not a flaw; it's the character.

### Akai S950
A 4-pole Butterworth (maximally flat) lowpass, cutoff at ~90% of Nyquist. Butterworth means no passband ripple and no resonance peak — just a smooth, gradual rolloff above the cutoff. The S950 sounds clean relative to the SP-1200 because aliasing is suppressed, but there's a slight high-frequency softness (loss of air above ~17–18 kHz at 44.1 kHz) that producers associate with warmth. In Lo-Fi mode (22.05 kHz) the cutoff drops proportionally and becomes very audible.

### Akai S612
Akai's first 12-bit sampler. Supports selectable rates (15, 20, 24, 30 kHz). The filter is believed to be a 4-pole design similar to the S950 lineage, but less well-documented. At lower rates (15 kHz) the cutoff drops into the mid-range and becomes tonally significant. Approximated at 75% of Nyquist.

### E-mu SP-12
Predecessor to the SP-1200, sharing E-mu's DSP chip lineage. Filter is similarly simple (2-pole) but the cutoff sits slightly higher at ~45% of Nyquist, making it marginally less gritty than the SP-1200. Still allows significant aliasing foldback.

### Akai MPC3000
Designed by Roger Linn for fidelity, not color. Runs at 44.1 kHz with a high-order oversampling filter (likely 8th order or linear-phase FIR in the custom VLSI chip). Effectively transparent — no audible filter character. Represented in the plugin at 99% of Nyquist with the filter on, though in practice the MPC3000 sound comes from the sample rate and bit depth, not any analog coloration.

### Boss SP-303
Standard mode at 32 kHz with a sigma-delta DAC reconstruction filter — high-order, very flat to near-Nyquist, no distinctive character. The SP-303's Lo-Fi effect is a **digital** process: 8-bit quantisation + downsampling, implemented in the COSM DSP chip. This is meaningfully different from the SP-1200 (analog under-filtering + aliasing). Represented at 95% of Nyquist.

---

## Plugin Implementation

### Filter topology
The plugin uses a **4th-order IIR cascade** (two biquad sections in series, Direct Form II transposed) to model the reconstruction filter:

- **Stage 1** — Q = 1.3066 (= 1 / 2·sin(π/8)) — resonant-ish pole pair
- **Stage 2** — Q = 0.5412 (= 1 / 2·sin(3π/8)) — damped pole pair

Together these produce a 4th-order Butterworth (maximally flat) response — accurate to the S950's documented topology. The same topology runs for all presets; the **cutoff frequency** is the only per-preset variable.

### Cutoff tracking
The cutoff parameter is expressed as a fraction of the **target sample rate's Nyquist** (not the host rate). When you load a preset, the cutoff snaps to that machine's value. You can then adjust it freely with the FILTER CUTOFF slider.

### Where the SP-1200 and SP-12 differ
A 4-pole Butterworth at 37% of Nyquist is not an accurate SP-1200 model — the real SP-1200 used a 2-pole filter and the aliasing character comes from the order mismatch + steep foldback. The plugin's filter at SP-1200 cutoff (37%) is a reasonable approximation of the tonal darkness, but won't produce the same alias harmonics as the real hardware. A future "aliasing mode" using a 2-pole model + intentional undersampling would be more accurate.

### Coefficient recomputation
Coefficients are recomputed lazily whenever target SR or cutoff changes by more than a small threshold (0.05% for cutoff), using the Audio EQ Cookbook bilinear-transform LPF formula. This avoids per-sample trig while remaining responsive to automation.

---

## Sources

- Hardware teardown discussions: Gearslutz / Gearspace, KVR Audio forums
- Signalsmith DSP blog (filter topology analysis)
- Goldbaby sampling documentation (SP-1200 characterisation)
- Audio EQ Cookbook — Robert Bristow-Johnson (biquad formulas)
- Akai S950 service manual (analog filter section)
- Knowledge cutoff: August 2025; SP-1200 filter cutoff frequency is debated (9–13 kHz range), 9.5 kHz used here
