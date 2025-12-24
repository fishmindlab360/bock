//! Bock Runtime CLI.

use std::collections::HashMap;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use color_eyre::eyre::Result;

use crate::bockfile::Bockfile;
use crate::build::{BuildOptions, Builder};
use crate::cache::CacheManager;
use crate::registry::{Registry, inspect_local};

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

        /// Build arguments (KEY=VALUE)
        #[arg(long = "build-arg")]
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

        /// Output directory for OCI image
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Push an image to a registry
    Push {
        /// Local image path or reference
        source: String,

        /// Remote image reference (registry/repo:tag)
        destination: String,
    },

    /// Pull an image from a registry
    Pull {
        /// Image reference (registry/repo:tag)
        image: String,

        /// Output directory
        #[arg(short, long, default_value = ".")]
        output: PathBuf,
    },

    /// Inspect an image
    Inspect {
        /// Image reference or local path
        image: String,

        /// Format output as JSON
        #[arg(long)]
        json: bool,
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
    List {
        /// Show detailed information
        #[arg(short, long)]
        verbose: bool,
    },
    /// Prune old cache entries
    Prune {
        /// Remove entries older than N days
        #[arg(long, default_value = "7")]
        older_than: u64,
    },
    /// Clear all cache
    Clear {
        /// Don't ask for confirmation
        #[arg(short = 'y', long)]
        yes: bool,
    },
    /// Show cache statistics
    Stats,
}

