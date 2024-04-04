use clap::Parser;

#[derive(Parser)]
#[clap(name = "lapon")]
#[clap(version = env!("CARGO_PKG_VERSION"))]
pub struct Cli {
    /// the runbooks for lapon to run
    pub runbooks: Vec<String>,
}
