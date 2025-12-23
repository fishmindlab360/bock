//! bockrose CLI.

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use color_eyre::eyre::Result;
use tabled::{Table, Tabled};

use crate::orchestrator::Orchestrator;
use crate::spec::BockoseSpec;

/// bockrose - Multi-container orchestration for Bock
#[derive(Parser)]
#[command(name = "bockrose")]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Path to bockrose.yaml
    #[arg(short, long, default_value = "bockrose.yaml")]
    pub file: PathBuf,

    /// Project name
    #[arg(short, long)]
    pub project_name: Option<String>,

    /// Enable debug logging
    #[arg(long, global = true)]
    pub debug: bool,

    /// The subcommand to execute.
    #[command(subcommand)]
    pub command: Commands,
}

/// bockrose commands.
#[derive(Subcommand)]
pub enum Commands {
    /// Start services
    Up {
        /// Run in background
        #[arg(short, long)]
        detach: bool,

        /// Build images before starting
        #[arg(long)]
        build: bool,

        /// Force recreate containers
        #[arg(long)]
        force_recreate: bool,

        /// Specific services to start
        services: Vec<String>,
    },

    /// Stop services
    Down {
        /// Remove named volumes
        #[arg(short, long)]
        volumes: bool,

        /// Remove images
        #[arg(long)]
        rmi: Option<String>,

        /// Timeout for stopping
        #[arg(short, long, default_value = "10")]
        timeout: u64,
    },

    /// Build or rebuild services
    Build {
        /// Don't use cache
        #[arg(long)]
        no_cache: bool,

        /// Always pull base images
        #[arg(long)]
        pull: bool,

        /// Services to build
        services: Vec<String>,
    },

    /// List containers
    Ps {
        /// Show all containers (including stopped)
        #[arg(short, long)]
        all: bool,

        /// Only show IDs
        #[arg(short, long)]
        quiet: bool,
    },

    /// View service logs
    Logs {
        /// Follow log output
        #[arg(short, long)]
        follow: bool,

        /// Show timestamps
        #[arg(short, long)]
        timestamps: bool,

        /// Number of lines to show
        #[arg(short, long)]
        tail: Option<u64>,

        /// Service name
        service: Option<String>,
    },

    /// Execute a command in a running container
    Exec {
        /// Run in background
        #[arg(short, long)]
        detach: bool,

        /// Environment variables
        #[arg(short, long)]
        env: Vec<String>,

        /// Allocate a TTY
        #[arg(short = 'T', long)]
        no_tty: bool,

        /// User
        #[arg(short, long)]
        user: Option<String>,

        /// Working directory
        #[arg(short, long)]
        workdir: Option<String>,

        /// Service name
        service: String,

        /// Command and arguments
        #[arg(trailing_var_arg = true, required = true)]
        command: Vec<String>,
    },

    /// Scale services
    Scale {
        /// Service=replicas pairs
        #[arg(required = true)]
        scale: Vec<String>,
    },

    /// Restart services
    Restart {
        /// Timeout for stopping
        #[arg(short, long, default_value = "10")]
        timeout: u64,

        /// Services to restart
        services: Vec<String>,
    },

    /// Stop services
    Stop {
        /// Timeout for stopping
        #[arg(short, long, default_value = "10")]
        timeout: u64,

        /// Services to stop
        services: Vec<String>,
    },

    /// Start services
    Start {
        /// Services to start
        services: Vec<String>,
    },

    /// Pull service images
    Pull {
        /// Include dependencies
        #[arg(long)]
        include_deps: bool,

        /// Don't print progress
        #[arg(short, long)]
        quiet: bool,

        /// Services to pull
        services: Vec<String>,
    },

    /// Push service images
    Push {
        /// Services to push
        services: Vec<String>,
    },

    /// Validate and show configuration
    Config {
        /// Output format (yaml, json)
        #[arg(short, long, default_value = "yaml")]
        format: String,

        /// Only validate
        #[arg(short, long)]
        quiet: bool,
    },

    /// Print the public port for a service
    Port {
        /// Service name
        service: String,

        /// Private port
        private_port: u16,
    },

