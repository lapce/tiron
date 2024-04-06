use clap::Parser;

#[derive(Parser)]
#[clap(name = "tiron")]
#[clap(version = env!("CARGO_PKG_VERSION"))]
pub struct Cli {
    /// the runbooks for tiron to run
    pub runbooks: Vec<String>,
}
