# Filter Research — sssssssssampler

Per-machine anti-aliasing and reconstruction filter characteristics, with verified sources.

---

## Machine Specs (Verified)

| Machine | Sample Rate | Bit Depth | ADC/DAC | Filter | Poles | Cutoff | Sources |
|---|---|---|---|---|---|---|---|
| **E-mu SP-1200** | 26,040 Hz | 12-bit | AD7541 (DAC), successive approx (ADC) | TL084 input AA + 5-pole Chebyshev output (4 ch), SSM2044 VCF (2 ch), unfiltered (2 ch) | 2-pole (modeled) | ~37% Nyquist | [1][2][3] |
| **Akai MPC60** | 40,000 Hz | 12-bit audio (16-bit converters) | Burr Brown PCM54HP (DAC), PCM77P (ADC) | No user filter (spec sheet: "no filter, no LFO") | 4-pole (modeled) | ~90% Nyquist | [11] |
| **Akai S950** | 7.5–48 kHz (variable) | 12-bit | — | MF6CN-50 switched-cap Butterworth | 6-pole (36 dB/oct) | ~80% Nyquist (at max BW) | [5][8] |
| **Ensoniq Mirage** | 10–33 kHz (variable) | 8-bit | ES5503 DOC | Curtis CEM3328 4-pole resonant (24 dB/oct) | 4-pole resonant | Envelope-controlled | [12] |
| **SCI Prophet 2000** | 15.6–41.7 kHz | 12-bit | — | Curtis CEM3379 4-pole resonant VCF (24 dB/oct) | 4-pole resonant | User-controlled + resonance | [13] |
| **Akai MPC3000** | 44,100 Hz | 16-bit in, 18-bit DAC | Burr Brown PCM69A (18-bit DAC) | 8× oversampling digital + 2-pole analog anti-alias at ~26 kHz | 2-pole output | ~99% Nyquist | [9][10] |
| **Boss SP-303** | 44,100 Hz | 16-bit (20-bit AD/DA) | Sigma-delta | High-order reconstruction (COSM DSP chip) | ~4-pole equiv | ~95% Nyquist | [7] |
|||||||||
| *E-mu SP-12* | *27,500 Hz* | *12-bit* | *Same DAC lineage as SP-1200* | *Reconstruction filter deliberately omitted* | *—* | *N/A* | *[2][4] — merged into SP-1200 preset* |
| *Akai S612* | *4–32 kHz* | *12-bit* | *—* | *MF6CN-50 (same as S950)* | *6-pole* | *Clock-tunable* | *[5][6] — removed (overlaps S950)* |

---

## Per-Machine Detail

### E-mu SP-1200

**Sample rate:** 26,040 Hz (fixed). Chosen during Drumulator development as a bandwidth/memory tradeoff. **12-bit linear** using the AD7541 DAC, also used as ADC via successive approximation [1].

**Filter architecture (complex, per-channel):**
- **Input anti-aliasing:** TL084 op-amp-based active lowpass filter arrangement [1]
- **Output channels 0-1 (toms):** SSM2044 dynamic VCF with Z80-generated AR envelope (5ms attack + decay). The SSM2044 is Dave Rossum's "improved Moog ladder on a chip" [2][3]
- **Output channels 2-5 (snare, bass, claps, cowbell):** Static 5-pole 1dB Chebyshev filters at fixed frequencies tuned per sound [1]
- **Output channels 6-7 (ride, hi-hat):** Unfiltered [1]

**Plugin model:** 2-pole Butterworth with low cutoff (~37% Nyquist). This is a simplification — the real SP-1200 has per-channel filtering with different topologies. The 2-pole model captures the essential under-filtering that allows aliasing foldback, which is the dominant character.

**Key insight:** The SP-1200's sound comes from the combination of under-filtering (aliases fold back), 12-bit quantization, drop-sample pitch shifting, and the SSM2044 VCF on two channels. It's not a single simple filter [2].

### E-mu SP-12

**Sample rate:** 27,500 Hz (fixed) [4]. Sometimes reported as 26,040 Hz (confusion with SP-1200), but the SP-12 runs at the higher rate.