    /// Display service resource usage
    Top {
        /// Services
        services: Vec<String>,
    },
}

#[derive(Tabled)]
struct ServiceRow {
    #[tabled(rename = "NAME")]
    name: String,
    #[tabled(rename = "IMAGE")]
    image: String,
    #[tabled(rename = "STATUS")]
    status: String,
    #[tabled(rename = "PORTS")]
    ports: String,
}

impl Cli {
    /// Execute the CLI command.
    pub async fn execute(self) -> Result<()> {
        let spec = BockoseSpec::from_file(&self.file)?;
        let orchestrator = Orchestrator::new(spec);

        match self.command {
            Commands::Up {
                detach,
                build,
                force_recreate: _,
                services: _,
            } => {
                if build {
                    tracing::info!("Building services...");
                }
                orchestrator.up(detach).await?;
                if detach {
                    println!("Started in detached mode");
                }
                Ok(())
            }

            Commands::Down {
                volumes,
                rmi: _,
                timeout: _,
            } => {
                orchestrator.down(volumes).await?;
                println!("Stopped");
                Ok(())
            }

            Commands::Build {
                no_cache: _,
                pull: _,
                services: _,
            } => {
                println!("Building services...");
                // TODO: Implement
                Ok(())
            }

            Commands::Ps { all: _, quiet } => {
                let services = orchestrator.list_services();
                if quiet {
                    for s in services {
                        for c in s.containers {
                            println!("{}", c);
                        }
                    }
                } else {
                    let rows: Vec<ServiceRow> = services
                        .iter()
                        .map(|s| ServiceRow {
                            name: s.name.clone(),
                            image: "".to_string(),
                            status: format!("{:?}", s.status),
                            ports: "".to_string(),
                        })
                        .collect();

                    if rows.is_empty() {
                        println!("No services running");
                    } else {
                        let table = Table::new(rows).to_string();
                        println!("{}", table);
                    }
                }
                Ok(())
            }

            Commands::Logs {
                follow: _,
                timestamps: _,
                tail: _,
                service: _,
            } => {
                println!("Logs...");
                // TODO: Implement
                Ok(())
            }

            Commands::Exec {
                detach: _,
                env: _,
                no_tty: _,
                user: _,
                workdir: _,
                service,
                command,
            } => {
                println!("Executing {:?} in service {}", command, service);
                // TODO: Implement
                Ok(())
            }

            Commands::Scale { scale } => {
                for s in scale {
                    let parts: Vec<&str> = s.split('=').collect();
                    if parts.len() == 2 {
                        let service = parts[0];
                        let replicas: u32 = parts[1].parse()?;
                        orchestrator.scale(service, replicas).await?;
                        println!("Scaled {} to {} replicas", service, replicas);
                    }
                }
                Ok(())
            }

            Commands::Restart {
                timeout: _,
                services: _,
            } => {
                println!("Restarting services...");
                // TODO: Implement
                Ok(())
            }

            Commands::Stop {
                timeout: _,
                services: _,
            } => {
                println!("Stopping services...");
                // TODO: Implement
                Ok(())
            }

            Commands::Start { services: _ } => {
                println!("Starting services...");
                // TODO: Implement
                Ok(())
            }

            Commands::Pull {
                include_deps: _,
                quiet: _,
                services: _,
            } => {
                println!("Pulling images...");
                // TODO: Implement
                Ok(())
            }

            Commands::Push { services: _ } => {
                println!("Pushing images...");
                // TODO: Implement
                Ok(())
            }

            Commands::Config { format: _, quiet } => {
                if quiet {
                    println!("Configuration is valid");
                } else {
                    // TODO: Print config in requested format
                    println!("Configuration:");
                }
                Ok(())
            }

            Commands::Port {
                service,
                private_port,
            } => {
                println!("Port mapping for {}:{}", service, private_port);
                // TODO: Implement
                Ok(())
            }

            Commands::Top { services: _ } => {
                println!("Resource usage:");
                // TODO: Implement
                Ok(())
            }
        }
    }
}
