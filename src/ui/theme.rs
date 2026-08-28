use eframe::egui::{self, Color32, ThemePreference as EguiThemePreference, Visuals};

use super::settings::ThemePreference;

#[derive(Clone, Copy)]
pub(crate) struct Palette {
    pub canvas: Color32,
    pub surface: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub border: Color32,
    pub accent: Color32,
    pub selected: Color32,
}

pub(crate) fn palette_for(dark: bool) -> Palette {
    if dark {
        Palette {
            canvas: Color32::BLACK,
            surface: Color32::from_rgb(18, 18, 18),
            text: Color32::from_rgb(245, 245, 245),
            muted: Color32::from_rgb(157, 157, 157),
            border: Color32::from_rgb(52, 52, 52),
            accent: Color32::WHITE,
            selected: Color32::from_rgb(42, 42, 42),
        }
    } else {
        Palette {
            canvas: Color32::from_rgb(250, 250, 250),
            surface: Color32::WHITE,
            text: Color32::from_rgb(18, 18, 18),
            muted: Color32::from_rgb(101, 101, 101),
            border: Color32::from_rgb(218, 218, 218),
            accent: Color32::BLACK,
            selected: Color32::from_rgb(235, 235, 235),
        }
    }
}

pub fn apply(ctx: &egui::Context, preference: ThemePreference) {
    ctx.set_theme(match preference {
        ThemePreference::System => EguiThemePreference::System,
        ThemePreference::Light => EguiThemePreference::Light,
        ThemePreference::Dark => EguiThemePreference::Dark,
    });
    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(10.0, 10.0);
        style.spacing.button_padding = egui::vec2(14.0, 9.0);
        style.spacing.window_margin = egui::Margin::symmetric(24, 22);
        style
            .text_styles
            .insert(egui::TextStyle::Heading, egui::FontId::proportional(27.0));
        style
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));
        style.visuals = visuals_for(style.visuals.dark_mode);
    });
}

pub(crate) fn visuals_for(dark: bool) -> Visuals {
    let mut visuals = if dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    let palette = palette_for(dark);
    visuals.panel_fill = palette.surface;
    visuals.window_fill = palette.surface;
    visuals.faint_bg_color = palette.surface;
    visuals.extreme_bg_color = palette.canvas;
    visuals.override_text_color = Some(palette.text);
    visuals.widgets.noninteractive.fg_stroke.color = palette.text;
    visuals.widgets.inactive.fg_stroke.color = palette.text;
    visuals.widgets.hovered.fg_stroke.color = palette.text;
    visuals.widgets.open.fg_stroke.color = palette.text;
    visuals.widgets.open.bg_fill = palette.selected;
    visuals.widgets.open.bg_stroke.color = palette.border;
    visuals.widgets.noninteractive.bg_stroke.color = palette.border;
    visuals.widgets.inactive.bg_stroke.color = palette.border;
    visuals.widgets.hovered.bg_fill = palette.selected;
    visuals.widgets.hovered.bg_stroke.color = palette.border;
    visuals.widgets.active.bg_fill = palette.accent;
    visuals.widgets.active.fg_stroke.color = palette.text;
    visuals.selection.bg_fill = palette.selected;
    visuals.selection.stroke.color = palette.text;
    visuals.weak_text_color = Some(palette.muted);
    visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(8);
    visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(8);
    visuals.widgets.active.corner_radius = egui::CornerRadius::same(8);
    visuals
}
