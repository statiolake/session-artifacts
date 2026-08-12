mod daemon;
mod install;
mod model;
mod storage;
mod template;

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

use crate::daemon::Daemon;
use crate::model::{Provider, SessionRequest};

#[derive(Debug, Parser)]
#[command(
    name = "session-whiteboard",
    version,
    about = "Live HTML whiteboards for coding-agent sessions"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run the local daemon in the foreground.
    Daemon {
        #[command(subcommand)]
        command: Option<DaemonCommand>,
        /// Bind to this local TCP port. Port 0 selects a free port.
        #[arg(long, default_value_t = 0)]
        port: u16,
    },
    /// Prepare a session and return its HTML whiteboard path.
    Prepare {
        #[arg(long, value_enum)]
        provider: ProviderArg,
        #[arg(long)]
        session_id: String,
        #[arg(long, default_value = ".")]
        cwd: PathBuf,
        /// Emit only JSON, suitable for an agent or integration adapter.
        #[arg(long)]
        json: bool,
    },
    /// Start the managed daemon and invoke the browser viewer.
    Browse {
        /// Emit only JSON, suitable for an integration adapter.
        #[arg(long)]
        json: bool,
    },
    /// Clean a session whiteboard and its registry record permanently.
    Clean {
        #[arg(long, value_enum)]
        provider: ProviderArg,
        #[arg(long)]
        session_id: String,
        #[arg(long, default_value = ".")]
        cwd: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Print the agent instructions for using session whiteboards.
    Skill {
        #[arg(long, value_enum, default_value_t = ProviderArg::Generic)]
        provider: ProviderArg,
    },
    /// Install provider-specific global skills and hooks.
    Install {
        /// Install for both supported providers when omitted.
        #[arg(long, value_enum)]
        provider: Option<ProviderArg>,
    },
    /// Remove provider-specific global skills and hooks.
    Uninstall {
        /// Uninstall for both supported providers when omitted.
        #[arg(long, value_enum)]
        provider: Option<ProviderArg>,
    },
}

#[derive(Debug, Subcommand)]
enum DaemonCommand {
    /// Start the managed background daemon if it is not already running.
    Start,
    /// Stop the managed background daemon.
    Stop,
    /// Restart the managed background daemon.
    Restart,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ProviderArg {
    Codex,
    Claude,
    Generic,
}

impl From<ProviderArg> for Provider {
    fn from(value: ProviderArg) -> Self {
        match value {
            ProviderArg::Codex => Self::Codex,
            ProviderArg::Claude => Self::Claude,
            ProviderArg::Generic => Self::Generic,
        }
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("session-whiteboard: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cli = Cli::parse();
    match cli.command {
        Command::Daemon { command, port } => match command {
            None => Daemon::run_foreground(port),
            Some(DaemonCommand::Start) => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&Daemon::start_managed()?)?
                );
                Ok(())
            }
            Some(DaemonCommand::Stop) => {
                println!("stopped={}", Daemon::stop_managed()?);
                Ok(())
            }
            Some(DaemonCommand::Restart) => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&Daemon::restart_managed()?)?
                );
                Ok(())
            }
        },
        Command::Prepare {
            provider,
            session_id,
            cwd,
            json,
        } => {
            let cwd = std::fs::canonicalize(cwd)?;
            let request = SessionRequest {
                provider: provider.into(),
                session_id,
                cwd,
            };
            let response = Daemon::prepare_via_client(&request)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&response)?);
            } else {
                println!("artifact_path={}", response.artifact_path.display());
                println!("viewer_url={}", response.viewer_url);
                if let Some(warning) = response.warning {
                    eprintln!("warning={warning}");
                }
            }
            Ok(())
        }
        Command::Browse { json } => {
            let response = Daemon::browse_via_client()?;
            if json {
                println!("{}", serde_json::to_string_pretty(&response)?);
            } else {
                println!("viewer_url={}", response.viewer_url);
                println!("opened={}", response.opened);
            }
            Ok(())
        }
        Command::Clean {
            provider,
            session_id,
            cwd,
            json,
        } => {
            let cwd = std::fs::canonicalize(cwd)?;
            let response = Daemon::clean_via_client(&SessionRequest {
                provider: provider.into(),
                session_id,
                cwd,
            })?;
            if json {
                println!("{}", serde_json::to_string_pretty(&response)?);
            } else if response.cleaned {
                println!("cleaned=true");
            } else {
                println!("cleaned=false");
            }
            Ok(())
        }
        Command::Skill { provider } => {
            print!("{}", install::skill_text(provider.into()));
            Ok(())
        }
        Command::Install { provider } => {
            let result = install::install(provider.map(Into::into))?;
            println!("{}", serde_json::to_string_pretty(&result)?);
            Ok(())
        }
        Command::Uninstall { provider } => {
            let result = install::uninstall(provider.map(Into::into))?;
            println!("{}", serde_json::to_string_pretty(&result)?);
            Ok(())
        }
    }
}
