//! Bock Runtime CLI.

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use color_eyre::eyre::Result;

use crate::bockfile::Bockfile;
use crate::build::Builder;

/// Bock Runtime - Spec-driven container image builder
#[derive(Parser)]
#[command(name = "bock-runtime")]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Enable debug logging
    #[arg(long, global = true)]
    pub debug: bool,

    /// Subcommand to execute.
    #[command(subcommand)]
    pub command: Commands,
}

/// Bock runtime commands.
#[derive(Subcommand)]
pub enum Commands {
    /// Build a container image
    Build {
        /// Path to Bockfile
        #[arg(short, long, default_value = "Bockfile.yaml")]
        file: PathBuf,

        /// Build context directory
        #[arg(default_value = ".")]
        context: PathBuf,

        /// Image tag
        #[arg(short, long)]
        tag: Option<String>,

        /// Build arguments
        #[arg(long = "arg")]
        args: Vec<String>,

        /// Target stage to build
        #[arg(long)]
        target: Option<String>,

        /// Don't use cache
        #[arg(long)]
        no_cache: bool,

        /// Pull base image
        #[arg(long)]
        pull: bool,
    },

    /// Push an image to a registry
    Push {
        /// Local image reference
        source: String,

        /// Remote image reference
        destination: String,
    },

    /// Inspect an image
    Inspect {
        /// Image reference
        image: String,
    },

    /// Manage build cache
    Cache {
        /// Cache subcommands.
        #[command(subcommand)]
        command: CacheCommands,
    },
}

/// Build cache management subcommands.
#[derive(Subcommand)]
pub enum CacheCommands {
    /// List cached layers
    List,
    /// Prune old cache entries
    Prune {
        /// Remove entries older than N days
        #[arg(long, default_value = "7")]
        older_than: u64,
    },
    /// Clear all cache
    Clear,
}

impl Cli {
    /// Execute the CLI command.
    pub async fn execute(self) -> Result<()> {
        match self.command {
            Commands::Build {
                file,
                context,
                tag,
                args: _,
                target: _,
                no_cache: _,
                pull: _,
            } => {
                tracing::info!(
                    file = %file.display(),
                    context = %context.display(),
                    tag = ?tag,
                    "Building image"
                );

                let bockfile = Bockfile::from_file(&file)?;
                let tag = tag.unwrap_or_else(|| "latest".to_string());

                let builder = Builder::new(bockfile, context, tag.clone());
                let digest = builder.build().await?;

                println!("Successfully built {:?}", digest);
                Ok(())
            }

            Commands::Push {
                source,
                destination,
            } => {
                tracing::info!(source = %source, destination = %destination, "Pushing image");
                println!("Pushing {} to {}", source, destination);
                // TODO: Implement
                Ok(())
            }

            Commands::Inspect { image } => {
                tracing::info!(image = %image, "Inspecting image");
                // TODO: Implement
                println!("Image: {}", image);
                Ok(())
            }

            Commands::Cache { command } => match command {
                CacheCommands::List => {
                    println!("Cached layers:");
                    // TODO: Implement
                    Ok(())
                }
                CacheCommands::Prune { older_than } => {
                    println!("Pruning cache entries older than {} days", older_than);
                    // TODO: Implement
                    Ok(())
                }
                CacheCommands::Clear => {
                    println!("Clearing all cache");
                    // TODO: Implement
                    Ok(())
                }
            },
        }
    }
}
