fn main() -> anyhow::Result<()> {
    tiron_node::node::start()?;
    Ok(())
}