impl Cli {
    /// Execute the CLI command.
    pub async fn execute(self) -> Result<()> {
        match self.command {
            Commands::Build {
                file,
                context,
                tag,
                args,
                target,
                no_cache,
                pull: _,
                output,
            } => {
                tracing::info!(
                    file = %file.display(),
                    context = %context.display(),
                    tag = ?tag,
                    "Building image"
                );

                // Parse build arguments
                let build_args: HashMap<String, String> = args
                    .iter()
                    .filter_map(|arg| {
                        let parts: Vec<&str> = arg.splitn(2, '=').collect();
                        if parts.len() == 2 {
                            Some((parts[0].to_string(), parts[1].to_string()))
                        } else {
                            eprintln!(
                                "Warning: Invalid build-arg format '{}'. Expected KEY=VALUE",
                                arg
                            );
                            None
                        }
                    })
                    .collect();

                let bockfile = Bockfile::from_file(&file)?;
                let tag = tag.unwrap_or_else(|| {
                    bockfile
                        .metadata
                        .name
                        .clone()
                        .map(|n| {
                            format!(
                                "{}:{}",
                                n,
                                bockfile.metadata.version.as_deref().unwrap_or("latest")
                            )
                        })
                        .unwrap_or_else(|| "bock-image:latest".to_string())
                });

                let options = BuildOptions {
                    args: build_args,
                    no_cache,
                    target,
                    output,
                };

                let builder = Builder::with_options(bockfile, context, tag.clone(), options);
                let result = builder.build().await?;

                println!("\nBuild complete!");
                println!("  Tag:    {}", result.tag);
                println!("  Digest: {}", result.digest);
                println!("  Layers: {}", result.layers);
                println!("  Size:   {} bytes", result.size);

                Ok(())
            }

            Commands::Push {
                source,
                destination,
            } => {
                tracing::info!(source = %source, destination = %destination, "Pushing image");

                // Parse destination: registry/repo:tag
                let (registry_url, repo, tag) = parse_image_ref(&destination)?;

                let registry = Registry::new(&registry_url);
                let source_path = PathBuf::from(&source);

                if !source_path.exists() {
                    return Err(color_eyre::eyre::eyre!(
                        "Source path does not exist: {}",
                        source
                    ));
                }

                let digest = registry.push(&source_path, &repo, &tag).await?;

                println!("Pushed {} to {}", source, destination);
                println!("Digest: {}", digest);

                Ok(())
            }

            Commands::Pull { image, output } => {
                tracing::info!(image = %image, "Pulling image");

                let (registry_url, repo, tag) = parse_image_ref(&image)?;
                let registry = Registry::new(&registry_url);

                let info = registry.pull(&repo, &tag, &output).await?;

                println!("Pulled {} to {}", image, output.display());
                println!("Digest: {}", info.digest);

                Ok(())
            }

            Commands::Inspect { image, json } => {
                tracing::info!(image = %image, "Inspecting image");

                let path = PathBuf::from(&image);

                let info = if path.exists() {
                    // Local image
                    inspect_local(&path)?
                } else {
                    // Remote image
                    let (registry_url, repo, tag) = parse_image_ref(&image)?;
                    let registry = Registry::new(&registry_url);
                    registry.inspect(&repo, &tag).await?
                };

                if json {
                    let output = serde_json::json!({
                        "digest": info.digest,
                        "tag": info.tag,
                        "architecture": info.architecture,
                        "os": info.os,
                        "created": info.created,
                        "author": info.author,
                        "layers": info.layer_count,
                        "size": info.size,
                        "config": {
                            "entrypoint": info.entrypoint,
                            "cmd": info.cmd,
                            "workdir": info.workdir,
                            "env": info.env,
                            "exposedPorts": info.exposed_ports,
                        },
                        "labels": info.labels,
                    });
                    println!("{}", serde_json::to_string_pretty(&output)?);
                } else {
                    println!("Image: {}", image);
                    println!("Digest: {}", info.digest);
                    if let Some(tag) = &info.tag {
                        println!("Tag: {}", tag);
                    }
                    println!("Architecture: {}", info.architecture);
                    println!("OS: {}", info.os);
                    if let Some(created) = &info.created {
                        println!("Created: {}", created);
                    }
                    println!("Layers: {}", info.layer_count);
                    println!("Size: {} bytes", info.size);
                    if !info.entrypoint.is_empty() {
                        println!("Entrypoint: {:?}", info.entrypoint);
                    }
                    if !info.cmd.is_empty() {
                        println!("Cmd: {:?}", info.cmd);
                    }
                    if let Some(workdir) = &info.workdir {
                        println!("WorkingDir: {}", workdir);
                    }
                    if !info.env.is_empty() {
                        println!("Environment:");
                        for env in &info.env {
                            println!("  {}", env);
                        }
                    }
                    if !info.exposed_ports.is_empty() {
                        println!("Exposed Ports: {:?}", info.exposed_ports);
                    }
                    if !info.labels.is_empty() {
                        println!("Labels:");
                        for (k, v) in &info.labels {
                            println!("  {}: {}", k, v);
                        }
                    }
                }

                Ok(())
            }

            Commands::Cache { command } => {
                let cache_dir = dirs::cache_dir()
                    .unwrap_or_else(|| PathBuf::from("/tmp"))
                    .join("bock")
                    .join("build-cache");

                let mut cache = CacheManager::new(&cache_dir);

                match command {
                    CacheCommands::List { verbose } => {
                        let entries = cache.list();

                        if entries.is_empty() {
                            println!("No cached layers");
                            return Ok(());
                        }

                        println!("Cached layers ({}):", entries.len());
                        println!();

                        for entry in entries {
                            if verbose {
                                println!("Key:     {}", entry.key);
                                println!("Size:    {}", entry.size_human());
                                println!("Created: {}", format_timestamp(entry.created));
                                println!("Accessed: {}", format_timestamp(entry.last_access));
                                if let Some(cmd) = &entry.command {
                                    println!("Command: {}", cmd);
                                }
                                println!();
                            } else {
                                println!(
                                    "  {} ({}) - {}",
                                    &entry.key[..12.min(entry.key.len())],
                                    entry.size_human(),
                                    format_timestamp(entry.last_access)
                                );
                            }
                        }

                        println!();
                        println!(
                            "Total: {} entries, {} total",
                            cache.entry_count(),
                            format_size(cache.total_size())
                        );

                        Ok(())
                    }

                    CacheCommands::Prune { older_than } => {
                        println!("Pruning cache entries older than {} days...", older_than);
                        let freed = cache.prune(older_than)?;
                        println!("Freed {} of disk space", format_size(freed));
                        Ok(())
                    }

                    CacheCommands::Clear { yes } => {
                        if !yes {
                            println!("This will delete all cached layers.");
                            print!("Continue? [y/N] ");
                            use std::io::{self, Write};
                            io::stdout().flush()?;

                            let mut input = String::new();
                            io::stdin().read_line(&mut input)?;

                            if !input.trim().eq_ignore_ascii_case("y") {
                                println!("Aborted");
                                return Ok(());
                            }
                        }

                        let freed = cache.clear()?;
                        println!("Cleared cache, freed {} of disk space", format_size(freed));
                        Ok(())
                    }

                    CacheCommands::Stats => {
                        println!("Build Cache Statistics");
                        println!("======================");
                        println!("Location: {}", cache.cache_dir().display());
                        println!("Entries:  {}", cache.entry_count());
                        println!("Size:     {}", format_size(cache.total_size()));
                        Ok(())
                    }
                }
            }
        }
    }
}

