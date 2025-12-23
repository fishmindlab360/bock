//! Bock CLI entry point.

use clap::Parser;
use color_eyre::eyre::Result;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

use bock::cli::Cli;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize error handling
    color_eyre::install()?;

    // Initialize tracing
    tracing_subscriber::registry()
        .with(fmt::layer().with_target(true))
        .with(EnvFilter::from_default_env().add_directive("bock=info".parse()?))
        .init();

    // Parse CLI arguments
    let cli = Cli::parse();

    // Execute command
    cli.execute().await
}
