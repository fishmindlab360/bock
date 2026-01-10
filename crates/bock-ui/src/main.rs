//! Bock UI - Native container management interface.
//!
//! A modern GUI for managing containers built with Iced.

#![allow(missing_docs)]

mod app;
mod icons;
mod theme;

use iced::Size;

/// Application entry point.
fn main() -> iced::Result {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    tracing::info!("Starting Bock UI...");

    // Run the application with custom font
    iced::application("Bock", app::BockApp::update, app::BockApp::view)
        .theme(app::BockApp::theme)
        .window_size(Size::new(1200.0, 800.0))
        .font(icons::FA_SOLID)
        .run_with(app::BockApp::new)
}
