use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::*;
use nih_plug_vizia::{create_vizia_editor, ViziaState, ViziaTheming};
use std::sync::Arc;

use nih_plug_vizia::vizia::binding::Data;

use crate::SssssssssamplerParams;

pub(crate) const WINDOW_WIDTH: u32 = 540;
pub(crate) const WINDOW_HEIGHT: u32 = 230;

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
            Self::NoniDark => "theme-noni-dark",
            Self::Paris => "theme-paris",
            Self::Rooney => "theme-rooney",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::NoniLight => "noni ☀",
            Self::NoniDark => "noni ◉",
            Self::Paris => "paris",
            Self::Rooney => "rooney",
        }
    }
}

impl Data for Theme {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
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
}

pub enum EditorEvent {
    SetTheme(Theme),
}

impl Model for EditorData {
    fn event(&mut self, _cx: &mut EventContext, event: &mut Event) {
        event.map(|e: &EditorEvent, _| {
            let EditorEvent::SetTheme(t) = e;
            self.theme = *t;
        });
    }
}

// ─── Editor factory ───────────────────────────────────────────────────────────

pub(crate) fn default_state() -> Arc<ViziaState> {
    ViziaState::new(|| (WINDOW_WIDTH, WINDOW_HEIGHT))
}

pub(crate) fn create(
    params: Arc<SssssssssamplerParams>,
    editor_state: Arc<ViziaState>,
) -> Option<Box<dyn Editor>> {
    create_vizia_editor(
        editor_state,
        ViziaTheming::Custom,
        move |cx, _| {
            EditorData {
                params: params.clone(),
                theme: Theme::NoniDark,
            }
            .build(cx);

            cx.add_stylesheet(include_str!("../assets/style.css"))
                .expect("Failed to load stylesheet");

            // Outer Binding on theme so the root element gets the right CSS class.
            // The whole UI re-renders on theme change — fine for a plugin that
            // rarely switches themes.
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

                    // ── Controls ──────────────────────────────────────────────
                    HStack::new(cx, |cx| {
                        param_column(cx, "SAMPLE RATE", EditorData::params, |p| &p.target_sr);
                        param_column(cx, "BIT DEPTH", EditorData::params, |p| &p.bit_depth);
                        param_column(cx, "JITTER", EditorData::params, |p| &p.jitter);
                        param_column(cx, "MIX", EditorData::params, |p| &p.mix);
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
