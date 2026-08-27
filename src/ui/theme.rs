use eframe::egui::{self, Color32, ThemePreference as EguiThemePreference, Visuals};

use super::settings::ThemePreference;

pub fn apply(ctx: &egui::Context, preference: ThemePreference) {
    ctx.set_theme(match preference {
        ThemePreference::System => EguiThemePreference::System,
        ThemePreference::Light => EguiThemePreference::Light,
        ThemePreference::Dark => EguiThemePreference::Dark,
    });
    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(10.0, 10.0);
        style.spacing.button_padding = egui::vec2(13.0, 8.0);
        style.spacing.window_margin = egui::Margin::symmetric(20, 18);
        style.visuals = calm_visuals(style.visuals.dark_mode);
    });
}

fn calm_visuals(dark: bool) -> Visuals {
    let mut visuals = if dark { Visuals::dark() } else { Visuals::light() };
    let (canvas, panel, text, muted, border, accent) = if dark {
        (Color32::from_rgb(18, 24, 21), Color32::from_rgb(27, 35, 30), Color32::from_rgb(232, 239, 234), Color32::from_rgb(164, 180, 170), Color32::from_rgb(53, 70, 61), Color32::from_rgb(80, 157, 112))
    } else {
        (Color32::from_rgb(248, 250, 248), Color32::from_rgb(239, 244, 240), Color32::from_rgb(29, 42, 34), Color32::from_rgb(104, 123, 112), Color32::from_rgb(214, 225, 217), Color32::from_rgb(39, 108, 75))
    };
    visuals.panel_fill = panel;
    visuals.window_fill = canvas;
    visuals.faint_bg_color = panel;
    visuals.extreme_bg_color = canvas;
    visuals.override_text_color = Some(text);
    visuals.widgets.noninteractive.bg_stroke.color = border;
    visuals.widgets.inactive.bg_stroke.color = border;
    visuals.widgets.hovered.bg_fill = accent.gamma_multiply(if dark { 0.45 } else { 0.18 });
    visuals.widgets.active.bg_fill = accent;
    visuals.selection.bg_fill = accent;
    visuals.selection.stroke.color = Color32::WHITE;
    visuals.weak_text_color = Some(muted);
    visuals
}
