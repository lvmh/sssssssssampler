use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::*;
use nih_plug_vizia::{create_vizia_editor, ViziaState, ViziaTheming};
use std::sync::Arc;

use nih_plug_vizia::vizia::binding::Data;

use crate::SssssssssamplerParams;
use crate::AnimationParams;
use crate::editor_view::AsciiRenderView;
use std::sync::Mutex;

pub(crate) const WINDOW_WIDTH: u32 = 540;
pub(crate) const WINDOW_HEIGHT: u32 = 270;

// ─── Machine presets ──────────────────────────────────────────────────────────
//
// poles: 2.0 = 2-pole LP (SP-1200 / SP-12 lineage — under-filtered, gritty)
//        4.0 = 4-pole Butterworth (S950 / S612 / SP-303 / MPC3000 — clean)
// cutoff is always set to 1.0 (fully open) on preset load.

struct MachinePreset {
    name: &'static str,
    sr: f32,
    bits: f32,
    jitter: f32,
    poles: f32,
}

const PRESETS: &[MachinePreset] = &[
    MachinePreset { name: "SP-1200", sr: 26_040.0, bits: 12.0, jitter: 0.01, poles: 2.0 },
    MachinePreset { name: "SP-12",   sr: 27_500.0, bits: 12.0, jitter: 0.01, poles: 2.0 },
    MachinePreset { name: "S612",    sr: 31_250.0, bits: 12.0, jitter: 0.01, poles: 4.0 },
    MachinePreset { name: "SP-303",  sr: 32_000.0, bits: 12.0, jitter: 0.01, poles: 4.0 },
    MachinePreset { name: "S950",    sr: 39_375.0, bits: 12.0, jitter: 0.01, poles: 4.0 },
    MachinePreset { name: "MPC3000", sr: 44_100.0, bits: 16.0, jitter: 0.0,  poles: 4.0 },
];

// S950 — matches the default target_sr in lib.rs (39_375 Hz)
const DEFAULT_PRESET: usize = 4;

// ─── Theme ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(clippy::enum_variant_names)]
pub enum Theme {
    NoniLight,
    NoniDark,
    Paris,
    Rooney,
}

impl Theme {
    fn css_class(self) -> &'static str {
        match self {
            Self::NoniLight => "theme-noni-light",
            Self::NoniDark  => "theme-noni-dark",
            Self::Paris     => "theme-paris",
            Self::Rooney    => "theme-rooney",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::NoniLight => "noni ☀",
            Self::NoniDark  => "noni ◉",
            Self::Paris     => "paris",
            Self::Rooney    => "rooney",
        }
    }
}

impl Data for Theme {
    fn same(&self, other: &Self) -> bool { self == other }
}

const THEMES: [Theme; 4] = [
    Theme::NoniLight,
    Theme::NoniDark,
    Theme::Paris,
    Theme::Rooney,
];

// ─── Model ────────────────────────────────────────────────────────────────────

#[derive(Lens)]
pub struct EditorData {
    pub params: Arc<SssssssssamplerParams>,
    pub theme: Theme,
    pub preset_idx: usize,
    #[lens(ignore)]
    pub gui_ctx: Arc<dyn GuiContext>,
    #[lens(ignore)]
    pub anim_params: Arc<Mutex<AnimationParams>>,
}

pub enum EditorEvent {
    SetTheme(Theme),
    PrevPreset,
    NextPreset,
}

impl Model for EditorData {
    fn event(&mut self, _cx: &mut EventContext, event: &mut Event) {
        event.map(|e: &EditorEvent, _| match e {
            EditorEvent::SetTheme(t) => self.theme = *t,
            EditorEvent::PrevPreset => {
                if self.preset_idx > 0 {
                    self.preset_idx -= 1;
                    self.apply_preset();
                }
            }
            EditorEvent::NextPreset => {
                if self.preset_idx + 1 < PRESETS.len() {
                    self.preset_idx += 1;
                    self.apply_preset();
                }
            }
        });
    }
}

