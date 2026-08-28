use eframe::egui::{self, Color32, ThemePreference as EguiThemePreference, Visuals};

use super::settings::ThemePreference;

pub fn apply(ctx: &egui::Context, preference: ThemePreference) {
    ctx.set_theme(match preference {
        ThemePreference::System => EguiThemePreference::System,
        ThemePreference::Light => EguiThemePreference::Light,
        ThemePreference::Dark => EguiThemePreference::Dark,
    });
    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(10.0, 12.0);
        style.spacing.button_padding = egui::vec2(12.0, 9.0);
        style.spacing.window_margin = egui::Margin::symmetric(24, 22);
        style.text_styles.insert(
            egui::TextStyle::Heading,
            egui::FontId::proportional(28.0),
        );
        style.text_styles.insert(
            egui::TextStyle::Body,
            egui::FontId::proportional(14.0),
        );
        style.visuals = calm_visuals(style.visuals.dark_mode);
    });
}

fn calm_visuals(dark: bool) -> Visuals {
    let mut visuals = if dark { Visuals::dark() } else { Visuals::light() };
    let (canvas, panel, text, muted, border, accent) = if dark {
        (Color32::from_rgb(18, 24, 21), Color32::from_rgb(26, 34, 29), Color32::from_rgb(232, 239, 234), Color32::from_rgb(164, 180, 170), Color32::from_rgb(53, 70, 61), Color32::from_rgb(80, 157, 112))
    } else {
        (Color32::from_rgb(255, 255, 255), Color32::from_rgb(246, 249, 246), Color32::from_rgb(24, 43, 32), Color32::from_rgb(106, 126, 114), Color32::from_rgb(222, 230, 223), Color32::from_rgb(39, 108, 75))
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
    visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(8);
    visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(8);
    visuals.widgets.active.corner_radius = egui::CornerRadius::same(8);
    visuals.selection.bg_fill = accent;
    visuals.selection.stroke.color = Color32::WHITE;
    visuals.weak_text_color = Some(muted);
    visuals
}