**Filter: Reconstruction filter deliberately omitted** [4]. This is confirmed by CCRMA research — spectral images above the 13.75 kHz Nyquist are not attenuated, resulting in a characteristically bright sound. The SP-12/SP-1200 use three pairs of analog lowpass filters at different frequencies on different channel pairs, but no proper reconstruction filter exists in the signal path [4].

**Plugin model:** 2-pole Butterworth at ~45% Nyquist. This approximates the partial filtering from the channel-specific output filters while allowing substantial imaging to pass through. The real machine would be better modeled with no filter at all on some channels.

### Akai S612

**Sample rate:** 4–32 kHz (variable, selectable). 12-bit, 6-voice polyphony [5][6].

**Filter: MF6CN-50 — 6th-order Butterworth switched-capacitor lowpass.** This is the same National Semiconductor filter IC used in the S950 [5][6]. The service manual confirms the MF6CN-50 as the lowpass filter component. The filter is non-resonant with a clock-tunable cutoff frequency. The S612 also includes an MF10CN universal switched-cap filter IC.

**CORRECTION:** Previously documented as 4-pole. The S612 service manual confirms 6-pole (MF6CN-50 = 6th order). Updated from `poles: 4.0` to `poles: 6.0` in presets.

### Boss SP-303

**Sample rate:** 44,100 Hz standard, 22,050 Hz (Long mode), 11,025 Hz (Lo-Fi mode) [7]. **16-bit** internal resolution; some sources suggest 20-bit AD/DA converters.

**Filter:** High-order sigma-delta DAC reconstruction. The SP-303's character comes primarily from its **digital effects** (Vinyl Sim, Lo-Fi, compressor) implemented in the COSM DSP chip, not from analog filter coloration [7]. The Vinyl Sim adds compression, noise, and wow/flutter — the Goodhertz Vulf Compressor is based on the SP-303's Vinyl Sim compression algorithm.

**Plugin model:** 4-pole Butterworth at 95% Nyquist (essentially transparent filter).

### Akai S950

**Sample rate:** 7.5–48 kHz (variable). Controlled via "bandwidth" setting: bandwidth × 2.5 = sample rate. Maximum bandwidth 19.2 kHz → 48 kHz sample rate [8].

**Filter: MF6CN-50 — 6th-order Butterworth switched-capacitor, 36 dB/oct.** National Semiconductor (now TI) datasheet confirms "6th Order Switched Capacitor Butterworth Lowpass Filter" [5][8]. Nine MF6CN-50 chips in both S900 and S950 service manuals. Non-resonant, no realtime modulation, but has its own envelope and velocity control [8].

**Minimum cutoff set relatively high** to avoid clock bleed into the filter output from the digital control signal — this is a characteristic limitation of switched-cap filter technology [8].

**Plugin model:** 6-pole Butterworth at ~80% Nyquist. The steep 36 dB/oct slope effectively suppresses aliasing, giving the S950 its clean-but-warm character. Matched.

### Akai MPC60

**Sample rate:** 40,000 Hz (fixed). **12-bit audio** but uses 16-bit converters: Burr Brown PCM54HP DAC, PCM77P ADC [11]. Intel 80186 CPU, 16-voice polyphony.

**Filter:** The MPC60 spec sheet lists **"no filter and no LFO"** — there is no user-accessible filter. However, the output stage includes reconstruction filtering inherent to the DAC. Roger Linn stated the MPC60's sound was due to 12-bit sampling and "the way he set up the filters" [11].

**Character:** The MPC60 is distinctly different from the SP-1200 despite both being 12-bit. The higher sample rate (40 kHz vs 26 kHz), 16-bit converters (vs 12-bit AD7541), and lack of SSM2044 VCF mean the MPC60 is cleaner with more headroom, while retaining 12-bit quantization grit. The famous "MPC swing" is a sequencer timing feature, not an audio characteristic, but the MPC60's transient response is notably punchy due to the Burr Brown converters [11].

**Plugin model:** 4-pole Butterworth at ~90% Nyquist (gentle, transparent filtering). Low jitter (0.5%). The differentiation from S950 and SP-1200 is in the 40 kHz rate + 12-bit depth combination.

### Ensoniq Mirage

