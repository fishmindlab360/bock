//! CLI command definitions and handlers.

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use color_eyre::eyre::Result;

/// Bock - Modern Container Runtime
#[derive(Parser)]
#[command(name = "bock")]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    /// Root directory for bock data
    #[arg(
        long,
        global = true,
        env = "BOCK_ROOT",
        default_value = "/var/lib/bock"
    )]
    pub root: PathBuf,

    /// Enable debug logging
    #[arg(long, global = true)]
    pub debug: bool,

    /// The subcommand to execute.
    #[command(subcommand)]
    pub command: Commands,
}

/// Helper commands.
#[derive(Subcommand)]
pub enum Commands {
    /// Create a container
    Create {
        /// Container ID
        container_id: String,

        /// Path to the OCI bundle
        #[arg(short, long)]
        bundle: PathBuf,

        /// Path to console socket
        #[arg(long)]
        console_socket: Option<PathBuf>,

        /// Path to PID file
        #[arg(long)]
        pid_file: Option<PathBuf>,

        /// Do not use pivot_root
        #[arg(long)]
        no_pivot: bool,

        /// Do not create new namespaces
        #[arg(long)]
        no_new_keyring: bool,
    },

    /// Start a created container
    Start {
        /// Container ID
        container_id: String,
    },

    /// Create and start a container
    Run {
        /// Container ID
        container_id: String,

        /// Path to the OCI bundle
        #[arg(short, long)]
        bundle: PathBuf,

        /// Path to console socket
        #[arg(long)]
        console_socket: Option<PathBuf>,

        /// Path to PID file
        #[arg(long)]
        pid_file: Option<PathBuf>,

        /// Detach from the container
        #[arg(short, long)]
        detach: bool,

        /// Keep stdin open
        #[arg(short, long)]
        keep_stdin: bool,
    },

    /// Query container state
    State {
        /// Container ID
        container_id: String,
    },

    /// Kill a running container
    Kill {
        /// Container ID
        container_id: String,

        /// Signal to send (default: SIGTERM)
        #[arg(default_value = "SIGTERM")]
        signal: String,

        /// Send signal to all processes
        #[arg(short, long)]
        all: bool,
    },

    /// Delete a container
    Delete {
        /// Container ID
        container_id: String,

        /// Force deletion of running container
        #[arg(short, long)]
        force: bool,
    },

    /// List containers
    List {
        /// Output format (table, json)
        #[arg(short, long, default_value = "table")]
        format: String,

        /// Only display container IDs
        #[arg(short, long)]
        quiet: bool,
    },

    /// Execute a command in a running container
    Exec {
        /// Container ID
        container_id: String,

        /// Path to console socket
        #[arg(long)]
        console_socket: Option<PathBuf>,

        /// Current working directory
        #[arg(long)]
        cwd: Option<PathBuf>,

        /// Environment variables
        #[arg(short, long)]
        env: Vec<String>,

        /// Allocate a pseudo-TTY
        #[arg(short, long)]
        tty: bool,

        /// User to run as (uid:gid)
        #[arg(short, long)]
        user: Option<String>,

        /// Process ID file
        #[arg(long)]
        pid_file: Option<PathBuf>,

        /// Detach from the exec session
        #[arg(short, long)]
        detach: bool,

        /// Command and arguments
        #[arg(trailing_var_arg = true, required = true)]
        command: Vec<String>,
    },

    /// Pause a running container
    Pause {
        /// Container ID
        container_id: String,
    },

    /// Resume a paused container
    Resume {
        /// Container ID
        container_id: String,
    },

    /// Output events for containers
    Events {
        /// Output stats periodically
        #[arg(long)]
        stats: bool,

        /// Time interval for stats (seconds)
        #[arg(long, default_value = "5")]
        interval: u64,

        /// Container ID
        container_id: String,
    },

    /// Display container resource usage statistics
    Stats {
        /// Container ID
        container_id: String,

        /// Output format (table, json)
        #[arg(short, long, default_value = "table")]
        format: String,
    },

    /// Update container resource limits
    Update {
        /// Container ID
        container_id: String,

        /// Path to resources JSON file
        #[arg(short, long)]
        resources: PathBuf,
    },

    /// Generate OCI spec
    Spec {
        /// Path to output file (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Generate rootless spec
        #[arg(long)]
        rootless: bool,
    },

    /// Show container features
    Features,

    /// Checkpoint a running container (CRIU)
    Checkpoint {
        /// Container ID
        container_id: String,

        /// Path to checkpoint image directory
        #[arg(long)]
        image_path: PathBuf,

        /// Leave container running after checkpoint
        #[arg(long)]
        leave_running: bool,
    },

    /// Restore a container from checkpoint
    Restore {
        /// Container ID
        container_id: String,

        /// Path to checkpoint image directory
        #[arg(long)]
        image_path: PathBuf,

        /// Path to OCI bundle
        #[arg(short, long)]
        bundle: PathBuf,
    },

    /// Fetch container logs
    Logs {
        /// Container ID
        container_id: String,

        /// Follow log output
        #[arg(short, long)]
        follow: bool,
    },
}

