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
        ///
        /// Default to main.tr if unspecified
        runbooks: Vec<String>,
    },
    /// Check Tiron runbooks
    Check {
        /// The runbooks for Tiron to check.
        ///
        /// Default to main.tr if unspecified
        runbooks: Vec<String>,
    },
    /// Format Tiron runbooks
    Fmt {
        /// If unspecified, Tiron will scan the current directory for *.tr files.
        ///
        /// If you provide a directory, it will scan that directory.
        ///
        /// If you provide a file, it will only format that file.
        targets: Vec<String>,
    },
    /// Show Tiron action docs
    Action {
        /// name of the action
        name: Option<String>,
    },
    #[clap(hide = true)]
    GenerateDoc,
}
