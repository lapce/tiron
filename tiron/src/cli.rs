use clap::{Parser, Subcommand};

#[derive(Parser)]
#[clap(name = "tiron")]
#[clap(about = "A reasonable automation engine")]
#[clap(version = env!("CARGO_PKG_VERSION"))]
pub struct Cli {
    #[command(subcommand)]
    pub cmd: CliCmd,
}

#[derive(Debug, Subcommand)]
pub enum CliCmd {
    /// Run Tiron runbooks
    Run {
        /// The runbooks for Tiron to run.
        /// Default to main.rcl if unspecified
        runbooks: Vec<String>,
    },
    /// Check Tiron runbooks
    Check {
        /// The runbooks for Tiron to check.
        /// Default to main.rcl if unspecified
        runbooks: Vec<String>,
    },
    /// Show Tiron action docs
    Action {
        /// name of the action
        name: Option<String>,
    },
}