**Sample rate:** 10–33 kHz (variable, user-selectable via parameter 34). Up to 50 kHz with optional Input Sampling Filter cartridge [12]. **8-bit** resolution — one of the lowest-resolution professional samplers, sharing 8-bit depth with the Emulator II and Fairlight CMI Series II.

**Filter: Curtis CEM3328 — 4-pole resonant lowpass (24 dB/oct).** 8× CEM3328 chips (one per voice). The filter has resonance, keytracking, and its own 5-segment APDSR envelope [12]. This is the key differentiator — the Mirage is the only preset with a **resonant** analog filter in the signal path.

**Digital oscillator:** Ensoniq ES5503 DOC (Digital Oscillator Chip), designed by Robert Yannes (creator of the MOS SID chip from Commodore 64) [12].

**Character:** The 8-bit resolution creates heavy quantization noise and gritty aliasing, especially when transposing from original sample pitch. The CEM3328 resonant filter smooths this into a warm, thick sound with a "filled-out bass end." The low sample rates (often 10-20 kHz to maximize sample time from 128 KB RAM) create strong aliasing [12].

**Plugin model:** 4-pole at 33 kHz, 8-bit, high jitter (3%). The 8-bit depth is the dominant character — produces 14× more quantization noise than 12-bit. The jitter models the inherent instability of the ES5503 DOC's sample playback timing.

### Sequential Circuits Prophet 2000

**Sample rate:** 15,625 / 31,250 / 41,667 Hz (three selectable rates). **12-bit**, 8-voice polyphony, 256 kiloword sample memory [13].

**Filter: Curtis CEM3379 — 4-pole resonant VCF (24 dB/oct).** 8× CEM3379 chips (one per voice). Adjustable cutoff, resonance, and modulation depth. Also includes VCA on the same chip [13].

**Character:** The Prophet 2000's analog VCF with resonance makes it unique among vintage samplers — most samplers have non-resonant filters. The CEM3379 allows musical filter sweeps that blend the sampled sound with classic analog synthesis character. Despite 12-bit limitations, the analog VCF/VCA path gives a warm, musical quality [13].

**Plugin model:** 4-pole at 41.667 kHz, 12-bit, standard jitter. The key differentiator is the resonant filter — when we add resonance control to the filter model, this preset would activate it. Currently approximated with standard 4-pole Butterworth (no resonance yet).

### Akai MPC3000

**Sample rate:** 44,100 Hz (fixed). **16-bit input, 18-bit DAC output** using the Burr Brown PCM69A dual 18-bit DAC [9][10].

**Signal chain:**
1. 8× oversampling digital filter (reads at 352.8 kHz) [9]
2. Burr Brown PCM69A 18-bit DAC [9]
3. 2-pole lowpass anti-aliasing filter at -3dB point of ~26 kHz [10]
4. NJM5532D low-noise op-amp output stage [10]

**NOT an AK4316** — the Burr Brown PCM69A is confirmed by the MPC3000 service manual [9]. The AKM (Asahi Kasei) chips may appear in other Akai models.

**Plugin model:** 4-pole Butterworth at 99% Nyquist. The real machine uses 8× oversampling + 2-pole output + 18-bit DAC for maximum transparency. The "MPC3000 sound" is the combination of the digital filter, DAC conversion artifacts, and the NJM5532D op-amp coloration [10].

---

## Plugin Implementation

### Filter topology
Direct Form II transposed biquad sections in cascade:

**2-pole** (SP-1200, SP-12 — models under-filtering):
- Single biquad, Q = 0.7071 (Butterworth 2nd order, 12 dB/oct)
- Real hardware: SP-1200 has mixed topology per channel; SP-12 omits reconstruction filter entirely
- This is an approximation that captures the dominant under-filtering character

**4-pole** (SP-303, MPC3000 — models transparent/clean filtering):
- Stage 1 — Q = 1.3066
- Stage 2 — Q = 0.5412
- 4th-order Butterworth (24 dB/oct)

**6-pole** (S612, S950 — matches MF6CN-50 hardware):
- Stage 1 — Q = 1.9319
- Stage 2 — Q = 1.0000
- Stage 3 — Q = 0.5176
- 6th-order Butterworth (36 dB/oct) — direct match to MF6CN-50 datasheet specification

