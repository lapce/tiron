fn main() -> anyhow::Result<()> {
    lapon_node::node::start()?;
    Ok(())
}
