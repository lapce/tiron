use crossbeam_channel::Sender;
use tiron_common::action::{ActionId, ActionMessage};

mod command;
mod copy;
pub mod data;
mod package;

pub trait Action {
    fn input(
        &self,
        cwd: &std::path::Path,
        params: Option<&rcl::runtime::Value>,
    ) -> anyhow::Result<Vec<u8>>;
    fn execute(
        &self,
        id: ActionId,
        input: &[u8],
        tx: &Sender<ActionMessage>,
    ) -> anyhow::Result<String>;
}
