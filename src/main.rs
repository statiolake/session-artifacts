mod daemon;
mod install;
mod model;
mod storage;
mod template;

use std::io::Read;
use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

use crate::daemon::Daemon;
use crate::model::{OpenRequest, Provider};

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
    /// Create or reactivate the HTML artifact for a session.
    Open {
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
    /// Mark a session inactive. The HTML remains on disk for resume.
    Close {
        #[arg(long, value_enum)]
        provider: ProviderArg,
        #[arg(long)]
        session_id: String,
        #[arg(long, default_value = ".")]
        cwd: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Permanently delete a session artifact and its registry record.
    Delete {
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
    /// Emit hook context for a provider. Reads provider event JSON from stdin.
    Hook {
        #[arg(long, value_enum)]
        provider: ProviderArg,
    },
    /// Deactivate a session from a provider SessionEnd hook.
    SessionEnd {
        #[arg(long, value_enum)]
        provider: ProviderArg,
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
        Command::Open {
            provider,
            session_id,
            cwd,
            json,
        } => {
            let cwd = std::fs::canonicalize(cwd)?;
            let request = OpenRequest {
                provider: provider.into(),
                session_id,
                cwd,
            };
            let response = Daemon::open_via_client(&request)?;
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
        Command::Close {
            provider,
            session_id,
            cwd,
            json,
        } => {
            let cwd = std::fs::canonicalize(cwd)?;
            let response = Daemon::close_via_client(&OpenRequest {
                provider: provider.into(),
                session_id,
                cwd,
            })?;
            if json {
                println!("{}", serde_json::to_string_pretty(&response)?);
            } else {
                println!("closed={}", response.closed);
            }
            Ok(())
        }
        Command::Delete {
            provider,
            session_id,
            cwd,
            json,
        } => {
            let cwd = std::fs::canonicalize(cwd)?;
            let response = Daemon::delete_via_client(&OpenRequest {
                provider: provider.into(),
                session_id,
                cwd,
            })?;
            if json {
                println!("{}", serde_json::to_string_pretty(&response)?);
            } else if response.deleted {
                println!("deleted=true");
            } else {
                println!("deleted=false");
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
        Command::Hook { provider } => {
            let mut input = String::new();
            std::io::stdin().read_to_string(&mut input)?;
            print!("{}", install::hook_context(provider.into(), &input));
            Ok(())
        }
        Command::SessionEnd { provider } => {
            let mut input = String::new();
            std::io::stdin().read_to_string(&mut input)?;
            let request = install::hook_session_end_request(provider.into(), &input)?;
            let _ = Daemon::close_via_client(&request)?;
            Ok(())
        }
    }
}
