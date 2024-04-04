use clap::Parser;

fn main() -> anyhow::Result<()> {
    let cli = lapon::cli::Cli::parse();
    lapon::core::start(&cli)
}
