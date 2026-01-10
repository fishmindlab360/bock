//! Custom theme for Bock UI.
//!
//! Provides a modern dark theme inspired by container management tools.

#![allow(dead_code)]

use iced::Color;

/// Bock color palette.
pub mod colors {
    use super::Color;

    /// Primary brand color (blue).
    pub const PRIMARY: Color = Color::from_rgb(0.2, 0.6, 1.0);

    /// Secondary accent color (cyan).
    pub const SECONDARY: Color = Color::from_rgb(0.0, 0.8, 0.8);

    /// Success color (green).
    pub const SUCCESS: Color = Color::from_rgb(0.2, 0.8, 0.4);

    /// Warning color (orange).
    pub const WARNING: Color = Color::from_rgb(1.0, 0.6, 0.2);

    /// Danger color (red).
    pub const DANGER: Color = Color::from_rgb(0.9, 0.3, 0.3);

    /// Background color (dark).
    pub const BACKGROUND: Color = Color::from_rgb(0.1, 0.1, 0.12);

    /// Surface color (slightly lighter).
    pub const SURFACE: Color = Color::from_rgb(0.15, 0.15, 0.18);

    /// Card background.
    pub const CARD: Color = Color::from_rgb(0.18, 0.18, 0.22);

    /// Text primary (white).
    pub const TEXT_PRIMARY: Color = Color::from_rgb(0.95, 0.95, 0.95);

    /// Text secondary (gray).
    pub const TEXT_SECONDARY: Color = Color::from_rgb(0.6, 0.6, 0.65);

    /// Text muted.
    pub const TEXT_MUTED: Color = Color::from_rgb(0.4, 0.4, 0.45);

    /// Border color.
    pub const BORDER: Color = Color::from_rgb(0.25, 0.25, 0.3);
}

/// Container status colors.
pub fn status_color(status: &str) -> Color {
    match status.to_lowercase().as_str() {
        "running" | "healthy" => colors::SUCCESS,
        "starting" | "restarting" => colors::WARNING,
        "stopped" | "exited" => colors::TEXT_MUTED,
        "unhealthy" | "dead" => colors::DANGER,
        _ => colors::TEXT_SECONDARY,
    }
}

/// Size constants.
pub mod sizes {
    /// Sidebar width.
    pub const SIDEBAR_WIDTH: f32 = 220.0;

    /// Card padding.
    pub const CARD_PADDING: u16 = 16;

    /// Default spacing.
    pub const SPACING: u16 = 12;

    /// Small spacing.
    pub const SPACING_SM: u16 = 8;

    /// Large spacing.
    pub const SPACING_LG: u16 = 20;

    /// Border radius.
    pub const BORDER_RADIUS: f32 = 8.0;

    /// Icon size.
    pub const ICON_SIZE: u16 = 20;
}

use iced::Theme;
use iced::widget::{container, text};

/// Secondary text style.
pub fn text_secondary(_theme: &Theme) -> text::Style {
    text::Style {
        color: Some(colors::TEXT_SECONDARY),
    }
}

/// Card container style.
pub fn card(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(colors::CARD.into()),
        text_color: Some(colors::TEXT_PRIMARY),
        border: iced::Border {
            color: colors::BORDER,
            width: 1.0,
            radius: sizes::BORDER_RADIUS.into(),
        },
        ..Default::default()
    }
}
