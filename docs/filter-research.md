# Filter Research — sssssssssampler

Per-machine anti-aliasing and reconstruction filter characteristics, and how they're implemented in the plugin.

---

## Machine Specs

| Machine | Sample Rate | Bit Depth | Filter Type | Cutoff (% of Nyquist) |
|---|---|---|---|---|
| E-mu SP-1200 | 26,040 Hz | 12-bit | 2-pole simple LP | ~37% |
| Akai S950 | 48,000 Hz | 12-bit | 6-pole Butterworth (MF6CN-50, 36 dB/oct) | ~80% |
| Akai S612 | 32,000 Hz | 12-bit | 4-pole Butterworth-ish | ~75% |
| E-mu SP-12 | 27,500 Hz | 12-bit | 2-pole (SP-1200 lineage) | ~45% |
| Akai MPC3000 | 44,100 Hz | 16-bit | High-order FIR | ~99% |
| Boss SP-303 | 44,100 Hz | 16-bit (20-bit AD/DA) | Sigma-delta DAC | ~95% |

---

## Per-Machine Detail

### E-mu SP-1200
The SP-1200's most distinctive feature is its **under-filtering**. The analog lowpass cuts at roughly 37% of its own Nyquist (~9.5 kHz), well below what's needed to suppress aliasing. Alias components fold back into the audible range and become part of the sound — the "grit" and pitch-dependent crunch SP-1200 users describe. The filter itself is a simple 2-pole (RC or op-amp) design with no resonance and ~12 dB/octave rolloff. This is not a flaw; it's the character.

### Akai S950
A 6-pole (6th-order) Butterworth lowpass, implemented with a National Semiconductor MF6CN-50 switched-capacitor filter IC. This gives 36 dB/octave rolloff — steeper than initially documented. The S950's maximum sample rate is 48 kHz (variable 7.5–48 kHz), controlled indirectly via a "bandwidth" setting (bandwidth × 2.5 = sample rate). At max bandwidth (19.2 kHz), the filter cuts well below Nyquist (~80%), giving the S950 its characteristic high-frequency softness. The same filter can be set even lower manually as an effect. No resonance. The S950 sounds clean relative to the SP-1200 because the steep 36 dB/oct slope effectively suppresses aliasing.

### Akai S612
Akai's first 12-bit sampler. Supports selectable rates from 4 to 32 kHz. The filter is believed to be a 4-pole design similar to the S950 lineage, but less well-documented. At lower rates the cutoff drops into the mid-range and becomes tonally significant. Approximated at 75% of Nyquist.

### E-mu SP-12
Predecessor to the SP-1200, sharing E-mu's DSP chip lineage. Filter is similarly simple (2-pole) but the cutoff sits slightly higher at ~45% of Nyquist, making it marginally less gritty than the SP-1200. Still allows significant aliasing foldback.

### Akai MPC3000
Designed by Roger Linn for fidelity, not color. Runs at 44.1 kHz with a high-order oversampling filter (likely 8th order or linear-phase FIR in the custom VLSI chip). Effectively transparent — no audible filter character. Represented in the plugin at 99% of Nyquist with the filter on, though in practice the MPC3000 sound comes from the sample rate and bit depth, not any analog coloration.

### Boss SP-303
Standard mode at 44.1 kHz (with 32 kHz as secondary "Long" mode) using 16-bit internal processing and 20-bit AD/DA converters. The sigma-delta DAC reconstruction filter is high-order, very flat to near-Nyquist, no distinctive character. The SP-303's Lo-Fi effect is a **digital** process: bit-crushing + downsampling, implemented in the COSM DSP chip — the "SP grit" comes from effects processing (Vinyl Sim, compressor), not from low-resolution sampling. This is meaningfully different from the SP-1200 (analog under-filtering + aliasing). Represented at 95% of Nyquist.

---

## Plugin Implementation

### Filter topology
The plugin supports three filter topologies via the `poles` parameter, all using Direct Form II transposed biquad sections in cascade:

**2-pole** (SP-1200, SP-12):
- Single biquad, Q = 0.7071 (= 1/√2, Butterworth 2nd order, 12 dB/oct)

**4-pole** (S612, SP-303, MPC3000):
- Stage 1 — Q = 1.3066 (= 1 / 2·sin(π/8)) — resonant-ish pole pair
- Stage 2 — Q = 0.5412 (= 1 / 2·sin(3π/8)) — damped pole pair
- Together: 4th-order Butterworth (24 dB/oct)

**6-pole** (S950):
- Stage 1 — Q = 1.9319 (= 1 / 2·sin(π/12)) — high-resonance pole pair
- Stage 2 — Q = 1.0000 (= 1 / 2·sin(3π/12)) — unity-Q pole pair
- Stage 3 — Q = 0.5176 (= 1 / 2·sin(5π/12)) — damped pole pair
- Together: 6th-order Butterworth (36 dB/oct) — matches the S950's MF6CN-50 IC

The **cutoff frequency** and **pole count** are the per-preset variables.

### Cutoff tracking
The cutoff parameter is expressed as a fraction of the **target sample rate's Nyquist** (not the host rate). When you load a preset, the cutoff snaps to that machine's value. You can then adjust it freely with the FILTER CUTOFF slider.

### Where the SP-1200 and SP-12 differ
The SP-1200 and SP-12 presets now correctly use a 2-pole Butterworth (12 dB/oct) at their respective cutoffs. This captures the under-filtering that allows aliasing to fold back into the audible range. However, the real alias harmonics also depend on the specific DAC and sample-and-hold behaviour — a future "aliasing mode" using intentional undersampling would be more accurate for the full SP-1200 character.

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
