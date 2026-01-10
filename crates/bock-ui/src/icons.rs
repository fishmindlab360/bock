//! Icon definitions using Font Awesome.
//!
//! This module provides icon constants from Font Awesome 7 Free.

#![allow(dead_code)]

use iced::Font;
use iced::widget::Text;

/// Font Awesome Solid font (Weight 900).
pub const FA_SOLID_FONT: Font = Font {
    family: iced::font::Family::Name("fa_solid"),
    weight: iced::font::Weight::Black,
    stretch: iced::font::Stretch::Normal,
    style: iced::font::Style::Normal,
};

/// Font bytes for Font Awesome Solid.
pub const FA_SOLID: &[u8] = include_bytes!("../assets/fa_regular.otf");

/// Font bytes for Font Awesome Regular.
pub const FA_REGULAR: &[u8] = include_bytes!("../assets/fa_solid.otf");

/// Create a text widget with an icon from Font Awesome.
pub fn icon<'a>(icon_char: char) -> Text<'a> {
    iced::widget::text(icon_char).font(FA_SOLID_FONT)
}

/// Create a text widget with an icon and label.
pub fn icon_text<'a>(icon_char: char, label: &str) -> String {
    format!("{}  {}", icon_char, label)
}

/// Icon codepoint constants (Font Awesome 7 Free).
pub mod icons {
    // Navigation
    pub const DASHBOARD: char = '\u{e3af}'; // chart-pie-simple or gauge
    pub const IMAGES: char = '\u{f302}'; // images
    pub const NETWORK: char = '\u{f6ff}'; // network-wired
    pub const DATABASE: char = '\u{f1c0}'; // database (for volumes)
    pub const SETTINGS: char = '\u{f013}'; // gear

    // Container states
    pub const CONTAINER: char = '\u{f4b8}'; // cube
    pub const PLAY: char = '\u{f04b}'; // play
    pub const STOP: char = '\u{f04d}'; // stop
    pub const PAUSE: char = '\u{f04c}'; // pause
    pub const RESTART: char = '\u{f01e}'; // rotate
    pub const TRASH: char = '\u{f2ed}'; // trash-can

    // Status indicators
    pub const CHECK: char = '\u{f00c}'; // check
    pub const TIMES: char = '\u{f00d}'; // xmark
    pub const CIRCLE: char = '\u{f111}'; // circle (solid)
    pub const CIRCLE_DOT: char = '\u{f192}'; // circle-dot
    pub const WARNING: char = '\u{f071}'; // triangle-exclamation

    // Actions
    pub const REFRESH: char = '\u{f021}'; // arrows-rotate
    pub const PLUS: char = '\u{f067}'; // plus
    pub const SEARCH: char = '\u{f002}'; // magnifying-glass
    pub const TERMINAL: char = '\u{f120}'; // terminal
    pub const FILE: char = '\u{f15b}'; // file
    pub const FOLDER: char = '\u{f07b}'; // folder

    // Misc
    pub const DOCKER: char = '\u{f395}'; // docker (brands - may not work in free)
    pub const INFO: char = '\u{f05a}'; // circle-info
    pub const ARROW_RIGHT: char = '\u{f061}'; // arrow-right
    pub const ARROW_LEFT: char = '\u{f060}'; // arrow-left
    pub const ELLIPSIS: char = '\u{f141}'; // ellipsis
}
