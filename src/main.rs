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
    name = "session-artifacts",
    version,
    about = "Live HTML artifacts for coding-agent sessions"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run the local daemon in the foreground.
    Daemon {
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
    /// Print the agent instructions for using session artifacts.
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
    /// Emit hook context for a provider. Reads provider event JSON from stdin.
    Hook {
        #[arg(long, value_enum)]
        provider: ProviderArg,
    },
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
        eprintln!("session-artifacts: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cli = Cli::parse();
    match cli.command {
        Command::Daemon { port } => Daemon::run_foreground(port),
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
        Command::Hook { provider } => {
            let mut input = String::new();
            std::io::stdin().read_to_string(&mut input)?;
            print!("{}", install::hook_context(provider.into(), &input));
            Ok(())
        }
    }
}