/// Parse image reference into (registry, repo, tag).
fn parse_image_ref(image: &str) -> Result<(String, String, String)> {
    // Handle formats:
    // - repo:tag -> default registry
    // - registry/repo:tag
    // - registry/repo (tag = latest)

    let (image_part, tag) = if let Some(idx) = image.rfind(':') {
        let potential_tag = &image[idx + 1..];
        // Check if this is actually a tag or part of a port number
        if potential_tag.contains('/') || potential_tag.parse::<u16>().is_ok() {
            (image, "latest".to_string())
        } else {
            (&image[..idx], potential_tag.to_string())
        }
    } else {
        (image, "latest".to_string())
    };

    let (registry, repo) = if let Some(idx) = image_part.find('/') {
        let first_part = &image_part[..idx];
        // Check if first part is a registry (has dot or colon or is localhost)
        if first_part.contains('.') || first_part.contains(':') || first_part == "localhost" {
            (
                format!("https://{}", first_part),
                image_part[idx + 1..].to_string(),
            )
        } else {
            // DockerHub with namespace
            (
                "https://registry-1.docker.io".to_string(),
                image_part.to_string(),
            )
        }
    } else {
        // Simple image name, use DockerHub
        (
            "https://registry-1.docker.io".to_string(),
            format!("library/{}", image_part),
        )
    };

    Ok((registry, repo, tag))
}

fn format_timestamp(ts: u64) -> String {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    let dt = UNIX_EPOCH + Duration::from_secs(ts);
    let now = SystemTime::now();

    if let Ok(diff) = now.duration_since(dt) {
        let secs = diff.as_secs();
        if secs < 60 {
            format!("{}s ago", secs)
        } else if secs < 3600 {
            format!("{}m ago", secs / 60)
        } else if secs < 86400 {
            format!("{}h ago", secs / 3600)
        } else {
            format!("{}d ago", secs / 86400)
        }
    } else {
        "unknown".to_string()
    }
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_image_ref_simple() {
        let (reg, repo, tag) = parse_image_ref("alpine").unwrap();
        assert!(reg.contains("docker"));
        assert!(repo.contains("alpine"));
        assert_eq!(tag, "latest");
    }

    #[test]
    fn test_parse_image_ref_with_tag() {
        let (_, repo, tag) = parse_image_ref("nginx:1.21").unwrap();
        assert!(repo.contains("nginx"));
        assert_eq!(tag, "1.21");
    }

    #[test]
    fn test_parse_image_ref_with_registry() {
        let (reg, repo, tag) = parse_image_ref("ghcr.io/user/image:v1").unwrap();
        assert!(reg.contains("ghcr.io"));
        assert_eq!(repo, "user/image");
        assert_eq!(tag, "v1");
    }
}