impl Cli {
    /// Execute the CLI command.
    pub async fn execute(self) -> Result<()> {
        let config = crate::runtime::RuntimeConfig::default().with_root(self.root.clone());
        let state_manager = crate::runtime::StateManager::new(config.paths.containers());

        match self.command {
            Commands::Create {
                container_id,
                bundle,
                console_socket: _,
                pid_file: _,
                no_pivot: _,
                no_new_keyring: _,
            } => {
                let spec_path = bundle.join("config.json");
                if !spec_path.exists() {
                    return Err(color_eyre::eyre::eyre!(
                        "Bundle config.json not found at {}",
                        spec_path.display()
                    ));
                }

                let spec_json = std::fs::read_to_string(&spec_path)?;
                let spec: bock_oci::Spec = serde_json::from_str(&spec_json)?;

                crate::runtime::Container::create(container_id.clone(), bundle, &spec, config)
                    .await
                    .map_err(|e| color_eyre::eyre::eyre!("Failed to create container: {}", e))?;

                println!("Container {} created", container_id);
                Ok(())
            }

            Commands::Start { container_id } => {
                let container = crate::runtime::Container::load(&container_id, config)
                    .await
                    .map_err(|e| color_eyre::eyre::eyre!("Failed to load container: {}", e))?;

                container
                    .start()
                    .await
                    .map_err(|e| color_eyre::eyre::eyre!("Failed to start container: {}", e))?;

                println!("Container {} started", container_id);
                Ok(())
            }

            Commands::Run {
                container_id,
                bundle,
                console_socket: _,
                pid_file: _,
                detach: _,
                keep_stdin: _,
            } => {
                let spec_path = bundle.join("config.json");
                if !spec_path.exists() {
                    return Err(color_eyre::eyre::eyre!(
                        "Bundle config.json not found at {}",
                        spec_path.display()
                    ));
                }

                let spec_json = std::fs::read_to_string(&spec_path)?;
                let spec: bock_oci::Spec = serde_json::from_str(&spec_json)?;

                let container =
                    crate::runtime::Container::create(container_id.clone(), bundle, &spec, config)
                        .await
                        .map_err(|e| {
                            color_eyre::eyre::eyre!("Failed to create container: {}", e)
                        })?;

                container
                    .start()
                    .await
                    .map_err(|e| color_eyre::eyre::eyre!("Failed to start container: {}", e))?;

                println!("Container {} running", container_id);
                Ok(())
            }

            Commands::State { container_id } => {
                let container = crate::runtime::Container::load(&container_id, config)
                    .await
                    .map_err(|e| color_eyre::eyre::eyre!("Failed to load container: {}", e))?;

                let state = container.state();
                let json = serde_json::to_string_pretty(&state)?;
                println!("{}", json);
                Ok(())
            }

            Commands::Kill {
                container_id,
                signal,
                all: _,
            } => {
                let container = crate::runtime::Container::load(&container_id, config)
                    .await
                    .map_err(|e| color_eyre::eyre::eyre!("Failed to load container: {}", e))?;

                let sig = match signal.as_str() {
                    "SIGTERM" => 15, // libc::SIGTERM not available directly here easily without dependency check
                    "SIGKILL" => 9,
                    s => s.parse::<i32>().unwrap_or(15),
                };

                container
                    .kill(sig)
                    .await
                    .map_err(|e| color_eyre::eyre::eyre!("Failed to kill container: {}", e))?;

                println!("Signal {} sent to container {}", signal, container_id);
                Ok(())
            }

            Commands::Delete {
                container_id,
                force: _,
            } => {
                let container = crate::runtime::Container::load(&container_id, config)
                    .await
                    .map_err(|e| color_eyre::eyre::eyre!("Failed to load container: {}", e))?;

                container
                    .delete()
                    .await
                    .map_err(|e| color_eyre::eyre::eyre!("Failed to delete container: {}", e))?;

                // Remove state
                state_manager
                    .delete(&container_id)
                    .map_err(|e| color_eyre::eyre::eyre!("Failed to delete state: {}", e))?;

                println!("Container {} deleted", container_id);
                Ok(())
            }

            Commands::List { format, quiet } => {
                let ids = state_manager
                    .list()
                    .map_err(|e| color_eyre::eyre::eyre!("Failed to list containers: {}", e))?;

                if quiet {
                    for id in ids {
                        println!("{}", id);
                    }
                } else if format == "json" {
                    let mut list = Vec::new();
                    for id in ids {
                        if let Ok(c) = state_manager.load(&id) {
                            list.push(c);
                        }
                    }
                    println!("{}", serde_json::to_string_pretty(&list)?);
                } else {
                    println!("ID\tSTATUS\tBUNDLE");
                    for id in ids {
                        if let Ok(state) = state_manager.load(&id) {
                            println!(
                                "{}\t{}\t{}",
                                state.id,
                                state.status,
                                std::path::PathBuf::from(&state.bundle).display()
                            );
                        }
                    }
                }
                Ok(())
            }

            Commands::Logs {
                container_id,
                follow,
            } => {
                let container_dir = config.paths.container(&container_id);
                // Try stdout first, maybe stderr? For now just stdout.
                let log_path = container_dir.join("stdout.log");

                if !log_path.exists() {
                    return Err(color_eyre::eyre::eyre!(
                        "Log file not found for container {}. (Note: only started containers have logs)",
                        container_id
                    ));
                }

                let file = std::fs::File::open(&log_path)?;
                let mut reader = std::io::BufReader::new(file);
                let mut line = String::new();

                loop {
                    line.clear();
                    match std::io::BufRead::read_line(&mut reader, &mut line) {
                        Ok(0) => {
                            if !follow {
                                break;
                            }
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        }
                        Ok(_) => {
                            print!("{}", line);
                        }
                        Err(e) => return Err(color_eyre::eyre::eyre!("Failed to read log: {}", e)),
                    }
                }
                Ok(())
            }

            // ... unimplemented stubs for Exec, Pause, Resume, Checkpoint ...
            _ => {
                println!("Command not fully implemented yet");
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_cli() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }
}