### Per-stage nonlinearity (V5)
Stages 2+ in the cascade apply subtle op-amp saturation: `y - y³ × 0.02`. This models the slight nonlinearity of cascaded analog stages in the MF6CN-50 implementation.

### Pre-filter (V5)
For machines with 4+ pole filters (S950, MPC60, Mirage, P-2000, MPC3000, SP-303), a pre-filter stage runs before sample-and-hold to model the analog input anti-aliasing filter. For under-filtered machines (SP-1200), the pre-filter is bypassed to allow aliasing.

### DAC reconstruction (V5)
Post-filter DAC smoothing models the sample-and-hold output stage: `0.7 × current + 0.3 × previous`, with transient emphasis `+ 0.1 × (current - previous)`.

### ADC nonlinearity (V5)
Subtle harmonic distortion before quantization: `s + s³ × 0.02 + |s| × s × 0.001`. Models real ADC input stage nonlinearity.

### Noise-shaped quantization (V5)
First-order error feedback: `shaped = sample + error × 0.5`, then quantize, track error. Reduces audible quantization noise by shifting it to higher frequencies.

---

## Sources

1. SP-1200 hardware teardown — per-channel filter topology, AD7541 DAC, TL084 input filters, SSM2044 VCF on channels 0-1, 5-pole Chebyshev on channels 2-5, channels 6-7 unfiltered
2. CCRMA Stanford — David T. Yeh, "Physical and Behavioral Circuit Modeling of the SP-12 Sampler" (2007). Confirms filter architecture, imaging from omitted reconstruction filter, SSM2044 usage
3. SSM2044 — Dave Rossum's "improved Moog ladder" IC. Used on SP-12/SP-1200 channels 0-1 only (dynamic VCF with AR envelope). SSI2144 reissue available
4. SP-12 specs — 27,500 Hz sample rate (distinct from SP-1200's 26,040 Hz). Reconstruction filter deliberately omitted per E-mu design decision during Drumulator development
5. National Semiconductor MF6CN-50 datasheet — "6th Order Switched Capacitor Butterworth Lowpass Filter". Confirmed in both Akai S612 and S950 service manuals
6. Akai S612 service manual — MF6CN-50 (lowpass filter) and MF10CN (universal filter). Available on Archive.org
7. Boss SP-303 specs — 44.1/22.05/11.025 kHz modes, 16-bit, COSM DSP for Vinyl Sim/Lo-Fi effects
8. Akai S950 technical analysis — 9 × MF6CN-50 chips, 36 dB/oct, non-resonant, clock-tunable cutoff, minimum cutoff limited by clock bleed
9. MPC3000 service manual — Burr Brown PCM69A 18-bit DAC (not AK4316), 8× oversampling digital filter at 352.8 kHz
10. MPC3000 output stage analysis — 2-pole anti-alias filter at -3dB ~26 kHz, NJM5532D op-amp. "The MPC3000 sound is a combination of the digital filter, the 18-bit Burr Brown DAC and the anti-aliasing filter with the opamps"
11. MPC60 specifications — 40 kHz, 12-bit audio, Burr Brown PCM54HP DAC + PCM77P ADC (both 16-bit converters). Intel 80186 CPU. Roger Linn / David Cockerell design. "No filter, no LFO" per spec sheet. 750 KB sample memory, 16-voice polyphony
12. Ensoniq Mirage specifications — 8-bit, 10-33 kHz variable (50 kHz with optional ISF cartridge). 8× Curtis CEM3328 4-pole resonant filters (24 dB/oct). Ensoniq ES5503 DOC (Robert Yannes, SID chip designer). 128 KB RAM, 8-voice polyphony. ~30,000 units sold
13. Sequential Circuits Prophet 2000 specifications — 12-bit, 15.6/31.25/41.667 kHz selectable. 8× Curtis CEM3379 4-pole resonant VCF (24 dB/oct) with resonance + VCA. 256 kiloword sample memory, 8-voice polyphony. Dave Smith design, 1985
14. Audio EQ Cookbook — Robert Bristow-Johnson (biquad coefficient formulas for Butterworth cascades)
