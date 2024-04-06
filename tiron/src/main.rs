use clap::Parser;

fn main() -> anyhow::Result<()> {
    let cli = tiron::cli::Cli::parse();
    tiron::core::start(&cli)
}
