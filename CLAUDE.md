# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

A VST3/CLAP audio plugin in Rust that emulates 7 vintage hardware samplers (SP-1200, MPC60, S950, Mirage, P-2000, MPC3000, SP-303) as a sample rate and bit depth reducer, combined with a real-time animated ASCII art visualization engine driven by audio analysis.

## Commands

```bash
# Build release bundle (VST3 + CLAP)
cargo xtask bundle sssssssssampler --release

# Build debug bundle
cargo xtask bundle sssssssssampler

# Install to DAW (macOS, release)
bash install.sh

# Install dev build + clear plugin caches (macOS)
bash install-dev.sh

# Run tests
cargo test

# Run benchmarks
cargo bench --bench render_bench

# Cross-compile (macOS targets)
cargo xtask bundle sssssssssampler --target aarch64-apple-darwin
cargo xtask bundle sssssssssampler --target x86_64-apple-darwin
```

## Architecture

### Source files (`src/`)

| File | Role |
|------|------|
| `lib.rs` | Plugin struct, all DSP, parameter definitions |
| `editor.rs` | V5 animation engine — 46×36 ASCII grid, 9 integrated features |
| `ascii_image_display.rs` | femtovg rendering, UI overlay, mouse interaction |
| `ascii_bank.rs` | Parses `ascii.txt` (38 images, `#N` separator format) |
| `audio_feed.rs` | `AudioFeed` + `AnimationParams` — audio analysis bridge to UI |
| `parameter_remapping.rs` | Perceptual nonlinear parameter mapping helpers |
| `render/color_system.rs` | 14 color themes × light/dark modes |
| `render/audio_analysis.rs` | RMS, transient detection |
| `render/offscreen.rs` | `FrameBuffer` struct — 46×36 grid + metadata |

### DSP pipeline (`lib.rs`)

Per-sample signal path:
1. **Pre-filter** (4+pole machines only) — analog input AA stage before S&H
2. **Sample-and-hold** — clock drift LFO + jitter noise
3. **ADC nonlinearity** — `s + s³·0.02 + |s|·s·0.001` (subtle harmonic distortion)
4. **Bit crush** — `crush_shaped()` with first-order noise shaping
5. **Reconstruction filter** — 2/4/6-pole Butterworth cascade (Cookbook biquads, Direct Form II transposed), or skipped when cutoff=100% and anti-alias=off
6. **DAC reconstruction** — `0.7·wet + 0.3·prev_dac` + transient emphasis
7. **Dry/wet mix** — `dry + (wet - dry) * mix`

### Animation engine (`editor.rs`)

Runs in the UI thread. `update_frame_buffer()` produces a `FrameBuffer` (46×36 chars + color/metadata) every frame from `AnimationParams` (RMS energy, transient flag, BPM, playing state). The V5 system has 9 layered features: temporal echo, DropPhase, hero lock, global field warping, intent rendering modes, per-preset signature behaviors, per-cell flicker, global activity reduction, and visual enhancements. All noise is hash-based (deterministic). No per-frame heap allocation in hot paths.

### Image system

`ascii.txt` contains 38 ASCII art images separated by `#N` markers. `AsciiBank` parses these at startup. Images cycle every 2 bars (BPM-synced); overlays cycle at 1.5–3.0 bar intervals. Images larger than the 46×36 viewport pan across their full extent.

### Hardware preset modeling

Each of the 7 machine presets has a `VisualProfile` (14 fields controlling motion, glitch, bloom, etc.) and distinct filter topology: 2-pole (SP-1200/SP-12), 4-pole (S612/MPC3000/SP-303), 6-pole (S950). Filter Q values are exact Butterworth section values. See `docs/filter-research.md` for hardware evidence.

## Key constraints

- No allocations in the DSP hot loop (`process()`) or animation hot path
- All hash/noise must be deterministic (LCG RNG seeded per-block, not time-based)
- Filter biquads use Direct Form II Transposed with NaN guards
- The UI overlay (title + menu) is never written into the `FrameBuffer` — it's drawn on top in `ascii_image_display.rs`
- Window is 422×600 px; grid is 46×36 characters using FiraCode Nerd Font (embedded TTF)

## Documentation

- `docs/ASCII_ANIMATION_SYSTEM.md` — full V4 architecture reference (still accurate for V5 base)
- `docs/superpowers/specs/2026-03-23-ascii-animation-v5-design.md` — V5 design spec
- `docs/filter-research.md` — hardware filter specs with sources
