//! Stable layout measurements for the Calm Workspace shell.

pub fn center_window_in_display(display_size: [f32; 2], window_size: [f32; 2]) -> [f32; 2] {
    [
        ((display_size[0] - window_size[0]) / 2.0).max(0.0),
        ((display_size[1] - window_size[1]) / 2.0).max(0.0),
    ]
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorkspaceMetrics {
    pub toolbar_height: f32,
    pub sidebar_width: f32,
    pub editor_width: f32,
    pub editor_top_padding: f32,
}

impl WorkspaceMetrics {
    pub const TOOLBAR_HEIGHT: f32 = 68.0;
    pub const INITIAL_WINDOW_SIZE: [f32; 2] = [1120.0, 720.0];
    pub const SIDEBAR_WIDTH: f32 = 286.0;
    pub const COMPACT_SIDEBAR_WIDTH: f32 = 224.0;
    pub const EDITOR_MAX_WIDTH: f32 = 684.0;
    pub const EDITOR_TOP_PADDING: f32 = 42.0;
    pub const SETTINGS_CLOSE_HITBOX: f32 = 40.0;
    pub const SIDEBAR_RESIZABLE: bool = false;
    pub const WELCOME_ACTIONS_SIZE: [f32; 2] = [520.0, 86.0];
    const EDITOR_GUTTERS: f32 = 64.0;

    pub fn for_window_width(window_width: f32) -> Self {
        let sidebar_width = if window_width < 900.0 {
            Self::COMPACT_SIDEBAR_WIDTH
        } else {
            Self::SIDEBAR_WIDTH
        };
        let editor_width = (window_width - sidebar_width - Self::EDITOR_GUTTERS)
            .clamp(420.0, Self::EDITOR_MAX_WIDTH);
        Self {
            toolbar_height: Self::TOOLBAR_HEIGHT,
            sidebar_width,
            editor_width,
            editor_top_padding: Self::EDITOR_TOP_PADDING,
        }
    }
}