impl EditorData {
    fn apply_preset(&self) {
        let p = &PRESETS[self.preset_idx];
        let setter = ParamSetter::new(&*self.gui_ctx);

        setter.begin_set_parameter(&self.params.target_sr);
        setter.set_parameter(&self.params.target_sr, p.sr);
        setter.end_set_parameter(&self.params.target_sr);

        setter.begin_set_parameter(&self.params.bit_depth);
        setter.set_parameter(&self.params.bit_depth, p.bits);
        setter.end_set_parameter(&self.params.bit_depth);

        setter.begin_set_parameter(&self.params.jitter);
        setter.set_parameter(&self.params.jitter, p.jitter);
        setter.end_set_parameter(&self.params.jitter);

        setter.begin_set_parameter(&self.params.filter_poles);
        setter.set_parameter(&self.params.filter_poles, p.poles);
        setter.end_set_parameter(&self.params.filter_poles);

        // Cutoff always 100% on preset change — user can sweep from there
        setter.begin_set_parameter(&self.params.filter_cutoff);
        setter.set_parameter(&self.params.filter_cutoff, 1.0);
        setter.end_set_parameter(&self.params.filter_cutoff);
    }
}

// ─── Editor factory ───────────────────────────────────────────────────────────

pub(crate) fn default_state() -> Arc<ViziaState> {
    ViziaState::new(|| (WINDOW_WIDTH, WINDOW_HEIGHT))
}

pub(crate) fn create(
    params: Arc<SssssssssamplerParams>,
    editor_state: Arc<ViziaState>,
    anim_params: Arc<Mutex<AnimationParams>>,
) -> Option<Box<dyn Editor>> {
    create_vizia_editor(
        editor_state,
        ViziaTheming::Custom,
        move |cx, gui_ctx| {
            EditorData {
                params: params.clone(),
                theme: Theme::NoniDark,
                preset_idx: DEFAULT_PRESET,
                gui_ctx: gui_ctx.clone(),
                anim_params: anim_params.clone(),
            }
            .build(cx);

            cx.add_stylesheet(include_str!("../assets/style.css"))
                .expect("Failed to load stylesheet");

            Binding::new(cx, EditorData::theme, |cx, theme_lens| {
                let theme = theme_lens.get(cx);

                VStack::new(cx, |cx| {
                    // ── Header ────────────────────────────────────────────────
                    HStack::new(cx, |cx| {
                        Label::new(cx, "sssssssssampler").class("plugin-title");

                        HStack::new(cx, |cx| {
                            for t in THEMES {
                                let is_active = theme == t;
                                Label::new(cx, t.label())
                                    .class("theme-pill")
                                    .toggle_class("active", is_active)
                                    .on_press(move |ex| {
                                        ex.emit(EditorEvent::SetTheme(t));
                                    });
                            }
                        })
                        .class("theme-switcher");
                    })
                    .class("header");

                    // ── Rendering view ────────────────────────────────────────
                    {
                        let editor_data = cx.data::<EditorData>().unwrap();
                        AsciiRenderView::new(cx, editor_data.anim_params.clone());
                    }

                    // ── Preset navigator ──────────────────────────────────────
                    HStack::new(cx, |cx| {
                        Label::new(cx, "◄")
                            .class("preset-arrow")
                            .on_press(|ex| ex.emit(EditorEvent::PrevPreset));

                        Binding::new(cx, EditorData::preset_idx, |cx, idx_lens| {
                            let idx = idx_lens.get(cx);
                            Label::new(cx, PRESETS[idx].name).class("preset-name");
                        });

                        Label::new(cx, "►")
                            .class("preset-arrow")
                            .on_press(|ex| ex.emit(EditorEvent::NextPreset));
                    })
                    .class("preset-row");

                    // ── Controls ──────────────────────────────────────────────
                    HStack::new(cx, |cx| {
                        param_column(cx, "SAMPLE RATE", EditorData::params, |p| &p.target_sr);
                        param_column(cx, "BIT DEPTH",   EditorData::params, |p| &p.bit_depth);
                        param_column(cx, "JITTER",      EditorData::params, |p| &p.jitter);
                        param_column(cx, "FILTER",      EditorData::params, |p| &p.filter_cutoff);
                        param_column(cx, "MIX",         EditorData::params, |p| &p.mix);
                    })
                    .class("controls");
                })
                .class("plugin-root")
                .class(theme.css_class());
            });
        },
    )
}

/// A labelled parameter column: small ALL-CAPS label above, slider below.
fn param_column<L, Params, P, FMap>(
    cx: &mut Context,
    label: &str,
    lens: L,
    map: FMap,
) where
    L: Lens<Target = Arc<Params>> + Clone,
    Params: nih_plug::params::Params,
    P: nih_plug::params::Param + 'static,
    FMap: Fn(&Arc<Params>) -> &P + Copy + 'static,
{
    VStack::new(cx, |cx| {
        Label::new(cx, label).class("param-label");
        ParamSlider::new(cx, lens, map).class("param-slider");
    })
    .class("param-col");
}
