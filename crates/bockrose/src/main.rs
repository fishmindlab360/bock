//! bockrose CLI entry point.

use clap::Parser;
use color_eyre::eyre::Result;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

use bockrose::cli::Cli;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    tracing_subscriber::registry()
        .with(fmt::layer().with_target(true))
        .with(EnvFilter::from_default_env().add_directive("bockrose=info".parse()?))
        .init();

    let cli = Cli::parse();
    cli.execute().await
}
